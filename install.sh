#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
command -v omarchy >/dev/null
command -v python >/dev/null
omarchy plugin validate "$repo/plugin"
install -Dm755 "$repo/streamdeck-pedal-actions" "$HOME/.local/bin/streamdeck-pedal-actions"
install -Dm644 "$repo/streamdeck-pedal-actions.service" "$HOME/.config/systemd/user/streamdeck-pedal-actions.service"
install -Dm644 "$repo/plugin/Panel.qml" "$HOME/.config/omarchy/plugins/sudonim.foot-pedal/Panel.qml"
install -Dm644 "$repo/plugin/manifest.json" "$HOME/.config/omarchy/plugins/sudonim.foot-pedal/manifest.json"
install -Dm755 "$repo/foot-pedal" "$HOME/.local/bin/foot-pedal"
install -Dm644 "$repo/foot-pedal.desktop" "$HOME/.local/share/applications/foot-pedal.desktop"
if [[ ! -e "$HOME/.config/foot-pedal/config.json" ]]; then
    install -Dm600 "$repo/config.json" "$HOME/.config/foot-pedal/config.json"
fi
systemctl --user daemon-reload
systemctl --user enable streamdeck-pedal-actions.service
systemctl --user restart streamdeck-pedal-actions.service
omarchy-shell shell rescanPlugins
for attempt in {1..20}; do
    if omarchy plugin list --json | python -c 'import json,sys;sys.exit(not any(p["id"]=="sudonim.foot-pedal" for p in json.load(sys.stdin)))'; then break; fi
    sleep 0.2
done
omarchy plugin enable sudonim.foot-pedal --section right
if omarchy plugin list --json | python -c 'import json,sys;sys.exit(not any(p["id"]=="sudonim.wave-xlr" and p["enabled"] for p in json.load(sys.stdin)))'; then
    omarchy bar move sudonim.foot-pedal --after sudonim.wave-xlr
fi
update-desktop-database "$HOME/.local/share/applications"
# Quickshell can retain an old imported QML component through a plugin rescan.
omarchy restart shell
printf '%s\n' 'Installed. Open Foot Pedal from the app launcher or the three-pedal bar icon.'
