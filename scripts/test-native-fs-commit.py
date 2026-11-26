#!/usr/bin/env python3
"""Exercise Gate 53's real policy-to-kernel namespace commit barrier (Linux/root)."""

import os
from pathlib import Path
import selectors
import shutil
import subprocess
import sys
import tempfile
import time


def children(pid):
    try:
        value = Path(f"/proc/{pid}/task/{pid}/children").read_text()
    except (FileNotFoundError, PermissionError, ProcessLookupError):
        return []
    return [int(child) for child in value.split()]


def descendants(pid):
    found = []
    pending = children(pid)
    while pending:
        child = pending.pop()
        if child in found:
            continue
        found.append(child)
        pending.extend(children(child))
    return found


def syscall_number(pid):
    try:
        value = Path(f"/proc/{pid}/syscall").read_text().split()
    except (FileNotFoundError, PermissionError, ProcessLookupError):
        return None
    try:
        return int(value[0])
    except (IndexError, ValueError):
        return None


def state(pid):
    try:
        for line in Path(f"/proc/{pid}/status").read_text().splitlines():
            if line.startswith("State:"):
                return line.split()[1]
    except (FileNotFoundError, PermissionError, ProcessLookupError):
        pass
    return None


def wait_until(description, predicate, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(0.01)
    raise RuntimeError(f"timed out waiting for {description}")


def main():
    if len(sys.argv) != 3 or os.geteuid() != 0:
        sys.exit("usage (as root): test-native-fs-commit.py <ginux binary> <native fixture>")
    binary, fixture = (Path(arg).resolve() for arg in sys.argv[1:])
    config = Path(__file__).resolve().parent.parent / "config/instance-gate5.toml"

    with tempfile.TemporaryDirectory(prefix="ginux-fs-commit-") as directory:
        work = Path(directory)
        shutil.copyfile(binary, work / "ginux")
        (work / "ginux").chmod(0o755)
        root = work / "root"
        (root / "usr/bin").mkdir(parents=True)
        shutil.copyfile(fixture, root / "usr/bin/bash")
        (root / "usr/bin/bash").chmod(0o755)
        (root / "tmp").mkdir()
        (root / "tmp").chmod(0o1777)
        (root / "scenario").write_text("X")
        work.chmod(0o755)
        configured = work / "configured.toml"
        configured.write_text(config.read_text().replace(
            'init = "/bin/bash"', 'init = "/usr/bin/bash"') + """
[contract.filesystem]
read = ["/"]
write = ["/"]
create = ["/"]
""")

        proc = subprocess.Popen(
            [str(work / "ginux"), "native", str(configured), str(root), "configured"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            bufsize=0)
        output = bytearray()
        selector = selectors.DefaultSelector()
        selector.register(proc.stdout, selectors.EVENT_READ)
        try:
            # The FIFO reader is now inside the host open syscall and holds a
            # shared namespace lease. This is a kernel-observed barrier, not a
            # probabilistic loop count.
            wait_until("blocked FIFO open", lambda: next(
                (pid for pid in descendants(proc.pid) if syscall_number(pid) == 2), None))
            proc.stdin.write(b"1")
            proc.stdin.flush()

            def exchange_announced():
                for key, _ in selector.select(timeout=0):
                    chunk = os.read(key.fd, 65536)
                    output.extend(chunk)
                return b"GINUX_FIXTURE: fs-commit-exchange-entering=1\n" in output

            wait_until("exchange entry announcement", exchange_announced)

            # At this point the exchange child has no syscall between its
            # report write and renameat2. Observe the same task in a persistent
            # ptrace stop: Ginux deferred the conflicting writer before effect.
            stopped = None
            for _ in range(5):
                current = {pid for pid in descendants(proc.pid) if state(pid) == "t"}
                stopped = next(iter(current), None) if stopped is None else stopped
                if stopped not in current:
                    stopped = None
                    break
                time.sleep(0.02)
            if stopped is None:
                raise RuntimeError("rename exchange was not held at a persistent tracing stop")

            proc.stdin.write(b"2")
            proc.stdin.close()
            proc.stdin = None
            remainder, _ = proc.communicate(timeout=20)
            output.extend(remainder)
        except Exception:
            proc.kill()
            remainder, _ = proc.communicate()
            output.extend(remainder)
            raise
        finally:
            selector.close()

        required = [
            b"GINUX_FIXTURE: fs-commit-reader=1\n",
            b"GINUX_FIXTURE: fs-commit-peer=1\n",
            b"GINUX_FIXTURE: fs-commit-exchange=1\n",
            b"GINUX_FIXTURE: fs-commit-object-moved-after=1\n",
            b"GINUX_NATIVE_CHECK: PASS\n",
        ]
        if proc.returncode != 0 or any(line not in output for line in required):
            sys.stderr.buffer.write(output)
            sys.exit("test-native-fs-commit: FAIL")
        print("test-native-fs-commit: PASS")


if __name__ == "__main__":
    main()
