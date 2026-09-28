# Installation verification · 2026-09-27

Installed and running on Omaxps. The service reports the physical Elgato pedal
connected; its process owns one `/dev/hidraw0` handle. Required Wayland,
Hyprland and session-bus environment is present. Startup is enabled, dispatch
is on, and **Workspaces + dictation** is active with no action errors.

- [47 backend and lifecycle tests](backend-tests.txt) passed: original commands/press edges,
  config validation and persistence, immutable built-ins, test suppression and
  ownership, disconnect, rapid release, pause/preset-change microphone restore,
  overlapping holds, crash recovery/retry, media routing, and per-pedal errors.
  Real subprocess tests verify timeout, output limits, literal argv handling, and
  clean daemon shutdown while a client remains connected. The shutdown regression
  covers Python 3.14 waiting for client transports in `Server.wait_closed()`.
  Optimization checks cover cached dependencies, duplicate-state suppression and
  initial delivery to new clients, idle sleep and test deadlines, and bounded
  retries for disconnected hardware and failed microphone restoration.
- Native keyboard-driven UI: main → presets → test → editor; recorded Ctrl+Shift+M,
  saved and activated a custom preset, restored the original and deleted the
  temporary preset. Saved microphone configuration without changing mute; checked
  keep/discard for unsaved edits. Test presets were removed afterward.
- Actual service socket: a connected test client without heartbeat lost its test
  lease after four seconds. Closing the actual testing panel ended the session
  and left normal dispatch enabled. Service restart preserved the saved config
  and the UI reconnected.
- Actual offline recovery: stopped the service, activated **Start controls**
  from the panel, and verified reconnection with no stale offline error.
- [Live theme check](theme-check.json): open panel changed to Catppuccin Latte
  and back to Tokyo Night without losing page or active preset. Original
  background restored. This tested actual native theme updates, not fixture colors.
- The installer passed an actual repeat run; the saved configuration hash was
  unchanged. Plugin manifest, desktop entry and installer shell syntax validated. Final
  source/live snapshots match. No runtime QML warnings after the corrected build.

## Version 1.1 efficiency pass

The Python UI bridge was removed; the installed QML panel connects directly
through `Quickshell.Io.Socket`. Only one pedal Python process remains. Changed
state is serialized once for subscribers; duplicate states are suppressed.
Dependency availability is cached until an explicit refresh. The daemon waits
for HID/socket events indefinitely when connected and idle, with timers only
for disconnected hardware, active test leases, or pending microphone recovery.

Reverified native offline → Start controls → online recovery, test mode renewal
beyond five seconds, four-second expiry without renewal, panel-close release,
shortcut recording, and Save & use. Restored the original preset and removed
the temporary custom preset; the saved config matches its pre-optimization
bytes. The QML log has no component errors; the deliberate service stop logged
the expected socket peer-closed warning.

[Idle measurements](idle-measurement.json) and the [repeatable measurement
script](measure-idle.py) record separate 30-second connected samples with the
panel closed and open. CPU uses `/proc` ticks; zero means below that resolution,
not a claim of zero cost for pedal actions. PSS accounts for shared Python
pages and excludes the existing Omarchy shell. The earlier 30-second baseline
had two Python processes: combined PSS 31,597 KiB (30.9 MiB), approximately
0.067% of one CPU core, and 2.5 voluntary context switches per second.
Both new samples recorded zero CPU ticks, zero context switches and zero
unsolicited packets. PSS was 16,482–16,504 KiB (16.1 MiB), about 48% lower
than the previous combined daemon/bridge footprint.

Screenshots from the actual installed panel:

- [Current setup](01-main.png)
- [Presets](02-presets.png)
- [Shortcut editor](03-edit-shortcut.png)
- [Input test](04-test-pedals.png)
- [Light theme](05-light-theme.png)
- [First-run setup](06-install-controls.png)
- [Disconnected pedal](07-disconnected.png)

[Installed status](installed-status.json) records the final configuration and
connection. `input_ready: false` means no physical report has arrived since the
restart: the panel asks for one press-and-release before normal use.

Physical presses, USB unplug/replug, and real microphone hold/release remain
for the user to exercise. Those transitions have automated fixture coverage;
they were not claimed as physical hardware tests. No dictation recording,
real microphone mute/unmute, or media playback action was triggered for testing.


## Public release 1.2.1

- 41 tests pass locally and in GitHub Actions on Python 3.12 and 3.14.
- Root manifest validates with stock Omarchy; the optional device-specific
  rule passes `udevadm verify`. No root rule was installed during this pass.
- Actual stock plugin add/remove/re-add and update paths were exercised on
  Omarchy 4.0.4. The native panel upgraded the legacy service, uninstalled it,
  and installed a fresh service while preserving saved presets byte-for-byte.
- The independent `foot-pedal-uninstall` also worked after the Git-managed
  plugin folder had already been removed. Service stop and executable removal
  were checked before reinstalling. Startup remains enabled and the original
  workspaces/dictation configuration remains active.
- Unit fixtures separately cover first-time default creation, disabled startup
  preservation, failure reporting, locally modified payload preservation, and
  uninstallation with saved configuration retained. These are isolated tests,
  not a second physical machine.
- During this release pass the owner disconnected the pedal. The daemon now
  uses filtered libudev events through its existing asyncio loop, with no
  helper process or periodic discovery timer when the monitor is available.
  Actual monitor startup is verified; a pipe fixture verifies that a hotplug
  notification opens the device and dispatches a subsequent HID report.
- [Unplugged idle measurements](disconnected-idle-measurement.json): separate
  30-second samples with the panel closed/open, zero measured CPU ticks and zero
  unsolicited state packets in both; one voluntary context switch closed and
  zero open. PSS was 17,132–17,185 KiB (16.7–16.8 MiB). Hardware notifications can
  wake the daemon; this is not a claim of zero CPU cost for device events.
- A two-second discovery fallback remains for unavailable notifications or
  failed device access. Failed microphone restores also retain bounded retries.

Physical reconnection and real pedal/microphone actuation remain manual
acceptance items. The release has not been tested on a second physical machine.


## Installer ownership review · 1.2.2

The marketplace reviewer identified that setup could replace an unrelated file
at a fixed install path. Setup now checks every destination before any payload
write or service operation. Existing files must match their receipt hash, the
current package bytes, or explicitly recorded hashes of the pre-receipt 1.1
release. Symlinks, unknown files, invalid receipts, and locally modified managed
files are preserved with a specific error. Users must back up and move a
conflicting file aside themselves before retrying.

47 tests pass. New regressions exercise every install target individually for
unknown-file and modified-file collisions, assert that no other payload or
receipt changes and no service command runs, and cover symlinks, invalid
receipts, identical payload adoption, and known legacy-file recognition.
