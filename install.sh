#!/usr/bin/env bash
# Explicit companion-service setup. Omarchy installs/updates the plugin itself.
set -euo pipefail
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
exec python3 "$repo/scripts/manage.py" install
