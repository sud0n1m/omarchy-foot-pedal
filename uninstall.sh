#!/usr/bin/env bash
# Removes the companion service and launchers; preserves presets and the plugin.
set -euo pipefail
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
exec python3 "$repo/scripts/manage.py" uninstall
