#!/usr/bin/env python3
"""Explicit, per-user companion-service lifecycle. Never runs on plugin load."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

UNIT = "streamdeck-pedal-actions.service"
REPO = Path(__file__).resolve().parents[1]


def atomic_install(path, data, mode=0o644):
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix="." + path.name, dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(data)
            f.flush()
            os.fsync(f.fileno())
        os.chmod(name, mode)
        os.replace(name, path)
    finally:
        Path(name).unlink(missing_ok=True)


class Manager:
    def __init__(self, repo=REPO, home=None, config=None, data=None, runner=subprocess.run):
        self.repo = Path(repo)
        self.home = Path(home) if home else Path.home()
        self.config = Path(config or os.environ.get("XDG_CONFIG_HOME", self.home / ".config"))
        self.data = Path(data or os.environ.get("XDG_DATA_HOME", self.home / ".local/share"))
        self.bin = self.home / ".local/bin"
        self.receipt = self.data / "foot-pedal/install.json"
        self.runner = runner
        self.targets = {
            "streamdeck-pedal-actions": (self.bin / "streamdeck-pedal-actions", 0o755),
            "streamdeck-pedal-actions.service": (self.config / "systemd/user" / UNIT, 0o644),
            "foot-pedal": (self.bin / "foot-pedal", 0o755),
            "foot-pedal.desktop": (self.data / "applications/foot-pedal.desktop", 0o644),
            "scripts/manage.py": (self.bin / "foot-pedal-uninstall", 0o755),
        }

    def command(self, *args, check=True):
        result = self.runner(args, capture_output=True, text=True, timeout=30)
        if check and result.returncode:
            raise RuntimeError(result.stderr.strip()[:500] or f"{args[0]} failed ({result.returncode})")
        return result

    def status(self):
        installed = all(self.targets[k][0].is_file() for k in ("streamdeck-pedal-actions", UNIT))
        try:
            version = json.loads(self.receipt.read_text())["version"]
        except (OSError, ValueError, KeyError):
            version = ""
        return {"installed": installed, "version": version}

    def refresh_launchers(self):
        try:
            self.command("update-desktop-database", str(self.data / "applications"), check=False)
        except FileNotFoundError:
            pass  # Desktop-file-utils is optional; the .desktop file still works.

    def install(self):
        # Read the complete payload before changing installed files.
        version = json.loads((self.repo / "manifest.json").read_text())["version"]
        payload = {key: (self.repo / key).read_bytes() for key in self.targets}
        default_config = (self.repo / "config.json").read_bytes()
        loaded = self.command("systemctl", "--user", "show", UNIT, "-p", "LoadState", "--value").stdout.strip()
        first_install = loaded == "not-found"
        # Preserve an existing user's disabled startup preference on upgrades.
        enabled = first_install or self.command("systemctl", "--user", "is-enabled", UNIT, check=False).returncode == 0
        hashes = {}
        for key, (target, mode) in self.targets.items():
            atomic_install(target, payload[key], mode)
            hashes[key] = hashlib.sha256(payload[key]).hexdigest()
        config = self.config / "foot-pedal/config.json"
        if not config.exists() and not config.is_symlink():
            atomic_install(config, default_config, 0o600)
        # Record ownership before service activation so a failed start remains removable.
        atomic_install(self.receipt, (json.dumps({"version": version, "files": hashes}, indent=2) + "\n").encode(), 0o600)
        self.command("systemctl", "--user", "daemon-reload")
        if enabled:
            self.command("systemctl", "--user", "enable", UNIT)
        self.command("systemctl", "--user", "restart", UNIT)
        self.command("systemctl", "--user", "is-active", UNIT)
        self.refresh_launchers()
        return {"ok": True, "version": version, "message": "Controls installed. Saved presets and startup preferences were preserved."}

    def uninstall(self):
        if not self.receipt.exists():
            raise RuntimeError("No installation receipt. Run Install controls once before using this uninstaller.")
        receipt = json.loads(self.receipt.read_text())
        # Only remove known installed payloads, never paths supplied by a receipt.
        for key, (target, _) in self.targets.items():
            if target.exists() and (target.is_symlink() or hashlib.sha256(target.read_bytes()).hexdigest() != receipt["files"].get(key)):
                raise RuntimeError(f"Locally modified file preserved: {target}. Back it up and reinstall controls before uninstalling.")
        self.command("systemctl", "--user", "stop", UNIT)
        self.command("systemctl", "--user", "disable", UNIT)
        for target, _ in self.targets.values():
            target.unlink(missing_ok=True)
        self.receipt.unlink()
        self.command("systemctl", "--user", "daemon-reload")
        self.command("systemctl", "--user", "reset-failed", UNIT, check=False)
        self.refresh_launchers()
        return {"ok": True, "message": "Controls removed. Saved presets were kept. You can now remove the Omarchy plugin."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("install", "uninstall", "status"), nargs="?",
                        default="uninstall" if Path(sys.argv[0]).name == "foot-pedal-uninstall" else "status")
    args = parser.parse_args()
    try:
        print(json.dumps(getattr(Manager(), args.action)()))
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as e:
        print(json.dumps({"ok": False, "error": str(e)}))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
