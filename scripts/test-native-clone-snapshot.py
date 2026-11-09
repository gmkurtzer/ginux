#!/usr/bin/env python3
"""Deterministically mutate owned clone arguments after production capture (Linux/root).

Trace only Ginux, never its children: Ginux remains their ptracer. Stop it at
its real SETREGS entry for the captured clone replay, then mutate the original
fixture structure and verify that the private snapshot is read-only. No test
hook, host target, timing-based race, or replacement runtime is used.
"""
import ctypes
import errno
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile


class Regs(ctypes.Structure):
    _fields_ = [(name, ctypes.c_ulonglong) for name in (
        "r15 r14 r13 r12 rbp rbx r11 r10 r9 r8 rax rcx rdx rsi rdi orig_rax "
        "rip cs eflags rsp ss fs_base gs_base ds es fs gs").split()]


class Iovec(ctypes.Structure):
    _fields_ = [("base", ctypes.c_void_p), ("length", ctypes.c_size_t)]


def main():
    if len(sys.argv) != 4 or os.geteuid() != 0:
        sys.exit("usage (as root): test-native-clone-snapshot.py <ginux> <native fixture> <collision fixture>")
    binary, fixture, collision = (Path(arg).resolve() for arg in sys.argv[1:])
    template = (Path(__file__).resolve().parent.parent / "config/instance-gate5.toml").read_text()
    libc = ctypes.CDLL(None, use_errno=True)
    libc.ptrace.argtypes = [ctypes.c_uint, ctypes.c_uint, ctypes.c_void_p, ctypes.c_void_p]
    libc.ptrace.restype = ctypes.c_long
    libc.process_vm_writev.argtypes = [ctypes.c_int, ctypes.POINTER(Iovec), ctypes.c_ulong,
                                     ctypes.POINTER(Iovec), ctypes.c_ulong, ctypes.c_ulong]
    libc.process_vm_writev.restype = ctypes.c_ssize_t

    def ptrace(request, pid, address=0, data=0):
        ctypes.set_errno(0)
        value = libc.ptrace(request, pid, address, data)
        if value == -1 and ctypes.get_errno():
            raise OSError(ctypes.get_errno(), f"ptrace {request}")
        return value

    def peek_regs(pid, address):
        values = [ptrace(2, pid, address + i * 8) & ((1 << 64) - 1) for i in range(27)]
        return Regs(*values)

    def store(pid, address, value):
        data = ctypes.c_ulonglong(value)
        local, remote = Iovec(ctypes.addressof(data), 8), Iovec(address, 8)
        ctypes.set_errno(0)
        size = libc.process_vm_writev(pid, ctypes.byref(local), 1, ctypes.byref(remote), 1, 0)
        return size, ctypes.get_errno()

    def no_snapshot(pid):
        for line in Path(f"/proc/{pid}/maps").read_text().splitlines():
            start, end = (int(value, 16) for value in line.split()[0].split("-"))
            if start < 0x600000080000 and end > 0x600000000000:
                raise RuntimeError(f"snapshot mapping retained after cleanup: {line}")

    with tempfile.TemporaryDirectory(prefix="ginux-clone-snapshot-") as directory:
        work = Path(directory)
        work.chmod(0o755)
        shutil.copy2(binary, work / "ginux")
        (work / "ginux").chmod(0o755)
        root = work / "root"
        (root / "usr/bin").mkdir(parents=True)
        shutil.copy2(fixture, root / "usr/bin/bash")
        (root / "usr/bin/bash").chmod(0o755)
        (root / "scenario").write_text("user-mutation")
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
        mutated = False
        parent_pid = None
        parent_cleanup = child_cleanup = False
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
            raise TimeoutError("clone snapshot fixture timed out")

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
                        if regs.orig_rax == 101 and regs.rdi == 13:
                            staged = peek_regs(pid, regs.r10)
                            match = re.search(rb"GINUX_FIXTURE: race-args=(\d+)\n", output)
                            if match and staged.orig_rax == 435:
                                original = int(match[1])
                                if not mutated and staged.rdi == 0x600000000000 and staged.rsi == 88 and staged.rax == 435:
                                    parent_pid = regs.rsi
                                    size, error = store(parent_pid, staged.rdi, 0)
                                    if size != -1 or error != errno.EFAULT:
                                        raise RuntimeError(f"snapshot writable: result={size}, errno={error}")
                                    size, error = store(parent_pid, original, 0x8000000200000000)
                                    if size != 8:
                                        raise RuntimeError(f"cannot mutate owned caller input: {size}, errno={error}")
                                    mutated = True
                                elif mutated and staged.rdi == original and staged.rsi == 88:
                                    if staged.rax == 0 and regs.rsi != parent_pid:
                                        no_snapshot(regs.rsi)
                                        child_cleanup = True
                                    elif 0 < staged.rax < 128 and regs.rsi == parent_pid:
                                        no_snapshot(regs.rsi)
                                        parent_cleanup = True
                delivered = 0 if stopped in (signal.SIGTRAP, signal.SIGTRAP | 0x80) else stopped
                ptrace(24, pid, 0, delivered)
            drain()
            if (not mutated or not parent_cleanup or not child_cleanup
                    or b"GINUX_FIXTURE: race-child=1\n" not in output
                    or b"GINUX_FIXTURE: race-caller-mutation=1\n" not in output):
                raise RuntimeError(f"incomplete snapshot control: mutation={mutated} parent={parent_cleanup} child={child_cleanup}\n{output.decode(errors='replace')}")
            print("PASS native clone3: original arguments mutated after capture; private snapshot rejects writes")
            print("PASS native clone3: parent and child mappings removed; captured call succeeds; caller mutation preserved")
            shutil.copy2(collision, root / "usr/bin/bash")
            (root / "scenario").write_text("keep-existing")
            control = subprocess.run([str(work / "ginux"), "native", str(config), str(root), "configured"],
                                     capture_output=True, timeout=20)
            lines = [line for line in control.stdout.splitlines() if line.startswith(b"GINUX_FIXTURE:")]
            if control.returncode != 0 or lines != [b"GINUX_FIXTURE: snapshot-collision=-17",
                    b"GINUX_FIXTURE: collision-input=1", b"GINUX_FIXTURE: collision-mapping=1"]:
                raise RuntimeError(f"snapshot collision corrupted workload state: {control!r}")
            print("PASS native clone3: pre-existing mapping and caller arguments preserved on allocation failure")
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
