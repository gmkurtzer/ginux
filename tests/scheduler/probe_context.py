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
               GINUX_ESP_DIR=str(runner / 'build/esp'), QEMU_ACCEL=a.accel)
    if a.accel == 'auto':
        env.pop('QEMU_ACCEL')
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
        def wait(marker, offset=0, timeout=None):
            timeout = a.response_timeout if timeout is None else timeout
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
        def status_response(name):
            offset = send('domain status ' + name)
            response = wait('Execution:', offset)
            wait('ginux> ', offset + response.index('Execution:'))
            return content()[offset:]
        try:
            wait('ginux> ', timeout=a.boot_timeout)
            for label in ['a', 'b'][:a.domains]:
                name = 's1' + label
                wait(name + ': created', send('domain new ' + name))
                wait('settings accepted', send(f'domain set {name} image={a.image} init=/{a.prefix}{expected_workers}{label} cpu={a.cpus}'))
                wait('starting (detached)', send('domain start ' + name))
                # Startup has multiple asynchronous cache/install phases. Do
                # not overwrite the selected startup slot with the next start.
                deadline = time.monotonic() + a.boot_timeout
                while time.monotonic() < deadline:
                    time.sleep(.5)
                    status = status_response(name)
                    if 'Rootfs:      ready' in status and 'Execution:   idle' in status:
                        record('started', name=name, response=status)
                        break
                    if 'State:       failed' in status or 'State:       stopped' in status:
                        raise RuntimeError(name + ': startup failed: ' + status)
                else:
                    raise RuntimeError(name + ': startup did not finish')
            record('quiet_interval_start', duration=a.quiet_seconds)
            deadline = time.monotonic() + a.quiet_seconds
            while time.monotonic() < deadline:
                if p.poll() is not None:
                    raise RuntimeError('QEMU exited in quiet interval')
                time.sleep(.2)
            record('quiet_interval_end')
            intervals = []
            for label in ['a', 'b'][:a.domains]:
                name = 's1' + label
                status = status_response(name)
                record('domain_status', name=name, response=status)
                if 'State:       running' not in status:
                    raise RuntimeError(name + ' did not reach its final console wait')
            for label in ['a', 'b'][:a.domains]:
                name = 's1' + label
                offset = send('domain attach ' + name)
                report = wait('S1_CONTEXT_', offset)
                # A full line is emitted in one buffered write, but allow the
                # serial device time to deliver its final fields.
                report = wait('\n', offset + report.index('S1_CONTEXT_'))
                match = re.search(r'S1_CONTEXT_PASS workers=(\d+) cpu_mask=(\d+) error=0 seed=(\d+)', report)
                if not match:
                    raise RuntimeError(name + ' sentinel failure: ' + report[-1200:])
                workers, mask, seed = map(int, match.groups())
                if workers != expected_workers or mask != (1 << a.cpus) - 1 or seed != (4951 if label == 'a' else 27368):
                    raise RuntimeError(f'{name}: unexpected worker report {match.group()}')
                elapsed = re.search(r'elapsed_us=(\d+)', report)
                if not elapsed or int(elapsed[1]) < a.min_elapsed_us:
                    raise RuntimeError(name + ': workload too short to exercise repeated timer exits')
                if a.prefix == 'signal' and 'signals=1 waits=5' not in report:
                    raise RuntimeError(name + ': missing signal-return/timed-wait evidence')
                if a.prefix == 'context':
                    interval = re.search(r'realtime_start_us=(\d+) realtime_end_us=(\d+)', report)
                    if not interval:
                        raise RuntimeError(name + ': missing competition interval')
                    intervals.append(tuple(map(int, interval.groups())))
                record('context_pass', name=name, workers=workers, cpu_mask=mask, seed=seed, report=report)
                exit_offset = send('x')
                wait('ginux: Linux init exited cleanly', exit_offset)
                wait('ginux> ', exit_offset)
            if len(intervals) == 2:
                overlap = min(end for _, end in intervals) - max(start for start, _ in intervals)
                record('competition_overlap', microseconds=overlap)
                if overlap < a.min_elapsed_us:
                    raise RuntimeError('domains did not overlap for repeated timer exits')
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
    p.add_argument('--response-timeout', type=float, default=10)
    p.add_argument('--boot-timeout', type=float, default=180)
    p.add_argument('--min-elapsed-us', type=int, default=500000)
    p.add_argument('--accel', choices=['auto', 'kvm', 'tcg'], default=os.environ.get('QEMU_ACCEL', 'auto'))
    p.add_argument('--image', default='docker.io/library/scheduler-test:local')
    p.add_argument('--domains', type=int, choices=[1, 2], default=2)
    p.add_argument('--prefix', choices=['context', 'signal'], default='context')
    raise SystemExit(run(p.parse_args()))
