#!/usr/bin/env python3
"""Exercise process-native against existing native binaries."""
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile


def main():
    if len(sys.argv) not in (4, 5):
        sys.exit("usage: test-process-native.py <ginux> <fixture directory> <legacy fixture> [scenario]")
    binary, fixtures, legacy = (str(Path(arg).resolve()) for arg in sys.argv[1:4])
    config = Path(__file__).resolve().parent.parent / "config/domain-gate7.toml"
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
    }
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
                path = legacy if mode == "legacy" else f"{fixtures}/{mode}"
                repetitions = 100 if mode == "legacy" else 20 if mode in (
                    "thread_exec", "exec_failure", "thread_reuse", "clone_tid", "vfork",
                    "concurrent_wait", "remaining_wait", "sigpipe", "segv", "thread_segv",
                    "thread_term", "parked_kill"
                ) else 1
                deferred_stops = 0
                for run in range(repetitions):
                    # Exercise both child-first and parent-first ptrace event
                    # ordering under different amounts of CPU concurrency.
                    affinity = (allowed_cpus, allowed_cpus[:1], allowed_cpus[:2])[run % 3]
                    proc = subprocess.Popen([binary, "process-native", str(config), path],
                                            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                            pass_fds=(9,), start_new_session=True,
                                            preexec_fn=lambda: os.sched_setaffinity(0, affinity))
                    try:
                        out, err = proc.communicate(timeout=15)
                    except subprocess.TimeoutExpired:
                        os.killpg(proc.pid, signal.SIGKILL)
                        proc.communicate()
                        raise RuntimeError(f"process scenario {mode} hung")
                    required = expected.get(mode, [b"GINUX_NATIVE_PARENT: PASS"])
                    required = [*required, b"GATE7_NATIVE_PROCESS: PASS", b"descriptors live-pipes=0"]
                    if proc.returncode or any(item not in out for item in required):
                        raise RuntimeError(f"scenario {mode}: exit={proc.returncode}\n{out!r}\n{err!r}")
                    if b"untraced child" in out or b"controlled sentinel" in out:
                        raise RuntimeError(f"scenario {mode} escaped supervision: {out!r}")
                    if mode in ("concurrent_wait", "remaining_wait") and out.count(b"wait parked caller=") != 2:
                        raise RuntimeError(f"scenario {mode} did not exercise two parked waiters: {out!r}")
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
