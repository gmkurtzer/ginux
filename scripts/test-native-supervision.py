#!/usr/bin/env python3
"""Check `ginux native` supervision edges with a fixture as /usr/bin/bash (Linux/root)."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def main():
    if len(sys.argv) != 3 or os.geteuid() != 0:
        sys.exit("usage (as root): test-native-supervision.py <ginux binary> <native fixture>")
    binary, fixture = (str(Path(arg).resolve()) for arg in sys.argv[1:])
    config = Path(__file__).resolve().parent.parent / "config/instance-gate5.toml"
    with tempfile.TemporaryDirectory(prefix="ginux-native-") as directory:
        work = Path(directory)
        # The tracee drops to UID 1000 and must be able to execute the copies.
        shutil.copyfile(binary, work / "ginux")
        (work / "ginux").chmod(0o755)
        root = work / "root"
        (root / "usr/bin").mkdir(parents=True)
        shutil.copyfile(fixture, root / "usr/bin/bash")
        (root / "usr/bin/bash").chmod(0o755)
        (root / "large").write_bytes(b"r" * (1024 * 1024 + 1))
        work.chmod(0o755)

        def run(scenario):
            (root / "scenario").write_text(scenario)
            proc = subprocess.run([str(work / "ginux"), "native", str(config), str(root)],
                                  stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=20)
            return proc.returncode, proc.stdout.decode(errors="replace")

        def check(name, ok, code, output):
            if not ok:
                raise RuntimeError(f"{name}: exit={code}\n{output}")
            print(f"PASS native {name}")

        # Without child supervision a host fork would run unsupervised.
        code, out = run("fork")
        check("fork denied", code == 0 and "GINUX_FIXTURE: fork=-38\n" in out
              and "unsupervised child ran" not in out and "GINUX_NATIVE_CHECK: PASS" in out, code, out)

        # The thread identity matches the virtual pid: set_tid_address returns
        # it instead of the host tid, and PI futexes, which would store the
        # host tid in the futex word, are denied.
        code, out = run("virtual-tid")
        fixture = [line for line in out.splitlines() if line.startswith("GINUX_FIXTURE: ")]
        check("thread identity is virtual", code == 0 and fixture == [
            "GINUX_FIXTURE: set_tid_address=4242", "GINUX_FIXTURE: getpid=4242",
            "GINUX_FIXTURE: gettid=-38", "GINUX_FIXTURE: futex_lock_pi=-1",
            "GINUX_FIXTURE: pi_word=0", "GINUX_FIXTURE: futex_lock_pi_flags=-1",
            "GINUX_FIXTURE: pi_flags_word=0"] and "GINUX_NATIVE_CHECK: PASS" in out, code, out)

        code, out = run("oversized")
        check("oversized I/O uses short transfers",
              code == 0 and "GINUX_FIXTURE: read=1048576\n" in out
              and "GINUX_FIXTURE: pread=1048576\n" in out
              and "GINUX_NATIVE_CHECK: PASS" in out, code, out)

        # A plain SIGTRAP stop is not a syscall stop; supervision must fail
        # closed instead of treating the next syscall entry as an exit.
        code, out = run("trap")
        check("signal stop fails closed", code == 1 and "GINUX_FIXTURE: uname-before=-38\n" in out
              and "uname-after" not in out
              and "GINUX_INSTANCE: failed reason=unexpected native trace stop" in out, code, out)
        # int 0x80 selects the i386 table; supervision must stop instead of
        # classifying i386 fork (2) as x86-64 open. Without IA32 emulation the
        # host raises SIGSEGV on int 0x80, which ends supervision as an
        # unexpected stop instead.
        code, out = run("int80")
        check("i386 syscall ABI fails closed", code == 1 and "unsupervised child ran" not in out
              and "int80-fork" not in out
              and ("GINUX_INSTANCE: failed reason=native requires the x86-64 syscall ABI" in out
                   or "GINUX_INSTANCE: failed reason=unexpected native trace stop" in out),
              code, out)
    print("test-native-supervision: PASS")


if __name__ == "__main__":
    main()
