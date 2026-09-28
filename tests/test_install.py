import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("manage", ROOT / "scripts/manage.py")
manage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(manage)


class InstallTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.home = Path(self.tmp.name) / "a user with spaces"
        self.calls = []
        self.loaded = "not-found"
        self.enabled = False
        self.fail = None

        def run(args, **kwargs):
            self.calls.append(args)
            stdout, code = "", 0
            if "LoadState" in args:
                stdout = self.loaded
            if "is-enabled" in args:
                code = 0 if self.enabled else 1
            if self.fail and self.fail in args:
                code = 1
            return subprocess.CompletedProcess(args, code, stdout, "Simulated failure" if code else "")

        self.manager = manage.Manager(ROOT, self.home, self.home / "config", self.home / "data", run)

    def tearDown(self):
        self.tmp.cleanup()

    def test_fresh_install_payload_permissions_and_startup(self):
        result = self.manager.install()
        self.assertTrue(result["ok"])
        for key, (target, mode) in self.manager.targets.items():
            self.assertEqual(target.read_bytes(), (ROOT / key).read_bytes())
            self.assertEqual(target.stat().st_mode & 0o777, mode)
        self.assertIn(("systemctl", "--user", "enable", manage.UNIT), self.calls)
        self.assertIn(("systemctl", "--user", "restart", manage.UNIT), self.calls)
        cfg = self.manager.config / "foot-pedal/config.json"
        self.assertEqual(cfg.stat().st_mode & 0o777, 0o600)
        self.assertTrue(self.manager.status()["installed"])

    def test_upgrade_preserves_custom_config_and_disabled_startup(self):
        self.manager.install()
        config = self.manager.config / "foot-pedal/config.json"
        custom = b'{"custom": "preserve these exact bytes"}\n'
        config.write_bytes(custom)
        self.calls.clear(); self.loaded = "loaded"
        self.manager.install()
        self.assertEqual(config.read_bytes(), custom)
        self.assertNotIn(("systemctl", "--user", "enable", manage.UNIT), self.calls)

    def test_failed_start_is_reported_and_remains_removable(self):
        self.fail = "restart"
        with self.assertRaisesRegex(RuntimeError, "Simulated failure"):
            self.manager.install()
        self.assertTrue(self.manager.receipt.exists())
        self.fail = None
        self.manager.uninstall()
        self.assertFalse(self.manager.status()["installed"])

    def test_uninstall_stops_first_and_keeps_presets(self):
        self.manager.install(); self.calls.clear()
        config = self.manager.config / "foot-pedal/config.json"
        original = config.read_bytes()
        self.manager.uninstall()
        self.assertEqual(self.calls[0], ("systemctl", "--user", "stop", manage.UNIT))
        self.assertEqual(config.read_bytes(), original)
        self.assertTrue(all(not target.exists() for target, _ in self.manager.targets.values()))

    def test_uninstall_aborts_if_stop_fails(self):
        self.manager.install(); self.fail = "stop"
        with self.assertRaises(RuntimeError):self.manager.uninstall()
        self.assertTrue(all(target.exists() for target, _ in self.manager.targets.values()))

    def test_uninstall_preserves_modified_payload(self):
        self.manager.install()
        target = self.manager.bin / "foot-pedal"
        target.write_text("user modification")
        with self.assertRaisesRegex(RuntimeError, "Locally modified"):
            self.manager.uninstall()
        self.assertEqual(target.read_text(), "user modification")

    def test_uninstall_works_after_plugin_checkout_removed(self):
        self.manager.install()
        self.manager.repo = self.home / "nonexistent checkout"
        self.assertTrue(self.manager.uninstall()["ok"])

    def test_versions_and_root_manifest_agree(self):
        version = json.loads((ROOT / "manifest.json").read_text())["version"]
        self.assertIn('VERSION = "' + version + '"', (ROOT / "streamdeck-pedal-actions").read_text())
        self.assertIn('releaseVersion: "' + version + '"', (ROOT / "plugin/Panel.qml").read_text())
        entry = json.loads((ROOT / "manifest.json").read_text())["entryPoints"]["barWidget"]
        self.assertTrue((ROOT / entry).is_file())


if __name__ == "__main__":
    unittest.main()
