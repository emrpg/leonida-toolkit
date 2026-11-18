#!/usr/bin/env bash
# Links the public-domain gta6map tile pyramid (≈162 MB, not committed) into the map app.
# Override the source with GTA6MAP_DIR=/path/to/gta6map.github.io
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="${GTA6MAP_DIR:-/root/wsl-demo/gta-peripherals/maps-and-apps/gta6map.github.io}"
if [ ! -d "$SRC/tiles" ]; then
  echo "gta6map tiles not found at $SRC/tiles" >&2
  echo "Clone it: git clone --depth 1 https://github.com/gta6map/gta6map.github.io \"$SRC\"" >&2
  exit 1
fi
mkdir -p "$ROOT/apps/map/public"
ln -sfn "$SRC/tiles" "$ROOT/apps/map/public/tiles"
echo "linked $SRC/tiles -> apps/map/public/tiles"
