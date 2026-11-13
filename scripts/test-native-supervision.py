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

        configured = work / "configured.toml"
        configured.write_text(config.read_text().replace('init = "/bin/bash"',
            'init = "/usr/bin/bash"') + """
[contract.filesystem]
read = ["/"]
write = ["/"]
create = ["/"]
[contract.kernel]
proc_read = ["/"]
sys_read = ["/"]
""")

        def run(scenario, instance=config, stdin=None, stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, **options):
            (root / "scenario").write_text(scenario)
            proc = subprocess.run([str(work / "ginux"), "native", str(instance), str(root)]
                                  + (["configured"] if instance == configured else []),
                                  stdin=stdin, stdout=stdout, stderr=stderr, timeout=20, **options)
            # Audit records and fixture output have separate writers. Merge
            # only after capture so a concurrent audit fragment cannot prefix
            # an otherwise complete fixture line.
            output = (proc.stdout or b"") + (proc.stderr or b"")
            return proc.returncode, output.decode(errors="replace")

        def check(name, ok, code, output):
            if not ok:
                raise RuntimeError(f"{name}: exit={code}\n{output}")
            print(f"PASS native {name}")

        # Host directories must not reach the chrooted workload: as a
        # standard stream the instance does not start, and every other
        # inherited descriptor is closed. A closed stream starts as before.
        outside = work / "outside"
        outside.mkdir()
        directory = os.open(outside, os.O_RDONLY | os.O_DIRECTORY)
        try:
            for stream, reason in (("stdin", "input"), ("stdout", "output"), ("stderr", "error")):
                code, out = run("fork", **{stream: directory})
                # With stdout a directory the refusal has nowhere to print.
                check(f"directory {stream} refused", code == 1
                      and "GINUX_FIXTURE" not in out
                      and (stream == "stdout" or f"standard {reason} is a directory" in out),
                      code, out)
            for number in (239, 242, 300):
                os.dup2(directory, number, inheritable=True)
            try:
                code, out = run("descriptors", pass_fds=(239, 242, 300))
            finally:
                for number in (239, 242, 300):
                    os.close(number)
            check("inherited descriptors closed", code == 0
                  and all(f"GINUX_FIXTURE: fd-{number}=-9\n" in out for number in (239, 242, 300))
                  and "GINUX_FIXTURE: fd-240=0\n" in out and "GINUX_FIXTURE: fd-241=0\n" in out,
                  code, out)
        finally:
            os.close(directory)
        path_directory = os.open(outside, os.O_PATH | os.O_DIRECTORY)
        try:
            code, out = run("fork", stdin=path_directory)
            check("O_PATH directory stdin refused", code == 1
                  and "standard input is a directory" in out, code, out)
        finally:
            os.close(path_directory)
        with (work / "regular").open("w+") as stream:
            code, out = run("fork", stdin=stream)
            check("regular stdin starts", code == 0
                  and "GINUX_FIXTURE: fork=-38\n" in out, code, out)
        code, out = run("fork", preexec_fn=lambda: os.close(0))
        check("closed stdin starts", code == 0 and "GINUX_FIXTURE: fork=-38\n" in out, code, out)

        # Without child supervision a host fork would run unsupervised.
        code, out = run("fork")
        check("fork denied", code == 0 and "GINUX_FIXTURE: fork=-38\n" in out
              and "unsupervised child ran" not in out and "GINUX_NATIVE_CHECK: PASS" in out, code, out)

        # fcntl cannot make the host kernel signal another process.
        code, out = run("guard")
        check("signal-driven I/O refused", code == 0
              and all(f"GINUX_FIXTURE: {name}=-1\n" in out
                      for name in ("setown", "setown-ex", "setsig", "async"))
              and "GINUX_FIXTURE: nonblock=0\n" in out, code, out)

        # Invalid FDs make the baseline safe: an admitted host ioctl returns
        # EBADF; the argument guard must instead return EPERM before dispatch.
        code, out = run("quiet-ioctl", configured)
        check("native ioctl signal routing refused", code == 0
              and all(f"GINUX_FIXTURE: {name}=-1\n" in out for name in
                      ("fioasync", "fiosetown", "siocspgrp", "high-fioasync"))
              and "GINUX_FIXTURE: tcgets=-9\n" in out, code, out)

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

        code, out = run("ysinfo")
        fixture = [line for line in out.splitlines() if line.startswith("GINUX_FIXTURE: ")]
        check("sysinfo is deterministic and private", code == 0 and fixture == [
            "GINUX_FIXTURE: sysinfo=0", "GINUX_FIXTURE: sysinfo-zero=1",
            "GINUX_FIXTURE: mem-unit=1", "GINUX_FIXTURE: sysinfo-null=-14"]
            and "GINUX_NATIVE_CHECK: PASS" in out, code, out)

        code, out = run("oversized")
        check("oversized I/O uses short transfers",
              code == 0 and "GINUX_FIXTURE: read=1048576\n" in out
              and "GINUX_FIXTURE: pread=1048576\n" in out
              and "GINUX_NATIVE_CHECK: PASS" in out, code, out)

        code, out = run("random")
        check("getrandom is a bounded short read",
              code == 0 and "GINUX_FIXTURE: getrandom=256\n" in out
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
        for scenario in ("sigchld-busy", "sigchld"):
            code, out = run(scenario, configured)
            fixture = [line for line in out.splitlines() if line.startswith("GINUX_FIXTURE: ")]
            expected = ["GINUX_FIXTURE: sigaction=0", "GINUX_FIXTURE: fork-ok=1"]
            if scenario == "sigchld-busy":
                expected.append("GINUX_FIXTURE: sigchld-userspace=1")
            expected += ["GINUX_FIXTURE: sigchld-poll=1", "GINUX_FIXTURE: sigchld-handled=1",
                         "GINUX_FIXTURE: wait-ok=1", "GINUX_FIXTURE: child-status=7"]
            check(f"SIGCHLD reaches a handler ({scenario})",
                  code == 0 and fixture == expected, code, out)
        # systemd 258+ waits for children through waitid(P_PIDFD), with no
        # fallback.
        code, out = run("pidfd-wait", configured)
        fixture = [line for line in out.splitlines() if line.startswith("GINUX_FIXTURE: ")]
        check("waitid(P_PIDFD) reaps a managed child", code == 0 and fixture == [
            "GINUX_FIXTURE: pidfd-open-ok=1", "GINUX_FIXTURE: waitid-self=-10",
            "GINUX_FIXTURE: waitid-not-pidfd=-9", "GINUX_FIXTURE: waitid-invalid=-9",
            "GINUX_FIXTURE: waitid-high=0", "GINUX_FIXTURE: waitid=0",
            "GINUX_FIXTURE: waited-child=1", "GINUX_FIXTURE: child-status=7"], code, out)
        # glibc 2.41+ posix_spawn starts children with clone3 CLONE_INTO_CGROUP.
        (root / "sys/fs/cgroup").mkdir(parents=True, exist_ok=True)
        code, out = run("cgroup-clone3", configured)
        fixture = [line for line in out.splitlines() if line.startswith("GINUX_FIXTURE: ")]
        check("clone3 CLONE_INTO_CGROUP starts a child", code == 0 and fixture == [
            "GINUX_FIXTURE: clone3-invalid=-9", "GINUX_FIXTURE: clone3-ordinary=-9",
            "GINUX_FIXTURE: cgroup-open-ok=1", "GINUX_FIXTURE: clone3-ok=1",
            "GINUX_FIXTURE: wait-ok=1", "GINUX_FIXTURE: child-status=7"], code, out)
        code, out = run("arguments", configured)
        fixture = [line for line in out.splitlines() if line.startswith("GINUX_FIXTURE: ")]
        check("immutable clone3 arguments and cleanup", code == 0
              and out.count("GINUX_POLICY_AUDIT: action=deny policy=clone-arguments result=-1") == 5
              and fixture == [
            "GINUX_FIXTURE: readonly-clone=1", "GINUX_FIXTURE: readonly-unchanged=1",
            "GINUX_FIXTURE: clone-zero-tail=1", "GINUX_FIXTURE: clone-pidfd=1", "GINUX_FIXTURE: pidfd-input=1",
            "GINUX_FIXTURE: clone-vfork=1", "GINUX_FIXTURE: vfork-unchanged=1",
            "GINUX_FIXTURE: clone-short=-22", "GINUX_FIXTURE: clone-large=-7",
            "GINUX_FIXTURE: clone-unreadable=-14", "GINUX_FIXTURE: clone-short-cgroup=-22",
            "GINUX_FIXTURE: clone-kernel-error=-22", "GINUX_FIXTURE: error-unchanged=1",
            "GINUX_FIXTURE: clone-tail=-7", "GINUX_FIXTURE: clone-version0=1",
            "GINUX_FIXTURE: snapshot-mmap=-1", "GINUX_FIXTURE: snapshot-mprotect=-1",
            "GINUX_FIXTURE: snapshot-munmap=-1", "GINUX_FIXTURE: snapshot-madvise=-1",
            "GINUX_FIXTURE: snapshot-mremap=-1", "GINUX_FIXTURE: snapshot-repeated=160"], code, out)
        # PIDFD_GET_INFO must not reach the host: it reports host pids.
        code, out = run("host-info", configured)
        fixture = [line for line in out.splitlines() if line.startswith("GINUX_FIXTURE: ")]
        check("PIDFD_GET_INFO is ENOTTY", code == 0 and fixture == [
            "GINUX_FIXTURE: pidfd-open-ok=1", "GINUX_FIXTURE: pidfd-info=-25",
            "GINUX_FIXTURE: pidfd-info-high=-25", "GINUX_FIXTURE: pidfd-info-invalid=-25",
            "GINUX_FIXTURE: unrelated-ioctl=-9", "GINUX_FIXTURE: wait-ok=1"], code, out)
    print("test-native-supervision: PASS")


if __name__ == "__main__":
    main()
