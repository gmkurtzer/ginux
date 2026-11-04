#!/usr/bin/env python3
"""Build the scheduler fixtures and a fresh offline OCI cache from source."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import subprocess
import tarfile

IMAGE = "docker.io/library/scheduler-test:local"


def prepare(source, output, iterations):
    output.mkdir(parents=True, exist_ok=False)
    cc = shlex.split(os.environ.get("GINUX_SCHEDULER_CC", "gcc"))
    flags = ["-O2", "-Werror", "-static-pie", "-pthread",
             f"-DCONTEXT_ITERATIONS={iterations}"]
    payloads = {}
    for name, files, defines in [
        ("spin", ["spin.c"], []),
        ("cooperative", ["spin.c"], ["-DCOOPERATIVE"]),
        ("busy4a", ["context.c", "context.S"],
         ["-DWORKERS=4", "-DSEED=4951", "-DCONTEXT_CONTINUOUS"]),
        *[(f"context{n}{label}", ["context.c", "context.S"],
           [f"-DWORKERS={n}", f"-DSEED={seed}"])
          for n in [1, 2, 4] for label, seed in [("a", 4951), ("b", 27368)]],
        *[(f"signal1{label}", ["signal_wait.c", "context.S"], [f"-DSEED={seed}"])
          for label, seed in [("a", 4951), ("b", 27368)]],
    ]:
        binary = output / name
        subprocess.run(cc + flags + defines +
                       [str(source / "tests/scheduler" / f) for f in files] +
                       ["-o", str(binary)], check=True)
        data = binary.read_bytes()
        if data[:5] != b"\x7fELF\x02" or data[18:20] != b"\x3e\x00":
            raise RuntimeError("payload compiler must produce x86-64 Linux ELF")
        payloads[name] = hashlib.sha256(data).hexdigest()
    archive = output / "rootfs.tar"
    with tarfile.open(archive, "w", format=tarfile.USTAR_FORMAT) as tar:
        for name in sorted(payloads):
            entry = tarfile.TarInfo(name)
            entry.mode = 0o755
            entry.size = (output / name).stat().st_size
            with (output / name).open("rb") as binary:
                tar.addfile(entry, binary)
        entry = tarfile.TarInfo("tmp")
        entry.type = tarfile.DIRTYPE
        entry.mode = 0o1777
        tar.addfile(entry)
    subprocess.run(["cargo", "run", "--locked", "--quiet", "--package",
                    "ginux-manifest-tool", "--bin", "seed-benchmark-cache", "--",
                    str(output / "volume.raw"), str(archive), IMAGE, "64"],
                   cwd=source, check=True)
    metadata = {"image": IMAGE, "iterations": iterations,
                "compiler": subprocess.check_output(cc + ["--version"], text=True),
                "host": platform.platform(), "payload_sha256": payloads,
                "rootfs_sha256": hashlib.sha256(archive.read_bytes()).hexdigest()}
    (output / "fixtures.json").write_text(json.dumps(metadata, indent=2) + "\n")
    return output / "volume.raw"


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--source", type=Path, default=Path(__file__).resolve().parents[2])
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--iterations", type=int, default=100_000_000)
    a = p.parse_args()
    if a.iterations < 5:
        p.error("iterations must be at least 5")
    prepare(a.source.resolve(), a.output.resolve(), a.iterations)
