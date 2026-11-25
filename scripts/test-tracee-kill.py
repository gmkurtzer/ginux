#!/usr/bin/env python3
"""Check that the supervisors end their tracee without PTRACE_KILL (Linux/root).

A seccomp filter from deny-ptrace fails ptrace requests of the supervisor with
EPERM: every request except PTRACE_TRACEME, so `native` and `process-native`
fail their ptrace setup and must still kill the stopped tracee, or only
PTRACE_KILL, so a post-setup native failure and Gate 1 must still end their
tracee. A supervisor without CAP_KILL (dropped from the bounding set) cannot
signal the native tracee after it drops to UID 65534 and must still end it
through its tracer relationship. Each run gets a private mount namespace with a temporary
directory bound over /var/tmp, so the host's /var/tmp is not touched.
"""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

PTRACE_KILL = 8
UNUSED_REQUEST = 0x4fff  # no such request: the filter fails nothing

RUN = 'mount --bind "$1" /var/tmp && shift && exec "$@"'


def main():
    if len(sys.argv) != 6 or os.geteuid() != 0:
        sys.exit("usage (as root): test-tracee-kill.py <ginux binary> <deny-ptrace> <payload> "
                 "<managed-payload> <native-fixture>")
    binary, deny, payload, managed, native = (
        str(Path(arg).resolve()) for arg in sys.argv[1:]
    )
    configs = Path(__file__).resolve().parent.parent / "config"

    def install(source, target):
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        target.chmod(0o755)

    def run(request, command, program, prefix, scenario=None):
        with tempfile.TemporaryDirectory(prefix="ginux-kill-") as directory:
            work = Path(directory)
            # Bound over /var/tmp: the marker of a process-native payload and
            # the Gate 1 payload path. The native tracee drops to UID 65534 in
            # its chroot and records into root/var/tmp.
            work.chmod(0o755)
            root = work / "root"
            (root / "var/tmp").mkdir(parents=True)
            (root / "var/tmp").chmod(0o1777)
            install(program, root / "usr/bin/bash")
            if scenario is not None:
                (root / "scenario").write_text(scenario)
            install(program, work / "payload")
            install(program, work / "ginux-managed-payload")
            arguments = {
                "native": ["native", str(configs / "instance-gate5.toml"), str(root)],
                "process-native": ["process-native", str(configs / "instance-gate7.toml"),
                                   "/var/tmp/payload"],
                "gate1": [],
            }[command]
            try:
                proc = subprocess.run(["unshare", "-m", "sh", "-c", RUN, "sh", str(work),
                                       *prefix, deny, str(request), binary, *arguments],
                                      capture_output=True, timeout=10)
                code = proc.returncode
                output = proc.stdout + proc.stderr
            except subprocess.TimeoutExpired as error:
                code = "timeout"
                output = (error.stdout or b"") + (error.stderr or b"")
            output = output.decode(errors="replace")
            # A detached tracee keeps running after the supervisor is gone.
            markers = [work / "gate1-payload-ran", root / "var/tmp/gate1-payload-ran"]
            deadline = time.monotonic() + 2
            while not any(m.exists() for m in markers) and time.monotonic() < deadline:
                time.sleep(0.05)
            return code, output, any(m.exists() for m in markers)

    failures = []

    def check(name, request, command, program, code, expected, never_ran, prefix=(), scenario=None):
        got, out, ran = run(request, command, program, prefix, scenario)
        if got != code or expected not in out or (never_ran and ran):
            print(f"FAIL {name}: exit={got} payload-ran={ran}\n{out}")
            failures.append(name)
        else:
            print(f"PASS {name}")

    # Before PTRACE_O_EXITKILL is set, a supervisor that blocks or exits
    # without killing the stopped tracee lets it run untraced.
    check("native setup failure kills tracee without PTRACE_KILL", "all", "native", payload,
          1, "GINUX_INSTANCE: failed reason=native ptrace setup failed\n", True)
    check("process-native setup failure kills tracee without PTRACE_KILL", "all",
          "process-native", payload,
          1, "GINUX_INSTANCE: failed reason=process ptrace options failed\n", True)
    # A post-setup failure must not hang when PTRACE_KILL is refused.
    check("native failure without PTRACE_KILL", PTRACE_KILL, "native", native,
          1, "GINUX_INSTANCE: failed reason=unexpected native trace stop\n", False,
          scenario="trap")
    check("gate1 exit without PTRACE_KILL", PTRACE_KILL, "gate1", managed,
          0, "GATE1: PASS\n", False)
    check("native failure without CAP_KILL", UNUSED_REQUEST, "native", native,
          1, "GINUX_INSTANCE: failed reason=unexpected native trace stop\n", False,
          ("setpriv", "--bounding-set", "-kill"), "trap")
    if failures:
        sys.exit(f"test-tracee-kill: FAIL ({len(failures)} of 5)")
    print("test-tracee-kill: PASS")


if __name__ == "__main__":
    main()
