#!/usr/bin/env python3
"""Mutate a send destination after Ginux captures it (Linux/root)."""
import ctypes
import errno
import os
from pathlib import Path
import re
import shutil
import signal
import sys
import tempfile


class Regs(ctypes.Structure):
    _fields_ = [(name, ctypes.c_ulonglong) for name in (
        "r15 r14 r13 r12 rbp rbx r11 r10 r9 r8 rax rcx rdx rsi rdi orig_rax "
        "rip cs eflags rsp ss fs_base gs_base ds es fs gs").split()]


class Iovec(ctypes.Structure):
    _fields_ = [("base", ctypes.c_void_p), ("length", ctypes.c_size_t)]


def main():
    if len(sys.argv) != 3 or os.geteuid() != 0:
        sys.exit("usage (as root): test-native-network-snapshot.py <ginux> <native fixture>")
    binary, fixture = (Path(arg).resolve() for arg in sys.argv[1:])
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
        values = [ptrace(2, pid, address + index * 8) & ((1 << 64) - 1)
                  for index in range(27)]
        return Regs(*values)

    def store(pid, address, value, size=8):
        data = ctypes.c_ulonglong(value)
        local, remote = Iovec(ctypes.addressof(data), size), Iovec(address, size)
        ctypes.set_errno(0)
        result = libc.process_vm_writev(pid, ctypes.byref(local), 1,
                                       ctypes.byref(remote), 1, 0)
        return result, ctypes.get_errno()

    with tempfile.TemporaryDirectory(prefix="ginux-network-snapshot-") as directory:
        work = Path(directory)
        work.chmod(0o755)
        shutil.copy2(binary, work / "ginux")
        (work / "ginux").chmod(0o755)
        root = work / "root"
        (root / "usr/bin").mkdir(parents=True)
        shutil.copy2(fixture, root / "usr/bin/bash")
        (root / "usr/bin/bash").chmod(0o755)
        (root / "scenario").write_text("Race-network")
        config = work / "instance.toml"
        config.write_text(template.replace('init = "/bin/bash"', 'init = "/usr/bin/bash"')
                          .replace('[network]\nbackend = "none"', '''[contract.network]
default = "deny"
inbound = ["udp:18081"]
outbound = ["udp:18081"]

[network]
backend = "shared"''') + """
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
            os.execv(work / "ginux", [str(work / "ginux"), "native", str(config),
                                       str(root), "configured"])
        os.close(write_fd)
        os.set_blocking(read_fd, False)
        output = bytearray()
        mutated = False
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
            raise TimeoutError("network snapshot fixture timed out")

        previous_alarm = signal.signal(signal.SIGALRM, expired)
        signal.setitimer(signal.ITIMER_REAL, 30)
        try:
            _, status = os.waitpid(pid, 0)
            if not os.WIFSTOPPED(status):
                raise RuntimeError("Ginux did not stop before exec")
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
                    if info.raw[0] == 1:
                        regs = Regs()
                        ptrace(12, pid, 0, ctypes.addressof(regs))
                        if regs.orig_rax == 101 and regs.rdi == 13:
                            staged = peek_regs(pid, regs.r10)
                            match = re.search(rb"GINUX_FIXTURE: network-race-address=(\d+)\n", output)
                            if (match and not mutated and staged.orig_rax == 44
                                    and 0x610000000000 <= staged.r8 < 0x610000200000):
                                tracee = regs.rsi
                                original = int(match[1])
                                result, error = store(tracee, staged.r8, 0, 2)
                                if result != -1 or error != errno.EFAULT:
                                    raise RuntimeError(
                                        f"network snapshot writable: result={result}, errno={error}")
                                result, error = store(tracee, original + 2, 0xA246, 2)
                                if result != 2:
                                    raise RuntimeError(
                                        f"cannot mutate caller destination: result={result}, errno={error}")
                                mutated = True
                delivered = 0 if stopped in (signal.SIGTRAP, signal.SIGTRAP | 0x80) else stopped
                ptrace(24, pid, 0, delivered)
            drain()
            expected = (b"GINUX_FIXTURE: network-race-send=1\n",
                        b"GINUX_FIXTURE: network-race-recv=1\n",
                        b"GINUX_FIXTURE: network-race-mutated=1\n")
            if not mutated or any(line not in output for line in expected):
                raise RuntimeError(
                    f"incomplete network snapshot control: mutation={mutated}\n"
                    f"{output.decode(errors='replace')}")
            for line in Path(f"/proc/{pid}/maps").read_text().splitlines() if not complete else ():
                start, end = (int(value, 16) for value in line.split()[0].split("-"))
                if start < 0x610000200000 and end > 0x610000000000:
                    raise RuntimeError(f"network snapshot retained: {line}")
            print("PASS native network: caller destination mutated after capture; read-only snapshot used")
        finally:
            signal.setitimer(signal.ITIMER_REAL, 0)
            signal.signal(signal.SIGALRM, previous_alarm)
            if not complete:
                os.kill(pid, signal.SIGKILL)
                os.waitpid(pid, 0)
            os.close(read_fd)


if __name__ == "__main__":
    main()
