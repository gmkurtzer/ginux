#!/usr/bin/env python3
"""Exercise process-native against existing native binaries."""
import ctypes
import os
from pathlib import Path
import signal
import struct
import subprocess
import sys
import tempfile

# Scenarios that also run with process_vm_writev unavailable, as on a kernel
# built without CONFIG_CROSS_MEMORY_ATTACH (ENOSYS) or under a seccomp profile
# that denies it (EPERM). The supervisor then falls back to ptrace writes,
# which still fail unmapped buffers with EFAULT but write read-only memory.
NO_VM_WRITEV = ("wait_status", "ro_status", "bad_status", "bad_rusage")
# ro_status on the ptrace fallback (reduced fidelity): the read-only word is
# written and the wait succeeds, as before process_vm_writev was used.
RO_STATUS_FALLBACK = [b"nohang wait=2", b"nohang ro cell=10752", b"nohang rusage=0", b"nohang rewait=-10",
                      b"blocking wait=3", b"blocking ro cell=10752", b"blocking rusage=0",
                      b"blocking rewait=-10"]
FALLBACK_AUDIT = b"tracee-write=ptrace-fallback"
NO_VM_WRITEV_ERRNO = {"/vm-writev-enosys": 38, "/vm-writev-eperm": 1}


def deny_process_vm_writev(errno):
    """Install a seccomp filter that fails process_vm_writev with errno."""
    libc = ctypes.CDLL(None, use_errno=True)
    # x86-64 only: load arch, allow others; load nr, errno for 311, else allow.
    program = [(0x20, 0, 0, 4), (0x15, 0, 3, 0xc000003e), (0x20, 0, 0, 0),
               (0x15, 0, 1, 311), (0x06, 0, 0, 0x00050000 | errno), (0x06, 0, 0, 0x7fff0000)]
    raw = ctypes.create_string_buffer(b"".join(struct.pack("HBBI", *op) for op in program))

    class SockFprog(ctypes.Structure):
        _fields_ = [("len", ctypes.c_ushort), ("filter", ctypes.c_void_p)]

    fprog = SockFprog(len(program), ctypes.addressof(raw))
    PR_SET_NO_NEW_PRIVS, PR_SET_SECCOMP, SECCOMP_MODE_FILTER = 38, 22, 2
    # prctl is variadic: pass every argument as a full unsigned long, since
    # PR_SET_NO_NEW_PRIVS rejects nonzero upper bits in the unused ones.
    ulong = ctypes.c_ulong
    if libc.prctl(PR_SET_NO_NEW_PRIVS, ulong(1), ulong(0), ulong(0), ulong(0)) or \
            libc.prctl(PR_SET_SECCOMP, ulong(SECCOMP_MODE_FILTER), ctypes.byref(fprog), ulong(0), ulong(0)):
        os._exit(125)


def main():
    if len(sys.argv) not in (4, 5):
        sys.exit("usage: test-process-native.py <ginux> <fixture directory> <legacy fixture> [scenario]")
    binary, fixtures, legacy = (str(Path(arg).resolve()) for arg in sys.argv[1:4])
    config = Path(__file__).resolve().parent.parent / "config/domain-gate7.toml"
    def wait_fault(status):
        # wait4 fails with EFAULT after reaping the child, whether it had
        # exited (nohang) or the wait blocked; a writable status is stored
        # before the rusage fault, a failed status store skips the rusage,
        # and memory around the faulting buffers stays untouched.
        return [path + line for path in (b"nohang", b"blocking")
                for line in (b" wait=-14", b" ro cell=0", b" edge=30583", b" status=" + status,
                             b" rusage=1515870810", b" rewait=-10")]

    expected = {
        "syscall_abi": [b"set_tid_address(218)=1", b"set_robust_list(273)=0", b"rseq(334)=-22"],
        "untraced_clone": [b"parent clone result=-38", b"parent wait result=-10"],
        "shared_pipe": [b"parent shared pipe read=1"],
        "wait_status": [b"unaligned wait result=2", b"status byte 7=0", b"status byte 8=42"],
        "thread_reuse": [b"sequential thread=33"],
        "denied_host_memory": [b"process_vm_writev(311)=-38"],
        "inherited_fds": [b"inherited read=-9", b"dup inherited fd=-9", b"read alias=-9", b"inherited close=-9"],
        "thread_exec": [b"exec-success", b"exec-complete virtual-pid=1"],
        "unused_parent_tid": [b"no-parent-settid thread=2"],
        "exec_failure": [b"thread exec result=-2", b"parent resumed=1"],
        "copied_files": [b"child close=0", b"parent pipe write=1"],
        "shared_exec": [b"shared exec wait=2", b"parent pipe write=1"],
        "clone_tid": [b"parent tid cell=2", b"child tid cell=2", b"child gettid=2", b"clone return=2", b"clone wait=2"],
        "child_signal": [b"signal wait=2", b"signal status=15"],
        "nonblocking_wait": [b"nonblocking wait=0", b"blocking wait=2"],
        "concurrent_wait": [b"concurrent wait result=2", b"concurrent wait result=-10"],
        "remaining_wait": [b"concurrent wait result=2", b"concurrent wait result=3"],
        "vfork": [b"vfork return=2", b"vfork wait=2", b"vfork status=10752"],
        "sigpipe": [b"sigpipe wait=2", b"sigpipe status=13"],
        "segv": [b"segv wait=2", b"segv status=11"],
        "kill_badsig": [b"kill signal 65=-22", b"kill signal -1=-22", b"kill missing pid signal 65=-3"],
        "thread_segv": [b"thread segv wait=2", b"thread segv status=11"],
        "thread_term": [b"thread term kill=0", b"thread term wait=2", b"thread term status=15"],
        "parked_kill": [b"parked kill wait=2", b"parked kill status=11"],
        "parked_term": [b"parked term kill=0", b"parked term wait=2", b"parked term status=15"],
        "orphan_reparent": [b"orphan middle wait=2", b"orphan getppid=1", b"orphan wait=3",
                            b"orphan status=1792"],
        "orphan_zombie": [b"orphan middle wait=2", b"orphan wait=3", b"orphan status=1792"],
        "ro_status": wait_fault(b"305419896"),
        "bad_status": wait_fault(b"305419896"),
        "bad_rusage": wait_fault(b"10752"),
        "straddle_status": wait_fault(b"305419896"),
        "alias_writes": [b"plain stdin write=-38", b"dup stdin write=-38", b"dup alias write=-38",
                         b"dup2 stdin write=-38", b"dup3 stdin write=-38", b"fcntl dupfd stdin write=-38",
                         b"fcntl dupfd-cloexec stdin write=-38", b"stdin over stdout write=-38",
                         b"stdout alias write=16", b"plain stdout write 4097=-38",
                         b"stdout alias write 4097=-38", b"restored stdout write 4097=-38",
                         b"stderr alias write=16", b"stderr alias write 4097=-38"],
    }
    for suffix, errno in NO_VM_WRITEV_ERRNO.items():
        for name in NO_VM_WRITEV:
            base = RO_STATUS_FALLBACK if name == "ro_status" else expected[name]
            expected[name + suffix] = [*base, FALLBACK_AUDIT + b" process_vm_writev=-%d" % errno]
    with tempfile.TemporaryDirectory(prefix="ginux-process-") as directory:
        sentinel = Path(directory) / "sentinel"
        sentinel.write_bytes(b"controlled sentinel\n")
        fd = os.open(sentinel, os.O_RDONLY)
        os.dup2(fd, 9)
        if fd != 9:
            os.close(fd)
        try:
            allowed_cpus = sorted(os.sched_getaffinity(0))
            selected = sys.argv[4:] or [*expected, "legacy"]
            for mode in selected:
                if mode not in expected and mode != "legacy":
                    raise RuntimeError(f"unknown scenario {mode}")
                scenario, _, variant = mode.partition("/")
                denied = NO_VM_WRITEV_ERRNO.get("/" + variant) if variant else None
                path = legacy if mode == "legacy" else f"{fixtures}/{scenario}"
                repetitions = 100 if mode == "legacy" else 20 if scenario in (
                    "thread_exec", "exec_failure", "thread_reuse", "clone_tid", "vfork",
                    "concurrent_wait", "remaining_wait", "sigpipe", "segv", "thread_segv",
                    "thread_term", "parked_kill", "parked_term", "ro_status", "bad_status", "bad_rusage",
                    "straddle_status", "orphan_reparent", "orphan_zombie"
                ) else 1
                deferred_stops = 0
                for run in range(repetitions):
                    # Exercise both child-first and parent-first ptrace event
                    # ordering under different amounts of CPU concurrency.
                    affinity = (allowed_cpus, allowed_cpus[:1], allowed_cpus[:2])[run % 3]
                    # A writable host file as stdin shows whether any write
                    # through a stdin alias reaches the host.
                    stdin_sink = Path(directory) / "stdin-sink"
                    stdin = os.open(stdin_sink, os.O_RDWR | os.O_CREAT | os.O_TRUNC) \
                        if mode == "alias_writes" else None
                    try:
                        proc = subprocess.Popen([binary, "process-native", str(config), path],
                                                stdin=stdin, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                                pass_fds=(9,), start_new_session=True,
                                                preexec_fn=lambda: (os.sched_setaffinity(0, affinity),
                                                                    denied and deny_process_vm_writev(denied)))
                    finally:
                        if stdin is not None:
                            os.close(stdin)
                    try:
                        out, err = proc.communicate(timeout=15)
                    except subprocess.TimeoutExpired:
                        os.killpg(proc.pid, signal.SIGKILL)
                        proc.communicate()
                        raise RuntimeError(f"process scenario {mode} hung")
                    if stdin is not None and stdin_sink.read_bytes():
                        raise RuntimeError(f"scenario {mode} wrote to the host stdin: "
                                           f"{stdin_sink.read_bytes()!r}\n{out!r}")
                    # A wrapped mode must take the fallback. A plain mode takes
                    # it only where the supervisor finds process_vm_writev
                    # unavailable, which its audit line reports; ro_status
                    # then has the reduced-fidelity result.
                    fallback = FALLBACK_AUDIT in out
                    if denied and not fallback:
                        raise RuntimeError(f"scenario {mode} did not use the ptrace fallback: {out!r}")
                    required = expected.get(mode, [b"GINUX_NATIVE_PARENT: PASS"])
                    if mode == "ro_status" and fallback:
                        required = RO_STATUS_FALLBACK
                    required = [*required, b"GATE7_NATIVE_PROCESS: PASS", b"descriptors live-pipes=0"]
                    if proc.returncode or any(item not in out for item in required):
                        raise RuntimeError(f"scenario {mode}: exit={proc.returncode}\n{out!r}\n{err!r}")
                    if b"untraced child" in out or b"controlled sentinel" in out:
                        raise RuntimeError(f"scenario {mode} escaped supervision: {out!r}")
                    if mode in ("concurrent_wait", "remaining_wait") and out.count(b"wait parked caller=") != 2:
                        raise RuntimeError(f"scenario {mode} did not exercise two parked waiters: {out!r}")
                    if (scenario in ("bad_status", "bad_rusage", "straddle_status")
                            or scenario == "ro_status" and not fallback) and (
                            out.count(b"wait store fault caller=1") != 2 or out.count(b"wait parked caller=1") != 1):
                        raise RuntimeError(f"scenario {mode} did not fault both the direct and the parked wait: {out!r}")
                    deferred = out.count(b"newborn-stop=deferred")
                    if deferred != out.count(b"newborn-stop=registered"):
                        raise RuntimeError(f"scenario {mode} left a child stop unregistered: {out!r}")
                    deferred_stops += deferred
                print(f"PASS process scenario {mode} ({repetitions} run(s), "
                      f"{deferred_stops} early child stop(s))")
        finally:
            os.close(9)
    print("test-process-native: PASS")


if __name__ == "__main__":
    main()
