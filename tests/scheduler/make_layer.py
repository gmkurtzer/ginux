#!/usr/bin/env python3
"""Build a deterministic USTAR/gzip layer from the two compiled probes."""
import gzip
import io
from pathlib import Path
import tarfile
import sys

output = io.BytesIO()
with tarfile.open(fileobj=output, mode='w', format=tarfile.USTAR_FORMAT) as archive:
    for name in (sys.argv[1:] or ['spin', 'cooperative']):
        data = Path(name).read_bytes()
        member = tarfile.TarInfo(name)
        member.size = len(data)
        member.mode = 0o755
        archive.addfile(member, io.BytesIO(data))
    member = tarfile.TarInfo('tmp')
    member.type = tarfile.DIRTYPE
    member.mode = 0o1777
    archive.addfile(member)
Path('layer-ustar.tar.gz').write_bytes(gzip.compress(output.getvalue(), mtime=0))
