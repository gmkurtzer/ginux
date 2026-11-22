#!/usr/bin/env python3
"""Deterministically signal a spanning native close_range between its parts (Linux/root).

A close_range from below the protected descriptors 240/241 to above them runs
as two calls: the caller's, ending at 239, and a replay from 242 that Ginux
sets up at the caller's exit stop. Trace only Ginux, never its children: Ginux
remains their ptracer. Stop it at the PTRACE_CONT that resumes the caller into
that replay and queue SIGUSR1 to the fixture first. close_range is not
interruptible on Linux, so the handler must run once, after the whole call:
it sees the workload's 250 closed, and a descriptor it opens stays open. No
test hook, host target, timing-based race, or replacement runtime is used.
"""
import ctypes
import os
from pathlib import Path
import shutil
import signal
import sys
import tempfile


class Regs(ctypes.Structure):
    _fields_ = [(name, ctypes.c_ulonglong) for name in (
        "r15 r14 r13 r12 rbp rbx r11 r10 r9 r8 rax rcx rdx rsi rdi orig_rax "
        "rip cs eflags rsp ss fs_base gs_base ds es fs gs").split()]


def main():
    if len(sys.argv) != 3 or os.geteuid() != 0:
        sys.exit("usage (as root): test-native-close-range-signal.py <ginux> <native fixture>")
    binary, fixture = (Path(arg).resolve() for arg in sys.argv[1:])
    template = (Path(__file__).resolve().parent.parent / "config/instance-gate5.toml").read_text()
    libc = ctypes.CDLL(None, use_errno=True)
    libc.ptrace.argtypes = [ctypes.c_uint, ctypes.c_uint, ctypes.c_void_p, ctypes.c_void_p]
    libc.ptrace.restype = ctypes.c_long

    def ptrace(request, pid, address=0, data=0):
        ctypes.set_errno(0)
        value = libc.ptrace(request, pid, address, data)
        if value == -1 and ctypes.get_errno():
            raise OSError(ctypes.get_errno(), f"ptrace {request}")
        return value

    def peek_regs(pid, address):
        values = [ptrace(2, pid, address + i * 8) & ((1 << 64) - 1) for i in range(27)]
        return Regs(*values)

    with tempfile.TemporaryDirectory(prefix="ginux-close-range-signal-") as directory:
        work = Path(directory)
        work.chmod(0o755)
        shutil.copy2(binary, work / "ginux")
        (work / "ginux").chmod(0o755)
        root = work / "root"
        (root / "usr/bin").mkdir(parents=True)
        shutil.copy2(fixture, root / "usr/bin/bash")
        (root / "usr/bin/bash").chmod(0o755)
        (root / "scenario").write_text("Q-close-range-signal")
        config = work / "instance.toml"
        config.write_text(template.replace('init = "/bin/bash"', 'init = "/usr/bin/bash"') + """
[contract.filesystem]
read = ["/"]
write = ["/"]
create = ["/"]
[contract.kernel]
proc_read = ["/"]
sys_read = ["/"]
""")
        read_fd, write_fd = os.pipe()
        pid = os.fork()
        if pid == 0:
            os.close(read_fd)
            os.dup2(write_fd, 1)
            os.dup2(write_fd, 2)
            os.close(write_fd)
            if libc.ptrace(0, 0, 0, 0) < 0:
                os._exit(125)
            os.kill(os.getpid(), signal.SIGSTOP)
            os.execv(work / "ginux", [str(work / "ginux"), "native", str(config), str(root), "configured"])
        os.close(write_fd)
        os.set_blocking(read_fd, False)
        output = bytearray()
        caller = None
        staged = []
        interrupted = False
        complete = False

        def drain():
            while True:
                try:
                    data = os.read(read_fd, 65536)
                except BlockingIOError:
                    break
                if not data:
                    break
                output.extend(data)

        def expired(_signal, _frame):
            raise TimeoutError("close_range signal fixture timed out")

        previous_alarm = signal.signal(signal.SIGALRM, expired)
        signal.setitimer(signal.ITIMER_REAL, 30)
        try:
            _, status = os.waitpid(pid, 0)
            if not os.WIFSTOPPED(status):
                raise RuntimeError("Ginux did not stop before exec")
            # TRACESYSGOOD, TRACEEXEC and EXITKILL; deliberately no TRACEFORK.
            ptrace(0x4200, pid, 0, 1 | 16 | (1 << 20))
            ptrace(24, pid)
            while True:
                _, status = os.waitpid(pid, 0)
                drain()
                if os.WIFEXITED(status) or os.WIFSIGNALED(status):
                    complete = True
                    if not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                        raise RuntimeError(f"Ginux failed: status={status}\n{output.decode(errors='replace')}")
                    break
                stopped = os.WSTOPSIG(status)
                if stopped == (signal.SIGTRAP | 0x80):
                    info = ctypes.create_string_buffer(128)
                    ptrace(0x420e, pid, len(info), ctypes.addressof(info))
                    if info.raw[0] == 1:  # syscall entry
                        regs = Regs()
                        ptrace(12, pid, 0, ctypes.addressof(regs))
                        if regs.orig_rax == 101 and regs.rdi == 13:  # PTRACE_SETREGS
                            values = peek_regs(pid, regs.r10)
                            # A close_range staged at a syscall instruction
                            # (rax holds its number) is an upper part.
                            if values.orig_rax == 436 and values.rax == 436:
                                caller = regs.rsi
                                staged.append((values.rdi, values.rsi, values.rdx))
                        elif (regs.orig_rax == 101 and regs.rdi == 7 and regs.rsi == caller
                                and len(staged) == 1 and not interrupted):  # PTRACE_CONT
                            # The caller is held at the lower part's exit
                            # stop; the signal is delivered on its way back
                            # to the staged replay.
                            os.kill(caller, signal.SIGUSR1)
                            interrupted = True
                delivered = 0 if stopped in (signal.SIGTRAP, signal.SIGTRAP | 0x80) else stopped
                ptrace(24, pid, 0, delivered)
            drain()
            fixture = [line for line in output.decode(errors="replace").splitlines()
                       if line.startswith("GINUX_FIXTURE: ")]
            expected = ["GINUX_FIXTURE: signal-close-range=0",
                        "GINUX_FIXTURE: signal-close-range-arguments-kept=1",
                        "GINUX_FIXTURE: signal-close-range-closed=1",
                        "GINUX_FIXTURE: signal-close-range-signals=1",
                        "GINUX_FIXTURE: signal-close-range-handler-250=-9",
                        "GINUX_FIXTURE: signal-close-range-handler-fd-open=1"]
            record = (b" first=3 last=239 upper-first=242 upper-last=300 result=pending\n"
                      in output)
            if not interrupted or staged != [(242, 300, 0)] or fixture != expected or not record:
                raise RuntimeError(f"signalled close_range: signal={interrupted} staged={staged} record={record}\n{output.decode(errors='replace')}")
            print("PASS native close_range: one upper part runs although a signal arrives between the parts")
            print("PASS native close_range: the handler runs after the call, which closed both sides")
        finally:
            signal.setitimer(signal.ITIMER_REAL, 0)
            signal.signal(signal.SIGALRM, previous_alarm)
            if not complete:
                # Kill only this fixture tree. Read identities while the traced
                # supervisor is still held, before its death releases tracing.
                children = Path(f"/proc/{pid}/task/{pid}/children")
                if children.exists():
                    for child in children.read_text().split():
                        try:
                            handle = os.pidfd_open(int(child))
                            signal.pidfd_send_signal(handle, signal.SIGKILL)
                            os.close(handle)
                        except ProcessLookupError:
                            pass
                os.kill(pid, signal.SIGKILL)
                os.waitpid(pid, 0)
            os.close(read_fd)


if __name__ == "__main__":
    main()
