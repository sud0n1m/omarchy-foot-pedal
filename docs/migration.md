# Migrating the locally copied 1.1 plugin

Omarchy refuses to add a second plugin with the same ID. Preserve the old local
plugin folder before installing the Git-managed release; keep your saved presets
and service in place. Run these commands from your Omarchy user session:

```bash
omarchy plugin remove sudonim.foot-pedal
omarchy plugin add https://github.com/sud0n1m/omarchy-foot-pedal.git --enable
```

For a non-Git plugin folder, Omarchy's remove command moves it into a timestamped
backup. Read the command's confirmation. The existing daemon continues running.
Open the new panel and click **Update controls** to install the current daemon
and an uninstall receipt. Presets and disabled startup preferences are preserved.
The old first-run setup did not write a receipt, so update controls once before
using the new uninstaller.

If the shell retains cached QML after migration, run `omarchy restart shell`.
Normal future updates use `omarchy plugin update sudonim.foot-pedal`.
