#!/usr/bin/env python3
"""Two detached sentinel workloads; no serial input during their competition.

Each payload finishes in a console read. Attaching only after both workloads
finish exposes their independent buffered reports and ends that read. The CPU
mask rejects a multi-vCPU boot whose workers never ran on the extra CPUs.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import time

def run(a):
    work = Path(a.output).resolve()
    work.mkdir(parents=True, exist_ok=False)
    volume = work / 'volume.raw'
    shutil.copyfile(a.volume, volume)
    # run.sh writes build/qemu/ovmf-vars.fd under its own repository root.
    # Give every probe private firmware and ESP files as well as a private volume.
    source = Path(a.source).resolve()
    runner = work / 'runner'
    scripts = runner / 'tools/qemu'
    scripts.mkdir(parents=True)
    for script in ['run.sh', 'common.sh']:
        shutil.copy2(source / 'tools/qemu' / script, scripts / script)
    shutil.copytree(source / 'build/esp', runner / 'build/esp')
    env = os.environ.copy()
    env.pop('GINUX_QEMU_HOSTFWD', None)
    env.update(GINUX_VOLUME_IMAGE=str(volume), GINUX_CPUS=str(a.cpus),
               GINUX_ESP_DIR=str(runner / 'build/esp'), QEMU_ACCEL='kvm')
    log = work / 'serial.log'
    events = []
    began = time.monotonic()
    expected_workers = a.workers or a.cpus
    result = {'cpus': a.cpus, 'workers': expected_workers, 'status': 'ERROR'}
    def record(kind, **values):
        events.append(dict(event=kind, seconds=round(time.monotonic() - began, 4), **values))
    with log.open('wb') as output:
        p = subprocess.Popen([str(scripts / 'run.sh')],
                             env=env, stdin=subprocess.PIPE, stdout=output,
                             stderr=subprocess.STDOUT, start_new_session=True)
        def content():
            return log.read_text(errors='replace').replace('\r', '')
        def wait(marker, offset=0, timeout=5):
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                text = content()[offset:]
                if marker in text:
                    return text
                if p.poll() is not None:
                    raise RuntimeError('QEMU exited')
                time.sleep(.05)
            raise TimeoutError(marker)
        def send(command):
            offset = len(content())
            p.stdin.write((command + '\n').encode()); p.stdin.flush()
            record('command', command=command)
            return offset
        try:
            wait('ginux> ', timeout=30)
            for label in ['a', 'b'][:a.domains]:
                name = 's1' + label
                wait(name + ': created', send('domain new ' + name))
                wait('settings accepted', send(f'domain set {name} image=alpine:latest init=/{a.prefix}{expected_workers}{label} cpu={a.cpus}'))
                wait('starting (detached)', send('domain start ' + name))
            record('quiet_interval_start', duration=a.quiet_seconds)
            deadline = time.monotonic() + a.quiet_seconds
            while time.monotonic() < deadline:
                if p.poll() is not None:
                    raise RuntimeError('QEMU exited in quiet interval')
                time.sleep(.2)
            record('quiet_interval_end')
            for label in ['a', 'b'][:a.domains]:
                name = 's1' + label
                status = wait('Execution:', send('domain status ' + name), 5)
                record('domain_status', name=name, response=status)
                if 'State:       running' not in status:
                    raise RuntimeError(name + ' did not reach its final console wait')
            for label in ['a', 'b'][:a.domains]:
                name = 's1' + label
                offset = send('domain attach ' + name)
                report = wait('S1_CONTEXT_', offset, 10)
                # A full line is emitted in one buffered write, but allow the
                # serial device time to deliver its final fields.
                report = wait('elapsed_us=', offset, 5)
                match = re.search(r'S1_CONTEXT_PASS workers=(\d+) cpu_mask=(\d+) error=0 seed=(\d+)', report)
                if not match:
                    raise RuntimeError(name + ' sentinel failure: ' + report[-1200:])
                workers, mask, seed = map(int, match.groups())
                if workers != expected_workers or mask != (1 << a.cpus) - 1 or seed != (4951 if label == 'a' else 27368):
                    raise RuntimeError(f'{name}: unexpected worker report {match.group()}')
                record('context_pass', name=name, workers=workers, cpu_mask=mask, seed=seed, report=report)
                exit_offset = send('x')
                wait('ginux: Linux init exited cleanly', exit_offset, 10)
                wait('ginux> ', exit_offset, 10)
            if any(marker in content() for marker in ['controller failed', 'GINUX_KERNEL_PANIC', 'domain marked failed', 'Linux user fault']):
                raise RuntimeError('kernel/controller failure in serial log')
            result['status'] = 'PASS'
        except (TimeoutError, RuntimeError, BrokenPipeError) as exc:
            result['error'] = str(exc)
        finally:
            if p.poll() is None: os.killpg(p.pid, signal.SIGTERM)
            try: p.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(p.pid, signal.SIGKILL); p.wait()
            volume.unlink()
            result['events'] = events
            (work / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
    return 0 if result['status'] == 'PASS' else 1

if __name__ == '__main__':
    p = argparse.ArgumentParser()
    for name in ['source', 'volume', 'output']: p.add_argument('--' + name, required=True)
    p.add_argument('--cpus', type=int, choices=[1, 2, 4], required=True)
    p.add_argument('--workers', type=int, choices=[1, 2, 4], help='payload worker count (default: CPUs)')
    p.add_argument('--quiet-seconds', type=float, default=50)
    p.add_argument('--domains', type=int, choices=[1, 2], default=2)
    p.add_argument('--prefix', choices=['context', 'signal'], default='context')
    raise SystemExit(run(p.parse_args()))
