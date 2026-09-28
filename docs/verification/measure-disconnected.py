"""Measure a real Rust worker with HID discovery hidden in a private mount namespace.

Does not unplug or reconfigure host hardware. Requires unprivileged user namespaces.
"""
import json
import os
from pathlib import Path
import select
import socket
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]

def sample(pid):
    proc = Path('/proc') / str(pid)
    stat = (proc / 'stat').read_text().rsplit(')', 1)[1].split()
    status = dict(line.split(':', 1) for line in (proc / 'status').read_text().splitlines())
    memory = dict(line.split(':', 1) for line in (proc / 'smaps_rollup').read_text().splitlines()[1:])
    return {'ticks': int(stat[11]) + int(stat[12]), 'voluntary': int(status['voluntary_ctxt_switches']),
            'involuntary': int(status['nonvoluntary_ctxt_switches']), 'pss_kib': int(memory['Pss'].split()[0])}

with tempfile.TemporaryDirectory(prefix='pedal-disconnected-') as tmp:
    tmp = Path(tmp)
    empty = tmp / 'hidraw'
    empty.mkdir()
    env = dict(os.environ, XDG_CONFIG_HOME=str(tmp / 'config'), XDG_RUNTIME_DIR=str(tmp))
    proc = subprocess.Popen(['unshare', '--user', '--map-root-user', '--mount', 'sh', '-c',
                             'mount --bind "$1" /sys/class/hidraw && exec "$2"',
                             'pedal-isolated-test', str(empty), str(ROOT / 'bin/foot-pedal')], env=env)
    try:
        path = tmp / 'foot-pedal/control.sock'
        for _ in range(200):
            if path.exists():
                break
            if proc.poll() is not None:
                raise RuntimeError('Isolated worker failed to start')
            time.sleep(.02)
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(3)
            client.connect(str(path))
            packet = b''
            while b'\n' not in packet:
                packet += client.recv(65536)
            state = json.loads(packet.split(b'\n')[0])['state']
            assert state['connected'] is False and state['discovery'] == 'udev', state
            time.sleep(.3)
            # Drain initial discovery notifications before the measurement.
            while select.select([client], [], [], 0)[0]:
                client.recv(65536)
            before = sample(proc.pid)
            start = time.monotonic()
            traffic = b''
            while (remaining := 30 - (time.monotonic() - start)) > 0:
                if select.select([client], [], [], remaining)[0]:
                    packet = client.recv(65536)
                    if not packet:
                        raise RuntimeError('Worker disconnected')
                    traffic += packet
            after = sample(proc.pid)
            result = {'fixture': 'Private mount namespace with empty /sys/class/hidraw; physical pedal remains untouched',
                      'seconds': round(time.monotonic()-start, 3), 'connected': False, 'discovery': state['discovery'],
                      'cpu_ticks': after['ticks']-before['ticks'], 'pss_kib': after['pss_kib'],
                      'voluntary_context_switches': after['voluntary']-before['voluntary'],
                      'involuntary_context_switches': after['involuntary']-before['involuntary'],
                      'unsolicited_state_packets': traffic.count(b'\n')}
            Path(__file__).with_name('rust-disconnected-idle-measurement.json').write_text(json.dumps(result, indent=2)+'\n')
            print(json.dumps(result), flush=True)
    finally:
        proc.terminate()
        proc.wait(timeout=10)
