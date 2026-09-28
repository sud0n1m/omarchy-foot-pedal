"""Black-box checks of the shipped Rust binary with isolated config/runtime paths."""
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("PEDAL_BINARY", ROOT / "bin/foot-pedal")).resolve()


class RuntimeTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.home = Path(self.tmp.name)
        self.runtime = self.home / "runtime"
        self.runtime.mkdir()
        mock = self.home / "bin"
        mock.mkdir()
        pactl = mock / "pactl"
        pactl.write_text('#!/bin/sh\nprintf "[]\\n"\n')
        pactl.chmod(0o755)
        for name in ('hyprctl', 'voxtype', 'busctl', 'wtype'):
            tool = mock / name
            tool.write_text('#!/bin/sh\nexit 0\n')
            tool.chmod(0o755)
        self.env = dict(os.environ, HOME=str(self.home), XDG_CONFIG_HOME=str(self.home / "config"), XDG_RUNTIME_DIR=str(self.runtime), PATH=f"{mock}:/usr/bin:/bin")
        self.processes = []
        self.sockets = []
        self.addCleanup(self.cleanup)

    def cleanup(self):
        for s in self.sockets:
            s.close()
        for p in self.processes:
            if p.poll() is None:
                p.terminate()
                try:
                    p.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    p.kill()
                    p.wait(timeout=3)
            p.stderr.close()

    def start(self, plugin_session=False):
        proc = subprocess.Popen([str(BINARY)] + (["--plugin-session"] if plugin_session else []), env=self.env, stderr=subprocess.PIPE, stdout=subprocess.DEVNULL)
        self.processes.append(proc)
        path = self.runtime / "foot-pedal/control.sock"
        for _ in range(150):
            if path.exists():
                break
            if proc.poll() is not None:
                self.fail(proc.stderr.read().decode())
            time.sleep(.01)
        s = socket.socket(socket.AF_UNIX)
        s.settimeout(3)
        s.connect(str(path))
        self.sockets.append(s)
        return proc, s, s.makefile('r')

    def rpc(self, s, reader, message):
        s.sendall((json.dumps(dict(message, id=1)) + '\n').encode())
        while True:
            result = json.loads(reader.readline())
            if result['type'] == 'result':
                return result

    def test_fresh_launch_without_installer_and_shutdown_with_client(self):
        p, s, r = self.start()
        state = json.loads(r.readline())['state']
        self.assertEqual(state['runtime'], 'rust')
        self.assertEqual(state['lifecycle'], 'plugin')
        self.assertEqual(state['active'], 'workspaces')
        self.assertEqual((self.runtime / 'foot-pedal/control.sock').stat().st_mode & 0o777, 0o600)
        self.assertFalse((self.home / '.config/systemd').exists())
        p.terminate()
        self.assertEqual(p.wait(timeout=3), 0)
        self.assertFalse((self.runtime / 'foot-pedal/control.sock').exists())
        r.close()

    def test_single_owner_refuses_duplicate_without_removing_socket(self):
        p, s, r = self.start()
        r.readline()
        duplicate = subprocess.run([str(BINARY)], env=self.env, capture_output=True, timeout=3)
        self.assertNotEqual(duplicate.returncode, 0)
        self.assertIn(b'already running', duplicate.stderr)
        self.assertTrue(self.rpc(s, r, {'op': 'status'})['ok'])
        self.assertIsNone(p.poll())
        r.close()

    def test_custom_preset_round_trip_and_validation_preserves_config(self):
        p, s, r = self.start()
        r.readline()
        preset = {'id': '', 'name': 'My saved controls', 'actions': [{'type': 'none'}] * 3}
        result = self.rpc(s, r, {'op': 'save', 'preset': preset})
        self.assertTrue(result['ok'], result)
        active = result['state']['active']
        path = self.home / 'config/foot-pedal/config.json'
        before = path.read_bytes()
        self.assertFalse(self.rpc(s, r, {'op': 'enabled', 'value': 'false'})['ok'])
        self.assertEqual(path.read_bytes(), before)
        p.terminate()
        p.wait(timeout=3)
        r.close()
        p2, s2, r2 = self.start()
        self.assertEqual(json.loads(r2.readline())['state']['active'], active)
        self.assertEqual(path.read_bytes(), before)
        r2.close()

    def test_invalid_config_is_preserved_and_pauses_actions(self):
        path = self.home / 'config/foot-pedal/config.json'
        path.parent.mkdir(parents=True)
        path.write_bytes(b'{invalid')
        p, s, r = self.start()
        state = json.loads(r.readline())['state']
        self.assertFalse(state['enabled'])
        self.assertIn('Configuration needs attention', state['config_error'])
        self.assertFalse(self.rpc(s, r, {'op': 'enabled', 'value': True})['ok'])
        self.assertEqual(path.read_bytes(), b'{invalid')
        r.close()

    def test_plugin_lifetime_socket_stops_worker_on_disconnect(self):
        p, s, r = self.start(plugin_session=True)
        r.readline()
        owner = socket.socket(socket.AF_UNIX)
        owner.settimeout(3)
        owner.connect(str(self.runtime / 'foot-pedal/owner.sock'))
        self.assertEqual(json.loads(owner.recv(256))['pid'], p.pid)
        owner.close()
        self.assertEqual(p.wait(timeout=3), 0)
        self.assertFalse((self.runtime / 'foot-pedal/owner.sock').exists())
        r.close()

    def test_oversize_request_does_not_break_daemon(self):
        p, s, r = self.start()
        r.readline()
        s.sendall(b'x' * 65537)
        self.assertEqual(r.readline(), '')
        self.assertIsNone(p.poll())
        result = subprocess.run([str(BINARY), '--status'], env=self.env, capture_output=True, timeout=3)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(json.loads(result.stdout)['ok'])
        r.close()


if __name__ == '__main__':
    unittest.main()
