#!/usr/bin/env python3
"""Deterministically race the paranoid yield broker against signals (Linux/root).

Seize only the broker, never Ginux or the workload: Ginux remains the
workload's ptracer. Interrupt the broker's own receive, then hold it at its
real receive and reply ioctls while a signal takes the workload out of its
pending sched_yield. Both ioctls must report the race with ENOENT and the
broker must keep serving. No test hook, timing-based race, or replacement runtime is used.
"""
import ctypes
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time

RECV, SEND = 0xc0502100, 0xc0182101
ENOENT, EINTR, ERESTARTSYS = 2, 4, 512


class Regs(ctypes.Structure):
    _fields_ = [(name, ctypes.c_ulonglong) for name in (
        "r15 r14 r13 r12 rbp rbx r11 r10 r9 r8 rax rcx rdx rsi rdi orig_rax "
        "rip cs eflags rsp ss fs_base gs_base ds es fs gs").split()]


def main():
    if len(sys.argv) != 3 or os.geteuid() != 0:
        sys.exit("usage (as root): test-native-yield-broker.py <ginux> <native fixture>")
    binary, fixture = (Path(arg).resolve() for arg in sys.argv[1:])
    template = (Path(__file__).resolve().parent.parent / "config/instance-gate5.toml").read_text()
    if 'control = "paranoid"' not in template:
        raise RuntimeError("the native template no longer selects the paranoid yield broker")
    libc = ctypes.CDLL(None, use_errno=True)
    libc.ptrace.argtypes = [ctypes.c_uint, ctypes.c_uint, ctypes.c_void_p, ctypes.c_void_p]
    libc.ptrace.restype = ctypes.c_long

    def ptrace(request, pid, address=0, data=0):
        ctypes.set_errno(0)
        value = libc.ptrace(request, pid, address, data)
        if value == -1 and ctypes.get_errno():
            raise OSError(ctypes.get_errno(), f"ptrace {request}")
        return value

    with tempfile.TemporaryDirectory(prefix="ginux-yield-broker-") as directory:
        work = Path(directory)
        work.chmod(0o755)
        shutil.copy2(binary, work / "ginux")
        (work / "ginux").chmod(0o755)
        root = work / "root"
        (root / "usr/bin").mkdir(parents=True)
        shutil.copy2(fixture, root / "usr/bin/bash")
        (root / "usr/bin/bash").chmod(0o755)
        (root / "scenario").write_text("Yield-broker")
        config = work / "instance.toml"
        config.write_text(template.replace('init = "/bin/bash"', 'init = "/usr/bin/bash"'))
        ginux = subprocess.Popen([str(work / "ginux"), "native", str(config), str(root)],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 stderr=subprocess.STDOUT)
        os.set_blocking(ginux.stdout.fileno(), False)
        output = bytearray()
        broker = workload = None

        def drain():
            while True:
                try:
                    data = os.read(ginux.stdout.fileno(), 65536)
                except BlockingIOError:
                    return
                if not data:
                    return
                output.extend(data)

        def descendants(pid):
            found = []
            try:
                for task in Path(f"/proc/{pid}/task").iterdir():
                    for child in (task / "children").read_text().split():
                        found += [int(child)] + descendants(int(child))
            except OSError:
                pass
            return found

        def running(pid, command):
            try:
                return Path(f"/proc/{pid}/comm").read_text() == command + "\n"
            except OSError:
                return False

        def blocked_in(pid, syscall, argument=None):
            try:
                fields = Path(f"/proc/{pid}/syscall").read_text().split()
            except OSError:
                return False
            return (fields[0] == str(syscall)
                    and (argument is None or int(fields[2], 16) == argument))

        def until(condition, what):
            while not condition():
                drain()
                if ginux.poll() is not None:
                    drain()
                    raise RuntimeError(f"Ginux exited while waiting for {what}: "
                                       f"exit={ginux.returncode}\n{output.decode(errors='replace')}")
                time.sleep(0.001)

        def release_yield():
            ginux.stdin.write(b"g")
            ginux.stdin.flush()

        def step():
            """Resume the broker to its next syscall stop: (op, request or result)."""
            ptrace(24, broker)
            while True:
                _, status = os.waitpid(broker, 0x40000000)
                if os.WIFEXITED(status) or os.WIFSIGNALED(status):
                    drain()
                    raise RuntimeError(f"yield broker ended: status={status}\n"
                                       f"{output.decode(errors='replace')}")
                if os.WSTOPSIG(status) == signal.SIGTRAP | 0x80:
                    info = ctypes.create_string_buffer(128)
                    ptrace(0x420e, broker, len(info), ctypes.addressof(info))
                    regs = Regs()
                    ptrace(12, broker, 0, ctypes.addressof(regs))
                    if info.raw[0] == 1:
                        return "entry", regs.orig_rax, regs.rsi
                    return "exit", regs.orig_rax, ctypes.c_longlong(regs.rax).value
                ptrace(24, broker, 0, os.WSTOPSIG(status) if status >> 16 == 0 else 0)

        def step_to(op, request):
            while True:
                stop = step()
                if stop[0] == op and stop[1] == 16 and (op == "exit" or stop[2] == request):
                    return stop[2]

        def expired(_signal, _frame):
            raise TimeoutError(f"yield broker fixture timed out\n{output.decode(errors='replace')}")

        previous_alarm = signal.signal(signal.SIGALRM, expired)
        signal.setitimer(signal.ITIMER_REAL, 30)
        try:
            # The workload waits on stdin before every yield, so the broker
            # is blocked in a receive with nothing pending.
            def find_broker():
                nonlocal broker
                broker = next((pid for pid in descendants(ginux.pid)
                               if blocked_in(pid, 16, RECV)), None)
                return broker is not None
            until(find_broker, "the broker's receive")
            # Ginux forks the broker at the workload's execve stop, so the
            # broker can be waiting before the workload has replaced Ginux.
            def find_workload():
                nonlocal workload
                workload = next((pid for pid in descendants(ginux.pid)
                                 if running(pid, "bash")), None)
                return workload is not None
            until(find_workload, "the workload's exec")
            # TRACESYSGOOD and EXITKILL. The interrupt wakes the receive:
            # Linux 6.6 and later restart it, earlier kernels return EINTR.
            ptrace(0x4206, broker, 0, 1 | (1 << 20))
            ptrace(0x4207, broker)
            _, status = os.waitpid(broker, 0x40000000)
            regs = Regs()
            ptrace(12, broker, 0, ctypes.addressof(regs))
            if (status >> 16 != 128 or regs.orig_rax != 16
                    or ctypes.c_longlong(regs.rax).value not in (-EINTR, -ERESTARTSYS)):
                raise RuntimeError(f"broker receive was not interrupted: status={status:#x} "
                                   f"syscall={regs.orig_rax} result={ctypes.c_longlong(regs.rax).value}")
            # A signal ends the pending yield before the broker receives it.
            step_to("entry", RECV)
            release_yield()
            until(lambda: blocked_in(workload, 24), "the first yield")
            os.kill(workload, signal.SIGUSR1)
            until(lambda: b"GINUX_FIXTURE: yield-before-receive=" in output, "the first result")
            if (result := step_to("exit", RECV)) != -ENOENT:
                raise RuntimeError(f"stale receive returned {result}, not -ENOENT")
            # A signal ends the yield after the receive, before the reply.
            step_to("entry", RECV)
            release_yield()
            if (result := step_to("exit", RECV)) != 0:
                raise RuntimeError(f"receive returned {result}")
            step_to("entry", SEND)
            until(lambda: blocked_in(workload, 24), "the second yield")
            os.kill(workload, signal.SIGUSR1)
            until(lambda: b"GINUX_FIXTURE: yield-before-reply=" in output, "the second result")
            if (result := step_to("exit", SEND)) != -ENOENT:
                raise RuntimeError(f"late reply returned {result}, not -ENOENT")
            ptrace(17, broker)
            broker = None
            release_yield()
            ginux.stdin.close()
            ginux.wait()
            drain()
        finally:
            signal.setitimer(signal.ITIMER_REAL, 0)
            signal.signal(signal.SIGALRM, previous_alarm)
            if ginux.poll() is None:
                for pid in descendants(ginux.pid):
                    try:
                        os.kill(pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                ginux.kill()
                ginux.wait()
        text = output.decode(errors="replace")
        records = re.findall(r"^GINUX_NATIVE_AUDIT: .*operation=syscall\.sched_yield .*$", text, re.M)
        if (ginux.returncode != 0
                or "GINUX_FIXTURE: yield-before-receive=-4\n" not in text
                or "GINUX_FIXTURE: yield-before-reply=-4\n" not in text
                or "GINUX_FIXTURE: yield-after=0\n" not in text
                or "GINUX_FIXTURE: yield-signals=2\n" not in text
                or len(records) != 2
                or not all(record.endswith(" result=0") for record in records)):
            raise RuntimeError(f"yield broker did not survive the races: exit={ginux.returncode}\n{text}")
        print("PASS native yield broker: interrupted receive retried")
        print("PASS native yield broker: yield interrupted before receive or reply; broker keeps serving")


if __name__ == "__main__":
    main()
