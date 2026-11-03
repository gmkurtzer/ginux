#!/usr/bin/env python3
"""Exercise a detached CPU hog and measure a real controller response.

Run inside an isolated test environment. --volume is a pre-seeded OCI cache
containing /spin and /cooperative at docker.io/library/alpine:latest. The harness
copies it, uses QEMU user networking without forwarded ports and always kills
its own QEMU process group on completion/timeout. This proves only controller
responsiveness, not inter-process fairness or register preservation.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time


def run(args):
    work = Path(args.output).resolve()
    work.mkdir(parents=True, exist_ok=False)
    volume = work / 'volume.raw'
    shutil.copyfile(args.volume, volume)
    # run.sh writes build/qemu/ovmf-vars.fd under its own repository root.
    # Give every probe private firmware and ESP files as well as a private volume.
    source = Path(args.source).resolve()
    runner = work / 'runner'
    scripts = runner / 'tools/qemu'
    scripts.mkdir(parents=True)
    for script in ['run.sh', 'common.sh']:
        shutil.copy2(source / 'tools/qemu' / script, scripts / script)
    shutil.copytree(source / 'build/esp', runner / 'build/esp')
    env = os.environ.copy()
    env.pop('GINUX_QEMU_HOSTFWD', None)
    env.update(GINUX_VOLUME_IMAGE=str(volume), GINUX_CPUS=str(args.cpus),
               GINUX_ESP_DIR=str(runner / 'build/esp'),
               QEMU_ACCEL='kvm')
    events = []
    start = time.monotonic()
    def event(kind, **kw):
        events.append(dict(seconds=round(time.monotonic() - start, 4), event=kind, **kw))
    log = work / 'serial.log'
    with log.open('wb') as output:
        proc = subprocess.Popen([str(scripts / 'run.sh')],
                                env=env, stdin=subprocess.PIPE, stdout=output,
                                stderr=subprocess.STDOUT, start_new_session=True)
        def contents():
            return log.read_text(errors='replace').replace('\r', '')
        def wait(marker, offset=0, seconds=30):
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                value = contents()[offset:]
                if marker in value:
                    return value
                if proc.poll() is not None:
                    raise RuntimeError('QEMU exited before ' + repr(marker))
                time.sleep(.05)
            raise TimeoutError(marker)
        def send(command):
            offset = len(contents())
            proc.stdin.write((command + '\n').encode())
            proc.stdin.flush()
            event('command', command=command)
            return offset
        result = {'probe': args.init, 'cpus': args.cpus, 'status': 'ERROR'}
        try:
            wait('ginux> ')
            offset = send('domain new s1probe')
            wait('s1probe: created', offset)
            offset = send(f'domain set s1probe image=alpine:latest init=/{args.init} cpu={args.cpus}')
            wait('settings accepted', offset)
            offset = send('domain start s1probe')
            wait('s1probe: starting (detached)', offset)
            # Let the controller enter the configured runtime with no queued input.
            time.sleep(1)
            for sample in range(args.samples):
                offset = send('domain status s1probe')
                sent = time.monotonic()
                try:
                    response = wait('Execution:', offset, args.response_timeout)
                    active = ('State:       running' in response or 'State:       booting' in response) and 'Rootfs:      ready' in response
                    active = active and 'controller failed' not in contents()
                    result.update(status='RESPONSIVE' if active else 'ERROR', latency_seconds=round(time.monotonic() - sent, 4), response=response)
                    event('response', sample=sample, active=active)
                    if not active:
                        break
                    time.sleep(1)
                except TimeoutError:
                    result.update(status='NO_RESPONSE', response_timeout=args.response_timeout)
                    event('response_timeout', sample=sample)
                    break
            if result['status'] == 'RESPONSIVE' and args.lifecycle:
                for cycle in range(3):
                    offset = send('domain stop s1probe')
                    wait('s1probe: stopped', offset, args.response_timeout)
                    offset = send('domain status s1probe')
                    response = wait('Execution:', offset, args.response_timeout)
                    if 'State:       stopped' not in response or 'Execution:   inactive' not in response:
                        raise RuntimeError('stop failed: ' + response)
                    event('stopped', cycle=cycle)
                    offset = send('domain start s1probe')
                    wait('starting (detached)', offset, args.response_timeout)
                    time.sleep(1)
                    offset = send('domain status s1probe')
                    response = wait('Execution:', offset, args.response_timeout)
                    if not ('State:       booting' in response or 'State:       running' in response) or 'Execution:   idle' not in response:
                        raise RuntimeError('restart failed: ' + response)
                    event('restarted', cycle=cycle)
            if 'GINUX_KERNEL_PANIC' in contents() or 'controller failed' in contents():
                raise RuntimeError('kernel/controller failure')
        except (TimeoutError, RuntimeError, BrokenPipeError) as exc:
            result.update(status='ERROR', error=str(exc))
        finally:
            os.killpg(proc.pid, signal.SIGTERM) if proc.poll() is None else None
            try:
                proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(proc.pid, signal.SIGKILL)
                proc.wait()
            result['events'] = events
            (work / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
            if args.remove_volume:
                volume.unlink()
        print(json.dumps(result, indent=2))
        return 0 if result['status'] == args.expect else 1

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', required=True)
    parser.add_argument('--volume', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--init', choices=['spin', 'cooperative', 'context1a', 'context2a', 'context4a'], required=True)
    parser.add_argument('--cpus', type=int, choices=[1, 2, 4], default=1)
    parser.add_argument('--response-timeout', type=float, default=3)
    parser.add_argument('--expect', choices=['RESPONSIVE', 'NO_RESPONSE'], default='RESPONSIVE')
    parser.add_argument('--remove-volume', action='store_true')
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--lifecycle', action='store_true')
    raise SystemExit(run(parser.parse_args()))
