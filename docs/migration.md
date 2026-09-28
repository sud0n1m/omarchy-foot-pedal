# Migrating from the Python companion service (1.x)

Version 2 starts a Rust worker directly from the enabled Omarchy plugin. The old
systemd service must be removed once to avoid competing for the same pedal and
socket. Your presets use the same file and format and are retained.

Before updating an existing 1.x installation:

```bash
foot-pedal-uninstall
omarchy plugin update sudonim.foot-pedal
omarchy restart shell
```

The 1.x uninstaller verifies its installation receipt before changing files,
stops/disables only `streamdeck-pedal-actions.service`, and removes its own
executables and app launcher. It retains `~/.config/foot-pedal/config.json`.
The copied uninstaller works even after updating or removing the old plugin.
Locally changed files are preserved with an error; inspect that error instead
of deleting files blindly. No migration is run automatically by version 2.

If controls report that another worker is running, finish the migration above.
The Rust worker takes the same exclusive lock as 1.x, so it cannot duplicate
pedal actions or remove the active worker's socket. Click **Start controls** or
disable/re-enable the plugin after migration.

For a pre-receipt local 1.1 installation, use the tagged 1.2.2 installer once to
recognize the known legacy payloads and create a receipt, then use its
uninstaller. Do this from a separate checkout of that tag; do not overwrite an
unrelated or modified file. Earlier release instructions remain in Git history.

The bar opens the panel. The previous separately installed launcher is removed;
`omarchy-shell sudonim.foot-pedal open` also opens it. Startup now follows whether
the plugin is enabled, and **Pedal actions** still controls whether presses run
actions. Shell restart briefly interrupts controls, with microphone recovery
recorded before unmuting and retried on startup if necessary.
