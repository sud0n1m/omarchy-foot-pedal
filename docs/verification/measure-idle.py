"""Measure the installed daemon with the native panel closed and open.

Run without pedal input. Includes an idle subscriber to count state packets.
PSS excludes the existing shared Omarchy shell; CPU is percent of one core.
"""
import argparse
import json
import os
from pathlib import Path
import select
import socket
import subprocess
import time

def command(*args):
    return subprocess.check_output(args, text=True).strip()

def sample(pid):
    root = Path('/proc') / str(pid)
    stat = (root / 'stat').read_text().rsplit(')', 1)[1].split()
    status = dict(line.split(':', 1) for line in (root / 'status').read_text().splitlines())
    memory = dict(line.split(':', 1) for line in (root / 'smaps_rollup').read_text().splitlines()[1:])
    return {
        'cpu_ticks': int(stat[11]) + int(stat[12]),
        'voluntary_switches': int(status['voluntary_ctxt_switches']),
        'involuntary_switches': int(status['nonvoluntary_ctxt_switches']),
        'rss_kib': int(memory['Rss'].split()[0]),
        'pss_kib': int(memory['Pss'].split()[0]),
    }

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--disconnected", action="store_true")
args = parser.parse_args()
results = []
for opened in (False, True):
    command('omarchy-shell', 'sudonim.foot-pedal', 'open' if opened else 'close')
    time.sleep(1)
    pid = int(command('systemctl', '--user', 'show', 'streamdeck-pedal-actions.service', '-p', 'MainPID', '--value'))
    with socket.socket(socket.AF_UNIX) as client:
        client.connect(os.environ['XDG_RUNTIME_DIR'] + '/foot-pedal/control.sock')
        initial = b''
        while b'\n' not in initial:
            initial += client.recv(65536)
        state = json.loads(initial.split(b'\n')[0])['state']
        assert state['connected'] != args.disconnected and not state['testing']
        before = sample(pid)
        start = time.monotonic()
        traffic = b''
        while (remaining := 30 - (time.monotonic() - start)) > 0:
            if select.select([client], [], [], remaining)[0]:
                chunk = client.recv(65536)
                if not chunk:
                    raise RuntimeError('Service disconnected during measurement')
                traffic += chunk
        duration = time.monotonic() - start
        after = sample(pid)
    result = {
        'panel': 'open' if opened else 'closed', 'pid': pid,
        'connected': state['connected'], 'discovery': state.get('discovery', 'poll'),
        'seconds': round(duration, 3),
        'cpu_percent_one_core': (after['cpu_ticks'] - before['cpu_ticks']) / os.sysconf('SC_CLK_TCK') / duration * 100,
        'voluntary_context_switches': after['voluntary_switches'] - before['voluntary_switches'],
        'involuntary_context_switches': after['involuntary_switches'] - before['involuntary_switches'],
        'rss_kib': after['rss_kib'], 'pss_kib': after['pss_kib'],
        'unsolicited_state_packets': traffic.count(b'\n'),
    }
    results.append(result)
    print(json.dumps(result), flush=True)
Path(__file__).with_name('disconnected-idle-measurement.json' if args.disconnected else 'idle-measurement.json').write_text(json.dumps(results, indent=2) + '\n')
