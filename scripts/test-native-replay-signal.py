#!/usr/bin/env python3
"""Deterministically signal a native replay between its stages (Linux/root).

Configured native mode runs a clone3 and a shared-network sendto as staged
calls: map a private read-only snapshot, call on the snapshot, unmap it. Each
next stage is set up at the previous stage's exit stop by rewinding the
caller to its syscall instruction. Trace only Ginux, never its children:
Ginux remains their ptracer. Queue a SIGUSR1, for which the fixture has a
handler, to the fixture at one of two points:

  into-call    the PTRACE_CONT that resumes the caller into the staged call
               (map -> call boundary)
  during-call  the PTRACE_SYSCALL that resumes the caller from the staged
               call's seccomp stop, so the signal is pending when the call
               ends (call -> unmap boundary; clone3 returns ERESTARTNOINTR)

Linux runs the handler once, with the caller's own registers, and the call
completes (clone3 is restarted after the handler). The control cases send no
signal. No test hook, host target, timing-based race, or replacement runtime
is used.
"""
import ctypes
import array
import os
from pathlib import Path
import re
import shutil
import signal
import socket
import struct
import sys
import tempfile

CLONE_BASE, CLONE_END = 0x600000000000, 0x600000080000
NETWORK_BASE, NETWORK_END = 0x610000000000, 0x610000200000
FILESYSTEM = """
[contract.filesystem]
read = ["/"]
write = ["/"]
create = ["/"]
[contract.kernel]
proc_read = ["/"]
sys_read = ["/"]
"""
NETWORK = '''[contract.network]
default = "deny"
inbound = ["udp:18081", "tcp:18083"]
outbound = ["udp:18081", "tcp:18083"]

[network]
backend = "shared"'''


def clone3_replay(regs):
    return (regs.orig_rax == 435 and regs.rax == 435 and regs.rsi == 88
            and CLONE_BASE <= regs.rdi < CLONE_END)


def sendto_replay(regs):
    return (regs.orig_rax == 44 and regs.rax == 44
            and NETWORK_BASE <= regs.rsi < NETWORK_END)


def close_replay(regs):
    return regs.orig_rax == 3 and regs.rax == 3


SCENARIOS = {
    "clone3": ("S-replay-signal-clone3", 435, clone3_replay, False, False,
               (b"GINUX_FIXTURE: replay-signal-clone3=1\n",
                b"GINUX_FIXTURE: replay-signal-child=1\n")),
    "send": ("V-replay-signal-send", 44, sendto_replay, True, False,
             (b"GINUX_FIXTURE: replay-signal-send=1\n",
              b"GINUX_FIXTURE: replay-signal-recv=1\n")),
    "send-eintr": ("E-replay-signal-send-eintr", 44, sendto_replay, True, True,
                   (b"GINUX_FIXTURE: replay-signal-ready=1\n",
                    b"GINUX_FIXTURE: replay-signal-blocking-send=-4\n")),
    "send-restart": ("H-replay-signal-send-restart", 44, sendto_replay, True, True,
                     (b"GINUX_FIXTURE: replay-signal-ready=1\n",
                      b"GINUX_FIXTURE: replay-signal-blocking-send=1\n")),
    "rights": ("J-inbound-rights-signal", 3, close_replay, True, True,
               (b"GINUX_FIXTURE: replay-signal-ready=1\n",
                b"GINUX_FIXTURE: inbound-rights-signal-recvmsg=-1\n",
                b"GINUX_FIXTURE: inbound-rights-signal-closed=1\n")),
}
STAGES = ("control", "into-call", "during-call")
DEFAULT_CASES = ([f"{scenario}:{stage}" for scenario in ("clone3", "send") for stage in STAGES]
                 + ["send-eintr:during-call", "send-restart:during-call",
                    "rights:cleanup"])


class Regs(ctypes.Structure):
    _fields_ = [(name, ctypes.c_ulonglong) for name in (
        "r15 r14 r13 r12 rbp rbx r11 r10 r9 r8 rax rcx rdx rsi rdi orig_rax "
        "rip cs eflags rsp ss fs_base gs_base ds es fs gs").split()]


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


def run(binary, fixture, scenario, stage):
    """Returns (passed, report)."""
    name, number, is_replay, network, needs_ready, expected = SCENARIOS[scenario]
    template = (Path(__file__).resolve().parent.parent / "config/instance-gate5.toml").read_text()
    template = template.replace('init = "/bin/bash"', 'init = "/usr/bin/bash"')
    if network:
        template = template.replace('[network]\nbackend = "none"', NETWORK)
    with tempfile.TemporaryDirectory(prefix="ginux-replay-signal-") as directory:
        work = Path(directory)
        work.chmod(0o755)
        shutil.copy2(binary, work / "ginux")
        (work / "ginux").chmod(0o755)
        root = work / "root"
        (root / "usr/bin").mkdir(parents=True)
        shutil.copy2(fixture, root / "usr/bin/bash")
        (root / "usr/bin/bash").chmod(0o755)
        (root / "scenario").write_text(name)
        rights_server = None
        rights_server_pid = None
        rights_fds = []
        if scenario == "rights":
            (root / "tmp").mkdir()
            regular = work / "regular"
            regular.write_bytes(b"regular")
            rights_path = root / "tmp/inbound-rights.sock"
            rights_server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            rights_server.bind(str(rights_path))
            rights_path.chmod(0o777)
            rights_server.listen(1)
            rights_server.settimeout(10)
            rights_fds = [os.open(regular, os.O_RDONLY), os.open("/dev/null", os.O_RDONLY)]

            rights_server_pid = os.fork()
            if rights_server_pid == 0:
                try:
                    connection, _ = rights_server.accept()
                    with connection:
                        connection.settimeout(10)
                        if connection.recv(1) != b"1":
                            raise RuntimeError("recvmsg rights handshake failed")
                        connection.sendmsg(
                            [b"x"],
                            [(socket.SOL_SOCKET, socket.SCM_RIGHTS,
                              array.array("i", rights_fds))])
                    os._exit(0)
                except BaseException:
                    os._exit(1)
            rights_server.close()
        config = work / "instance.toml"
        config.write_text(template + FILESYSTEM)
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
        staged = 0
        interrupted = False
        handler_view = None
        complete = False
        status = None

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
            raise TimeoutError(f"{scenario} {stage} timed out")

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
                    break
                stopped = os.WSTOPSIG(status)
                if stopped == (signal.SIGTRAP | 0x80):
                    info = ctypes.create_string_buffer(128)
                    ptrace(0x420e, pid, len(info), ctypes.addressof(info))
                    if info.raw[0] == 1:  # syscall entry
                        regs = Regs()
                        ptrace(12, pid, 0, ctypes.addressof(regs))
                        if regs.orig_rax == 101 and regs.rdi == 13:  # PTRACE_SETREGS
                            ready = b"GINUX_FIXTURE: replay-signal-ready=1\n" in output
                            if is_replay(peek_regs(pid, regs.r10)) and (not needs_ready or ready):
                                caller = regs.rsi
                                staged += 1
                        elif (stage in ("into-call", "cleanup")
                                and regs.orig_rax == 101 and regs.rdi == 7
                                and regs.rsi == caller and staged == 1 and not interrupted):
                            # PTRACE_CONT from the map stage's exit stop: the
                            # caller is rewound to the staged call.
                            os.kill(caller, signal.SIGUSR1)
                            interrupted = True
                        elif (stage == "during-call" and regs.orig_rax == 101 and regs.rdi == 24
                                and regs.rsi == caller and staged == 1 and not interrupted):
                            # PTRACE_SYSCALL from the staged call's seccomp
                            # stop: the signal is pending when it ends.
                            call = Path(f"/proc/{caller}/syscall").read_text().split()
                            if call[0] != str(number):
                                raise RuntimeError(f"caller is not at the staged call: {call}")
                            os.kill(caller, signal.SIGUSR1)
                            interrupted = True
                        elif (interrupted and handler_view is None and regs.orig_rax == 101
                                and regs.rdi == 12 and regs.rsi == caller):
                            # Ginux reads the caller's registers at a stop.
                            # Once the handler has run, record what it saw and
                            # which call the caller has stopped at.
                            match = re.search(rb"GINUX_FIXTURE: replay-signal-context=(\d+)\n", output)
                            if match:
                                with open(f"/proc/{caller}/mem", "rb") as memory:
                                    memory.seek(int(match[1]))
                                    view = struct.unpack("5Q", memory.read(40))
                                if view[0]:
                                    call = Path(f"/proc/{caller}/syscall").read_text().split()
                                    handler_view = (view, call)
                delivered = 0 if stopped in (signal.SIGTRAP, signal.SIGTRAP | 0x80) else stopped
                ptrace(24, pid, 0, delivered)
            drain()
        finally:
            signal.setitimer(signal.ITIMER_REAL, 0)
            signal.signal(signal.SIGALRM, previous_alarm)
            if not complete:
                os.kill(pid, signal.SIGKILL)
                os.waitpid(pid, 0)
            os.close(read_fd)

        if rights_server_pid is not None:
            _, rights_status = os.waitpid(rights_server_pid, 0)
            for descriptor in rights_fds:
                os.close(descriptor)
            if not os.WIFEXITED(rights_status) or os.WEXITSTATUS(rights_status) != 0:
                raise RuntimeError("inbound SCM_RIGHTS sender failed")

        handled = 0 if stage == "control" else 1
        wanted = expected + (f"GINUX_FIXTURE: replay-signal-handled={handled}\n".encode(),
                             b"GINUX_FIXTURE: replay-signal-handler-args-match=1\n")
        passed = (os.WIFEXITED(status) and os.WEXITSTATUS(status) == 0
                  and staged >= 1 and interrupted == (stage != "control")
                  and all(line in output for line in wanted))
        report = [f"ginux status: {'exit ' + str(os.WEXITSTATUS(status)) if os.WIFEXITED(status) else 'signal ' + str(os.WTERMSIG(status))}",
                  f"staged {scenario} replays: {staged}, signal sent: {interrupted}"]
        if handler_view:
            (count, rip, rax, rdi, rsi), call = handler_view
            report.append(f"handler ucontext: count={count} rip={rip:#x} rax={rax} rdi={rdi:#x} rsi={rsi:#x}")
            report.append(f"caller stopped at syscall {call[0]} when Ginux read its registers")
        report.append("output:")
        report.extend("  " + line for line in output.decode(errors="replace").splitlines())
        return passed, "\n".join(report)


def main():
    if len(sys.argv) < 3 or os.geteuid() != 0:
        sys.exit("usage (as root): test-native-replay-signal.py <ginux> <native fixture> "
                 "[clone3|send:control|into-call|during-call ...]")
    binary, fixture = (Path(arg).resolve() for arg in sys.argv[1:3])
    cases = sys.argv[3:] or DEFAULT_CASES
    failed = []
    for case in cases:
        scenario, stage = case.split(":")
        passed, report = run(binary, fixture, scenario, stage)
        print(f"{'PASS' if passed else 'FAIL'} native {scenario} replay, signal {stage}")
        if not passed or os.environ.get("VERBOSE"):
            print(report)
        if not passed:
            failed.append(case)
    if failed:
        sys.exit(f"failed: {' '.join(failed)}")


if __name__ == "__main__":
    main()
