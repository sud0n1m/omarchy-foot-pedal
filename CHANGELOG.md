# Changelog

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
