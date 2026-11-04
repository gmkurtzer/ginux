#!/usr/bin/env python3
"""Run offline detached-domain acceptance and retain every case's evidence."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

from prepare import prepare


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source', type=Path, default=Path(__file__).resolve().parents[2])
    p.add_argument('--output', type=Path)
    p.add_argument('--cpus', type=int, choices=[1, 2, 4], nargs='+', default=[1, 2, 4])
    p.add_argument('--accel', choices=['auto', 'kvm', 'tcg'], default=os.environ.get('QEMU_ACCEL', 'auto'))
    p.add_argument('--iterations', type=int, default=100_000_000)
    p.add_argument('--quiet-seconds', type=float, default=90)
    p.add_argument('--response-timeout', type=float, default=15)
    p.add_argument('--negative-source', type=Path, help='unpatched checkout with its normal EFI already built')
    a = p.parse_args()
    if a.iterations < 5 or a.quiet_seconds <= 0 or a.response_timeout <= 0:
        p.error('iterations >= 5 and positive timeouts are required')
    source = a.source.resolve()
    output = a.output.resolve() if a.output else Path(tempfile.mkdtemp(prefix='ginux-scheduler-'))
    output.mkdir(parents=True, exist_ok=True)
    print('Scheduler artifacts:', output, flush=True)
    # prepare() rejects an existing fixtures directory: stale volumes never pass.
    volume = prepare(source, output / 'fixtures', a.iterations)
    efi = source / 'build/esp/EFI/BOOT/BOOTX64.EFI'
    evidence = {'status': 'RUNNING', 'accel_requested': a.accel,
                'cpus': a.cpus, 'iterations': a.iterations,
                'efi_sha256': hashlib.sha256(efi.read_bytes()).hexdigest(),
                'cases': []}
    identity = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=source,
                              capture_output=True, text=True)
    evidence['source_commit'] = identity.stdout.strip() if identity.returncode == 0 else 'uncommitted source export'
    def save():
        (output / 'summary.json').write_text(json.dumps(evidence, indent=2) + '\n')
    def run(name, script, cpus, extra=(), candidate=source):
        command = [sys.executable, str(source / 'tests/scheduler' / script),
                   '--source', str(candidate), '--volume', str(volume),
                   '--output', str(output / name), '--cpus', str(cpus),
                   '--accel', a.accel, '--response-timeout', str(a.response_timeout), *extra]
        started = time.monotonic()
        print('Running', name, flush=True)
        with (output / (name + '.log')).open('w') as log:
            # Each probe owns its QEMU process group and all waits are bounded.
            # Killing the probe with a second outer timeout would orphan its VM.
            result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
        evidence['cases'].append({'name': name, 'exit_code': result.returncode,
                                  'seconds': round(time.monotonic() - started, 3),
                                  'command': command})
        save()
        if result.returncode:
            raise RuntimeError(f'{name} failed; see {output / (name + ".log")}')
    try:
        if a.negative_source:
            candidate = a.negative_source.resolve()
            evidence['negative_efi_sha256'] = hashlib.sha256(
                (candidate / 'build/esp/EFI/BOOT/BOOTX64.EFI').read_bytes()).hexdigest()
            run('baseline-cooperative', 'probe_responsiveness.py', 1,
                ['--init', 'cooperative', '--remove-volume'], candidate)
            run('baseline-starvation', 'probe_responsiveness.py', 1,
                ['--init', 'spin', '--expect', 'NO_RESPONSE', '--remove-volume'], candidate)
        for cpus in a.cpus:
            run(f'cooperative-{cpus}', 'probe_responsiveness.py', cpus,
                ['--init', 'cooperative', '--remove-volume'])
            run(f'spinner-{cpus}', 'probe_responsiveness.py', cpus,
                ['--init', 'spin', '--samples', '30' if cpus == 1 else '3',
                 '--lifecycle', '--remove-volume'])
            run(f'context-{cpus}', 'probe_context.py', cpus,
                ['--quiet-seconds', str(a.quiet_seconds)])
        if 1 in a.cpus:
            run('siblings-1', 'probe_context.py', 1,
                ['--workers', '4', '--quiet-seconds', str(a.quiet_seconds)])
            run('signals-1', 'probe_context.py', 1,
                ['--prefix', 'signal', '--quiet-seconds', str(a.quiet_seconds)])
        if 4 in a.cpus:
            run('active-stop-restart-4', 'probe_responsiveness.py', 4,
                ['--init', 'busy4a', '--samples', '1', '--lifecycle', '--remove-volume'])
        evidence['status'] = 'PASS'
    except (RuntimeError, subprocess.TimeoutExpired) as error:
        evidence.update(status='FAIL', error=str(error))
        print(error, file=sys.stderr)
    finally:
        save()
    print(json.dumps(evidence, indent=2))
    return 0 if evidence['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
