# Foot Pedal

First installed 2026-09-27. A native Omarchy panel and a single-owner background
service for the Elgato Stream Deck Pedal (`0fd9:0086`).

Open **Foot Pedal** in the app launcher, click the three-pedal icon next to
Wave XLR in the bar, or run `foot-pedal`.

The active preset is **Workspaces + dictation**:

| Left | Middle | Right |
| --- | --- | --- |
| Previous workspace | Toggle Voxtype dictation | Next workspace |

After the service starts or USB reconnects, press and release a pedal once to
establish the released input state. The panel shows this instruction until
initialization. Use **Test pedals** to see press/release input without executing
actions; closing the panel ends testing and restores the prior pause state.

## Controls

- **Change preset** offers the original setup, media controls and push-to-talk.
  Push-to-talk asks for a microphone and saves a custom copy; it is not Voxtype.
- Select a pedal to choose a workspace, dictation, media, microphone, shortcut,
  command or no-action assignment. The other two assignments are preserved.
- **Record shortcut** captures a chord without sending it to another app.
  Escape cancels recording. The saved chord is sent once per pedal press.
- **Save & use** commits a named preset. Built-ins are immutable; custom presets
  can be edited, duplicated, or deleted after switching to another preset.
- Commands take an executable, quoted arguments and optional working directory.
  There is no implicit shell expansion. They must finish within ten seconds;
  output capture is bounded. Existing actions may finish when new presses are
  paused. Failures appear beside the affected pedal.
- **Pedal actions** pauses dispatch. **Start at sign-in** enables/disables the
  user service for future sessions without stopping this session's controls.
- Press Tab to navigate, Enter/Space to activate controls, and Escape to go back.
  Unsaved edits can be kept or discarded; dismissing the popup retains the draft
  until the shell exits/reloads. The daemon does not depend on the open panel.

## Installed files and persistence

| Installed path | Repository snapshot |
| --- | --- |
| `~/.local/bin/streamdeck-pedal-actions` | [Daemon and control CLI](../streamdeck-pedal-actions) |
| `~/.config/systemd/user/streamdeck-pedal-actions.service` | [User service](../streamdeck-pedal-actions.service) |
| `~/.config/omarchy/plugins/sudonim.foot-pedal/` | [Native QML panel](../plugin) |
| `~/.local/bin/foot-pedal` | [Launcher](../foot-pedal) |
| `~/.local/share/applications/foot-pedal.desktop` | [Desktop entry](../foot-pedal.desktop) |
| `~/.config/foot-pedal/config.json` | [Initial installed configuration](../config.json) |

The configuration stores schema version, pause state, active preset and custom
presets. Writes validate the entire preset, atomically replace the file, and
fsync it. Existing invalid configuration is retained and dispatch starts paused
with a visible error. The installer does not overwrite an existing config.
Update its snapshot when intentionally changing saved defaults later.

The user-private socket is `$XDG_RUNTIME_DIR/foot-pedal/control.sock` (0600),
under a 0700 directory. A process lock prevents two utility daemons owning the
device. The original script was replaced, not run alongside the new service.
The native QML socket receives changed state directly; no Python UI bridge runs.
A lost connection reconnects automatically. Connected idle operation uses no
periodic daemon timer or UI heartbeat. Filtered libudev notifications wake discovery when HID devices change, including
while unplugged. Two-second discovery is only a fallback when notifications
are unavailable or device access fails; failed microphone restores retry every two seconds while
pending. Test mode alone renews its lease every second. Dependency availability
is cached until panel refresh; actions still validate dependencies when executed.
Opening the panel refreshes available microphones and actions. Socket loss is
detected immediately; a hung service is detected on a request timeout, rather
than through recurring idle probes.
Test sessions belong to one client, expire after four seconds without renewal,
and end on client disconnect. Inputs collected during tests are never replayed.

Push-to-talk restores the microphone's previous mute state on release, pause,
preset change, disconnect or normal shutdown. Shared holds of the same mic are
reference-counted. A runtime recovery journal also restores state after daemon
failure/restart. Failed restoration is retained for retry when the mic returns.
No microphone mute state was changed during installation or UI verification.

The UI binds directly to `Color.popups`, `Color.accent`, `Style.font`, native
`Button`, `Dropdown`, `TextField`, `Toggle` and panel surfaces. Theme colors,
fonts, borders and corner rounding update through Omarchy's existing machinery.
There is no private theme setting or hardcoded Tokyo Night palette.

## Installation and removal

See the [current installation, update, USB access, and removal instructions](../README.md).
Use Omarchy to install the plugin and its explicit Install controls action for
the companion service. Saved presets survive upgrades and removal.

## Verification and troubleshooting

See [verification](verification/README.md) for actual UI screenshots, test
results and remaining physical checks. The [Paper design](design/README.md)
is the source of the interaction/layout decisions.

```bash
streamdeck-pedal-actions --status
omarchy-shell sudonim.foot-pedal-status status
systemctl --user status streamdeck-pedal-actions.service
journalctl --user -u streamdeck-pedal-actions.service -n 30
python -m unittest discover -s tests -v
```

If controls are offline, **Start controls** starts the user service. If the pedal
is disconnected or access is denied, the UI distinguishes those conditions.
Configuration remains editable while the hardware is unplugged.

## Rollback

The original daemon/service are in [backups](backups). To restore only the old
behavior, disable `sudonim.foot-pedal` using `omarchy plugin disable`, install
`docs/backups/original-daemon.py` over `~/.local/bin/streamdeck-pedal-actions`, install
`docs/backups/original.service` over its user unit, reload systemd, and restart the
service. Keep the saved custom config for a later upgrade.
