#!/usr/bin/env python3
"""Check that Gate 1 kills its payload when ptrace setup fails (Linux/root).

Gate 1 executes the fixed path /var/tmp/ginux-managed-payload. Each run gets a
private mount namespace with a temporary directory bound over /var/tmp, so the
host's /var/tmp is not touched. A seccomp filter fails one ptrace request of the
supervisor, or every request except PTRACE_TRACEME, with EPERM.
"""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

PTRACE_SETOPTIONS = 0x4200
PTRACE_SYSEMU = 31

RUN = 'mount --bind "$1" /var/tmp && exec "$2" "$3" "$4"'


def main():
    if len(sys.argv) != 4 or os.geteuid() != 0:
        sys.exit("usage (as root): test-gate1-setup.py <ginux binary> <deny-ptrace> <payload>")
    binary, deny, payload = (str(Path(arg).resolve()) for arg in sys.argv[1:])

    def run(request):
        with tempfile.TemporaryDirectory(prefix="ginux-gate1-") as directory:
            work = Path(directory)
            shutil.copyfile(payload, work / "ginux-managed-payload")
            (work / "ginux-managed-payload").chmod(0o755)
            try:
                proc = subprocess.run(["unshare", "-m", "sh", "-c", RUN, "sh",
                                       str(work), deny, str(request), binary],
                                      capture_output=True, timeout=10)
                code = proc.returncode
                output = proc.stdout + proc.stderr
            except subprocess.TimeoutExpired as error:
                code = "timeout"
                output = (error.stdout or b"") + (error.stderr or b"")
            output = output.decode(errors="replace")
            # A detached payload keeps running after the supervisor exits.
            marker = work / "gate1-payload-ran"
            deadline = time.monotonic() + 2
            while not marker.exists() and time.monotonic() < deadline:
                time.sleep(0.05)
            return code, output, marker.exists()

    def check(name, request, reason):
        code, out, ran = run(request)
        if code != 1 or f"GATE1: FAIL: {reason}\n" not in out or ran:
            raise RuntimeError(f"{name}: exit={code} payload-ran={ran}\n{out}")
        print(f"PASS gate1 {name}")

    # Before PTRACE_O_EXITKILL is set, exiting would detach the payload.
    check("setoptions failure kills payload", PTRACE_SETOPTIONS, "PTRACE_SETOPTIONS failed")
    check("sysemu failure kills payload", PTRACE_SYSEMU, "PTRACE_SYSEMU unavailable")
    # A policy that refuses the setup may refuse PTRACE_KILL as well.
    check("all requests but TRACEME denied", "all", "PTRACE_SETOPTIONS failed")
    print("test-gate1-setup: PASS")


if __name__ == "__main__":
    main()
