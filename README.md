# Omarchy Foot Pedal

A configurable Elgato Stream Deck Pedal utility for Omarchy. The native panel
inherits the active theme's colors, fonts, and controls, including live theme
changes. Version **1.1.0**.

![Foot Pedal panel](docs/verification/01-main.png)

## Features

- Presets for workspaces and dictation, media controls, and push-to-talk.
- Per-pedal workspace, dictation, media, microphone, keyboard shortcut, command,
  and no-action assignments. Save, duplicate, and delete custom presets.
- Shortcut recording and a test mode that shows input without running actions.
- Pause controls and startup at sign-in.
- A single Python daemon, direct native UI socket, and no connected idle polling.

The original mapping is included as **Workspaces + dictation** and is the
default on a fresh installation:

| Left | Middle | Right |
| --- | --- | --- |
| Previous workspace | Toggle Voxtype dictation | Next workspace |

## Install

Requires a running Omarchy desktop with its Quickshell-based shell, Python 3,
systemd user services, and user access to the Elgato pedal's HID device
(`0fd9:0086`). Actions use `hyprctl`, `voxtype`, `busctl`, `pactl`, and `wtype`;
missing dependencies are shown in the panel.

```bash
git clone https://github.com/sud0n1m/omarchy-foot-pedal.git
cd omarchy-foot-pedal
./install.sh
foot-pedal
```

The installer writes user files, preserves existing presets, enables and
restarts the user service, adds the bar widget, and restarts the Omarchy shell
to load the panel. It places the widget after Wave XLR when that widget is
enabled. No root installation or additional Python packages are required.

Open **Foot Pedal** from the launcher or the three-pedal bar icon. After startup
or USB reconnection, press and release a pedal once to initialize input. Use
**Test pedals** to check the three inputs without triggering their actions.

On another machine, verify the signed-in user has access to the matching
`/dev/hidraw*` device. The tested workstation already grants access through its
active-seat ACL; this installer does not install a udev rule.

To update, run `git pull --ff-only` and `./install.sh` from this checkout.
Saved configuration lives in `~/.config/foot-pedal/config.json`.

## Development and verification

```bash
python -m unittest discover -s tests -v
bash -n install.sh
omarchy plugin validate plugin
```

The 30 backend tests cover input edges, configuration persistence, test leases,
microphone restoration, subprocess handling, connection shutdown, and idle
scheduling. [Installed verification](docs/verification/README.md) records UI,
theme, and resource checks, along with remaining physical hardware checks.
Connected idle samples measured about **16 MiB PSS** with no CPU ticks or
context switches over 30 seconds, both with the panel open and closed. This
excludes the existing Omarchy shell and does not measure action execution cost.

## Repository

- [Daemon and control CLI](streamdeck-pedal-actions), [user service](streamdeck-pedal-actions.service), and [native panel](plugin/).
- [Usage, troubleshooting, and rollback](docs/usage.md).
- [Paper designs and exported artboards](docs/design/README.md).
- [Tests](tests/) and [verification evidence](docs/verification/README.md).

This is the standalone source for the utility. It was extracted from
[`omarchy-setup` commit `21cc015`](https://github.com/sud0n1m/omarchy-setup/commit/21cc0151b5bda51da24fee0723d40a1259af8a4e).
The setup repository retains workstation restore snapshots; unrelated desktop
configuration and history are not included here.
