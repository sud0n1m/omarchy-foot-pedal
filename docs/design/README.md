# Foot Pedal utility — design

2026-09-27. Implemented and installed. See the [utility](../usage.md)
and [verification](../verification/README.md).
The design was revised to native Omarchy styling and live theme inheritance.

[Open “Elgato Foot Pedal” in Paper](https://app.paper.design/file/01M3J9Y4HAHE0QJV80TJDJKMGX/p-1-0).

Four editable artboards, with reviewed PNGs and exact inline-style JSX exports
beside this file:

1. [Current setup](01-current-setup.png): physical pedal layout, active preset,
   pause, sign-in startup and test entry point.
2. [Presets](02-presets.png): original setup, media controls, push-to-talk and
   custom presets.
3. [Customize](03-customize.png): change a single pedal, record a shortcut,
   name and save a custom preset.
4. [Test pedals](04-test-pedals.png): an illustrative middle-pedal press while
   action dispatch is suppressed. This is a design state, not a hardware test.

## Confirmed current defaults

Before implementation, the live `~/.local/bin/streamdeck-pedal-actions` matched
the repository script byte-for-byte. That original is now preserved in the
[pre-install backup](../backups/original-daemon.py).

| Pedal | Action | Current command arguments |
| --- | --- | --- |
| Left | Previous workspace | `hyprctl`, `eval`, `hl.dispatch(hl.dsp.focus({ workspace = "e-1" }))` |
| Middle | Toggle dictation | `voxtype`, `record`, `toggle` |
| Right | Next workspace | `hyprctl`, `eval`, `hl.dispatch(hl.dsp.focus({ workspace = "e+1" }))` |

The daemon recognizes USB `0fd9:0086`, reads the three pressed flags from report
bytes 4–6, and runs an action on the press edge. Holding a pedal does not repeat
the action. It retries discovery after disconnect. Preserve these semantics in
the built-in **Workspaces + dictation** preset.

## Interaction contract

- One small desktop window backed by the existing user-service concept. Closing
  settings leaves the daemon running. Pause and startup are separate controls.
- Preset selection is a draft until **Use preset**. Keep the active badge on
  the running preset; give the selected row a border. Selecting the current
  preset and applying it simply closes the chooser without restarting actions.
- Built-ins stay intact. Editing a built-in creates a named custom copy.
  Preserve untouched pedal assignments. **Save & use** validates and persists
  the whole preset atomically before activating it; Cancel discards edits.
  Existing custom presets can be edited or duplicated. No app-specific automatic
  switching, chords across pedals, or macro sequencer in the first version.
- Clicking a pedal or using the Left/Middle/Right tabs opens the same editor.
  Switching tabs retains drafts. A close with unsaved edits offers keep editing
  or discard, without applying partial changes.
- Action picker groups: Workspaces (previous/next), Dictation (toggle), Media
  (previous/play-pause/next), Microphone (toggle mute/hold to unmute), Keyboard
  shortcut, Run command, and No action. Check required executables and show a
  useful reason when an action is unavailable.
- Shortcut recording captures a chord, previews it, and offers Record again and
  Cancel. It must consume the keys without sending the shortcut to another app.
  First version sends one chord per press, with no auto-repeat.
- Commands use an executable and argument fields, with an optional working
  directory. The UI must not silently evaluate user strings through a shell.
  Show validation and exit errors beside the affected pedal.
- Proposed media preset: previous track / play-pause / next track. Proposed
  push-to-talk preset: no action / hold to unmute / no action. These are new
  capabilities, not features of the current daemon.
- Push-to-talk targets an explicitly selected microphone; explain the target
  before applying. Capture its prior mute state at press, unmute while held,
  and restore it on release. Restore on disconnect, pause, preset change and
  daemon shutdown; never switch targets mid-hold. Do not change mute state just
  by selecting a preset. This is distinct from Voxtype dictation.
- Test mode is owned by the daemon: suppress actions before showing readiness,
  publish actual press/release state, and restore the previous enabled/paused
  state on exit. Use a session lease so a crashed window cannot leave it stuck.
  Do not replay presses collected during testing. A held pedal must be released
  before normal dispatch resumes.
- Disconnected: retain configuration, show “Pedal disconnected — reconnect USB”,
  clear all pressed indicators and disable testing; editing remains available.
  Reconnect automatically, require a released baseline, and never replay input.
- Service unavailable: show “Controls aren't running” with Start controls and
  Details. Access denied: explain device access separately from disconnection.
  Failed actions show the pedal, action and concise error; do not claim success
  from a press alone. No repeated failure toast on every poll.

## Visual contract and review

The current Paper preview uses the **loaded Tokyo Night theme**, read from
`~/.local/state/omarchy/current/theme/colors.toml` and `shell.toml`, with the
user overrides in `~/.config/omarchy/shell.toml`. `omarchy font current` reported
JetBrainsMono Nerd Font; Hyprland rounding is 0. This replaces the first
iteration's Alpine palette, Inter typography and rounded controls.

The utility must inherit the active Omarchy theme **at runtime**, including
changes while it is open. Paper is a tokenized design preview, not a live
connection to the desktop. [Theme tokens](theme-tokens.json) record the preview
values and their required runtime sources. Never ship these preview hex values
as the utility's fixed palette, or assume every theme is dark.

| UI role | Required native binding |
| --- | --- |
| Panel background and text | `Color.popups.background`, `Color.popups.text` |
| Panel border, including gradients/widths | Native panel surface / `Border` popup specification |
| Accent, selection emphasis | `Color.accent` |
| Secondary labels | Popup text at an appropriate opacity; preserve contrast |
| Control fill/borders, hover/focus/selected/pressed | `qs.Ui.Button`, `Style` state functions, `Border.controlSpec` |
| Switches | `qs.Ui.Toggle` / `ToggleSwitch`; native track and knob states |
| Fonts and sizes | `Style.font.family`, `heading`, `title`, `body`, `bodySmall` |
| Padding, gaps and scaling | `Style.spacing.panelPadding`, `panelGap`, `Style.space()` |
| Corners | `Style.cornerRadius`; square switch handles when rounding is zero |
| Errors | `Color.urgent`, with explicit error text |

Use the shell's native QML components and `qs.Commons` singletons. They already
merge theme and user overrides and receive theme IPC updates. Do not add a
utility-specific theme picker, poll a hardcoded theme directory, parse colors
once at startup, or require restarting the daemon to refresh appearance.
Appearance changes must preserve the active preset, drafts, focus and test mode.
Keep hardware behavior independent of the theme and UI lifecycle.

The revised overview/test panels are 620 logical px wide; chooser/editor views
are 560 px in the design. Height follows content. Native preview typography:
16 px heading, 14 px title, 12 px controls, 11 px support; 18 px panel padding,
14 px main gaps. Production honors the user's font/spacing scaling and available
screen dimensions rather than locking these preview sizes. Pedal silhouette
keeps the middle pedal wider, with flat state surfaces and no decorative shadow.

Use semantic keyboard-focusable controls and the native keyboard navigation
conventions. Give small switches an expanded row hit area. “Pressed”/“Released”
and “On”/“Off” supplement color. Narrow views scroll instead of clipping.

All four revised Paper screenshots were visually inspected for spacing, type,
contrast, alignment and fit. The final pass corrected inherited black shortcut
text and circular switch handles. Layout and behavior remain the same design
flow. At design review, the source script and desktop configuration were unchanged;
the subsequent installation is documented in the utility notes linked above.
There are no measured latency or hardware compatibility claims in this study.

## Implementation checks for the next stage

Verify unchanged original press-edge commands, preset persistence and atomic
switching, action errors, shortcut capture, unplug/replug while held, test-mode
exit/crash, and microphone restoration. Use one device owner; never start a
second competing HID reader. Back up the running script/service before replacing
them. Snapshot deployed changes and update the restore procedure at deployment.

Also verify live dark/light theme switching, font scaling, user shell overrides,
focus/hover/pressed states, and rounded/square themes without losing UI state.
