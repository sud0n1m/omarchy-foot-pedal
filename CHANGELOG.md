# Changelog

## Unreleased

- USB permission setup exclusively creates a new rule and refuses every existing
  path. Replace unconditional rule deletion with manual ownership review and
  removal of only the line the user added.

- Remove uninstall from the main panel. Removal remains available through
  `foot-pedal-uninstall` followed by Omarchy’s plugin manager.

## 1.2.2

- Preflight every installed path before any writes. Refuse unrelated files,
  symlinks, modified managed files, and invalid receipts.
- Recognize pre-receipt 1.1 installations only through explicit known hashes.
- Add collision regressions for every target and verify all files remain
  untouched when setup is refused.

## 1.2.1

- Filtered libudev notifications replace unplugged-state polling. No helper
  process and no periodic discovery when the monitor is available.
- Retain two-second discovery for unavailable notifications or access errors.
- Add tests for unplugged idle sleep, notification-driven reconnect, actual
  HID report dispatch through a pipe fixture, and notification fallback.

## 1.2.0

- Standard Omarchy Git plugin installation through a root manifest.
- Explicit Install controls, Update controls, and Uninstall controls actions.
- Service updates preserve saved presets and startup preferences.
- Independent uninstaller stops the service and retains configuration, including
  when the plugin has already been removed.
- Optional device-specific active-seat USB access rule and setup instructions.
- MIT license, public installation/removal documentation, and lifecycle tests.

## 1.1.0

- Direct native socket connection replaces the Python UI bridge.
- Connected idle daemon sleeps until an event; state broadcasts are deduplicated.
- Dependency checks are cached until refresh.

## 1.0.0

- Native themed Omarchy panel, configurable actions, presets, and input testing.
- Original workspace/dictation mappings remain the default preset.
