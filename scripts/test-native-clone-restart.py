#!/usr/bin/env python3
"""Deterministically interrupt the replayed native clone3 with a signal (Linux/root).

Trace only Ginux, never its children: Ginux remains their ptracer. Stop it at
the PTRACE_SYSCALL that resumes its private clone3 replay from the seccomp
stop and queue an ignored SIGUSR1 to the fixture first. The kernel abandons
that clone3 with ERESTARTNOINTR before creating a child; natively the caller
never sees that value because the call is reissued. No test hook, host
target, timing-based race, or replacement runtime is used.
"""
import ctypes
import os
from pathlib import Path
import shutil
import signal
import sys
import tempfile

SNAPSHOT_BASE, SNAPSHOT_END = 0x600000000000, 0x600000080000


class Regs(ctypes.Structure):
    _fields_ = [(name, ctypes.c_ulonglong) for name in (
        "r15 r14 r13 r12 rbp rbx r11 r10 r9 r8 rax rcx rdx rsi rdi orig_rax "
        "rip cs eflags rsp ss fs_base gs_base ds es fs gs").split()]


def main():
    if len(sys.argv) != 3 or os.geteuid() != 0:
        sys.exit("usage (as root): test-native-clone-restart.py <ginux> <native fixture>")
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

    with tempfile.TemporaryDirectory(prefix="ginux-clone-restart-") as directory:
        work = Path(directory)
        work.chmod(0o755)
        shutil.copy2(binary, work / "ginux")
        (work / "ginux").chmod(0o755)
        root = work / "root"
        (root / "usr/bin").mkdir(parents=True)
        shutil.copy2(fixture, root / "usr/bin/bash")
        (root / "usr/bin/bash").chmod(0o755)
        (root / "scenario").write_text("x-clone3-restart")
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
        replays = 0
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
            raise TimeoutError("clone restart fixture timed out")

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
                            staged = peek_regs(pid, regs.r10)
                            if (staged.orig_rax == 435 and staged.rsi == 88
                                    and SNAPSHOT_BASE <= staged.rdi < SNAPSHOT_END):
                                caller = regs.rsi
                                replays += 1
                        elif (regs.orig_rax == 101 and regs.rdi == 24 and regs.rsi == caller
                                and replays == 1 and not interrupted):  # PTRACE_SYSCALL
                            # The caller is held at the seccomp stop of the
                            # replay; the signal stays pending into clone3.
                            call = Path(f"/proc/{caller}/syscall").read_text().split()
                            if call[0] != "435" or not SNAPSHOT_BASE <= int(call[1], 16) < SNAPSHOT_END:
                                raise RuntimeError(f"caller is not at the clone3 replay: {call}")
                            os.kill(caller, signal.SIGUSR1)
                            interrupted = True
                delivered = 0 if stopped in (signal.SIGTRAP, signal.SIGTRAP | 0x80) else stopped
                ptrace(24, pid, 0, delivered)
            drain()
            if (not interrupted
                    or b"GINUX_FIXTURE: restart-clone3=1\n" not in output
                    or b"GINUX_FIXTURE: restart-child=1\n" not in output
                    or replays != 2):
                raise RuntimeError(f"interrupted clone3 not reissued: signal={interrupted} replays={replays}\n{output.decode(errors='replace')}")
            print("PASS native clone3: replay interrupted by a signal is reissued from a fresh snapshot")
            print("PASS native clone3: caller sees the reissued child, never ERESTARTNOINTR")
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
