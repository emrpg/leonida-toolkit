#!/usr/bin/env node
// Normalises raw public-domain datasets into apps/map/public/data and builds the tile manifest.
// Usage: node scripts/build-data.mjs [--tiles <dir>]   (default tiles dir: apps/map/public/tiles)
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const tilesDir = resolve(root, args[args.indexOf('--tiles') + 1] && args.includes('--tiles') ? args[args.indexOf('--tiles') + 1] : 'apps/map/public/tiles');
const outDir = join(root, 'apps/map/public/data');
mkdirSync(outDir, { recursive: true });

const rawDir = join(root, 'data/raw');
const datasets = {
  generatedAt: new Date().toISOString(),
  sources: [
    { id: 'gta6map.github.io', file: 'gta6map-landmarks.json', license: 'Public Domain (Unlicense)', url: 'https://github.com/gta6map/gta6map.github.io' },
    { id: 'gtamaplib-vc', file: 'gtamaplib-vc-landmarks.json', license: 'Public Domain (Unlicense)', url: 'https://github.com/rolux/gtamaplib-vc' },
  ],
};
for (const s of datasets.sources) {
  const src = join(rawDir, s.file);
  const json = JSON.parse(readFileSync(src, 'utf8'));
  writeFileSync(join(outDir, s.file), JSON.stringify(json));
  s.records = Array.isArray(json) ? json.length : (json.landmarks?.length ?? 0);
}

// Tile manifest: { tileset: { z: ["y,x", ...] } }
const manifest = {};
if (existsSync(tilesDir)) {
  for (const set of readdirSync(tilesDir)) {
    const setDir = join(tilesDir, set);
    if (!statSync(setDir).isDirectory()) continue;
    manifest[set] = {};
    for (const z of readdirSync(setDir)) {
      const zDir = join(setDir, z);
      if (!/^\d+$/.test(z) || !statSync(zDir).isDirectory()) continue;
      manifest[set][z] = readdirSync(zDir)
        .map((f) => /^(\d+),(\d+),(\d+)\.jpg$/.exec(f))
        .filter(Boolean)
        .map((m) => `${m[2]},${m[3]}`);
    }
  }
} else {
  console.warn(`[build-data] tiles dir not found (${tilesDir}); run "npm run assets:link" first. Map will render without basemap.`);
}
writeFileSync(join(outDir, 'tile-manifest.json'), JSON.stringify(manifest));
writeFileSync(join(outDir, 'datasets.json'), JSON.stringify(datasets, null, 2));

const tileCount = Object.values(manifest).reduce((s, l) => s + Object.values(l).reduce((a, k) => a + k.length, 0), 0);
console.log(`[build-data] ${datasets.sources.map((s) => `${s.id}=${s.records}`).join(' ')} tilesets=${Object.keys(manifest).length} tiles=${tileCount}`);
