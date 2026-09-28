# Omarchy Foot Pedal

A configurable Elgato Stream Deck Pedal utility for **Omarchy 4's Quickshell
shell**. Its native panel inherits the active theme's colors, fonts, and controls,
including live theme changes. Released under the [MIT license](LICENSE).

![Foot Pedal panel](docs/verification/01-main.png)

## Features

- Presets for workspaces and dictation, media controls, and push-to-talk.
- Per-pedal workspace, dictation, media, microphone, shortcut, command, and
  no-action assignments. Save, duplicate, and delete custom presets.
- Shortcut recording and a test mode that shows input without running actions.
- Pause controls and startup at sign-in.
- One Python daemon, a direct native UI socket, and no idle discovery polling while connected or unplugged.

The **Workspaces + dictation** preset preserves the original mapping:

| Left | Middle | Right |
| --- | --- | --- |
| Previous workspace | Toggle Voxtype dictation | Next workspace |

## Install

Requires an Omarchy 4 desktop, Python 3.12 or newer, systemd user services, and
an Elgato Stream Deck Pedal (`0fd9:0086`). Tested on Omarchy 4.0.4. The system libudev library supplies filtered device notifications. No Python
packages need to be installed. Individual actions use `hyprctl`, `voxtype`,
`busctl`, `pactl`, or `wtype`; the panel identifies missing dependencies.

```bash
omarchy plugin add https://github.com/sud0n1m/omarchy-foot-pedal.git --enable
```

Click the three-pedal bar icon, then **Install controls**. This explicit step
installs and starts the per-user background service and adds a **Foot Pedal**
app launcher. No administrator access is requested. The plugin itself does
not install anything automatically. Saved presets are preserved. Setup checks every destination before writing:
unrelated files, symlinks, or locally modified installed files are preserved
and reported. Back up and move a conflicting file aside yourself before
retrying. Known pre-receipt 1.1 files are recognized by explicit hashes.

For terminal setup after adding the plugin:

```bash
~/.config/omarchy/plugins/sudonim.foot-pedal/install.sh
```

The installer manages the companion service only; use Omarchy to manage the
plugin. First installation enables startup at sign-in. Updates preserve your
existing startup preference. Settings are stored in
`~/.config/foot-pedal/config.json` (or `$XDG_CONFIG_HOME/foot-pedal/config.json`).

After startup or USB reconnection, press and release a pedal once to initialize
input. Use **Test pedals** to check inputs without triggering their actions.

### USB permissions

If the panel reports **Device access denied**, create the supplied rule from
a terminal. It grants the active local seat access only to this pedal model.
This command exclusively creates a new file: any existing path (including an
identical rule or a symlink) causes an error without changing it. On an existing
path, stop and inspect its ownership and contents with your administrator;
do not delete, move, or overwrite it to force installation.

```bash
sudo python3 -c '
import os, sys
rule = sys.stdin.buffer.read()
fd = os.open(sys.argv[1], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
with os.fdopen(fd, "wb") as target:
    target.write(rule)
' /etc/udev/rules.d/70-elgato-foot-pedal.rules \
  < ~/.config/omarchy/plugins/sudonim.foot-pedal/packaging/70-elgato-foot-pedal.rules \
  && sudo udevadm control --reload-rules
```

Keep a record if you create this rule: it is a manual system change and is not
owned or removed by the plugin installer. If writing fails after file creation,
inspect the new file before retrying; the command will not overwrite it.

Unplug and reconnect the pedal afterward. Do not run the daemon as root or make
all HID devices world-writable. No rule is needed if your system already grants
access. The service detects reconnection through filtered libudev events. It sleeps
while unplugged and rescans when a HID device changes. A two-second fallback
is used only when notifications are unavailable or device access fails.

## Update

```bash
omarchy plugin update sudonim.foot-pedal
```

Open the panel and click **Update controls** if offered. Plugin updates do not
execute installer hooks; this explicit action updates and restarts the daemon
while preserving presets and startup preferences. This also works while the
service is stopped; terminal users can run `install.sh` above instead.

If you previously installed the 1.1 local snapshot, use the
[migration instructions](docs/migration.md) before the first `plugin add`.

## Remove

Remove the background controls, then remove the panel through Omarchy’s plugin manager:

```bash
foot-pedal-uninstall
omarchy plugin remove sudonim.foot-pedal
```

This stops and disables the service, removes its executables and app launcher,
and keeps saved presets. The uninstaller also works if you already removed the
plugin. Removing just the plugin hides the panel; it does **not** stop the
independently installed pedal service. Locally modified installed files are
preserved with an explanatory error.

The optional USB rule is a separate manual system change. There is deliberately
no automatic rule deletion. First inspect the file and check package ownership:

```bash
pacman -Qo /etc/udev/rules.d/70-elgato-foot-pedal.rules
sudo cat /etc/udev/rules.d/70-elgato-foot-pedal.rules
```

A “no package owns” result or matching rule contents does **not** prove this
plugin created the file. If a package owns it, follow that package’s guidance.
If its origin is uncertain or the path is a symlink, leave it alone. Only if you know you added the rule
yourself and it is not package-owned, use `sudoedit` on that file to remove the
pedal-specific line you added, preserving all other contents. Leave the file
in place; an empty or comment-only rules file is harmless. Then run
`sudo udevadm control --reload-rules` and reconnect the pedal.

## Development and verification

```bash
python3 -m unittest discover -s tests -v
bash -n install.sh uninstall.sh
omarchy plugin validate .
udevadm verify packaging/70-elgato-foot-pedal.rules
```

Tests cover input edges, presets, test leases, microphone restoration, process
handling, idle scheduling, and fresh install/update/uninstall behavior using
isolated filesystem and service fixtures. [Verification](docs/verification/README.md)
records actual installed UI and theme checks and distinguishes physical tests
from simulated ones. Real pedal presses and USB unplug/replug still need human
acceptance; no fresh-machine hardware claim is made.

With the pedal unplugged, 30-second samples measured **16.7–16.8 MiB PSS**,
zero CPU ticks and zero state broadcasts, with the panel closed and open. There
was one voluntary wakeup in the closed sample and none in the open sample.
This excludes the existing Omarchy shell and does not measure action execution
cost. See the [measurements](docs/verification/disconnected-idle-measurement.json).

- [Usage and action behavior](docs/usage.md)
- [Paper designs and exported artboards](docs/design/README.md)
- [Release notes](CHANGELOG.md)
- [Report a problem](https://github.com/sud0n1m/omarchy-foot-pedal/issues)
