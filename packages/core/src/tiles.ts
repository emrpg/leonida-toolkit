import { WORLD_EXTENT, type WorldPoint } from './coords.js';

/**
 * Tile pyramid compatible with gta6map.github.io `tiles.py`:
 *   - 1024 px tiles, levels 0..6, tile file `{z}/{z},{y},{x}.jpg`
 *   - level 0: whole 32 768 m square in one tile (32 m/px); level 5: exactly 1 m/px; level 6: 0.5 m/px
 *   - tile (0,0) is the north-west corner at world (-16384, +16384)
 */
export const TILE_SIZE = 1024;
export const MIN_TILE_Z = 0;
export const MAX_TILE_Z = 6;

export interface TileCoord {
  readonly z: number;
  readonly x: number;
  readonly y: number;
}

export function tilesPerSide(z: number): number {
  return 2 ** z;
}

export function metresPerPixel(z: number): number {
  return WORLD_EXTENT.size / (tilesPerSide(z) * TILE_SIZE);
}

/** World metres → global pixel coordinates at level z (origin = NW corner). */
export function worldToPixel(p: WorldPoint, z: number): { px: number; py: number } {
  const mpp = metresPerPixel(z);
  return { px: (p.x + WORLD_EXTENT.half) / mpp, py: (WORLD_EXTENT.half - p.y) / mpp };
}

export function pixelToWorld(px: number, py: number, z: number): WorldPoint {
  const mpp = metresPerPixel(z);
  return { x: px * mpp - WORLD_EXTENT.half, y: WORLD_EXTENT.half - py * mpp };
}

export function tileForWorld(p: WorldPoint, z: number): TileCoord {
  const { px, py } = worldToPixel(p, z);
  return { z, x: Math.floor(px / TILE_SIZE), y: Math.floor(py / TILE_SIZE) };
}

/** World-space bounds of a tile. */
export function tileBounds(t: TileCoord): { min: WorldPoint; max: WorldPoint } {
  const nw = pixelToWorld(t.x * TILE_SIZE, t.y * TILE_SIZE, t.z);
  const se = pixelToWorld((t.x + 1) * TILE_SIZE, (t.y + 1) * TILE_SIZE, t.z);
  return { min: { x: nw.x, y: se.y }, max: { x: se.x, y: nw.y } };
}

export function tileFileName(t: TileCoord, ext = 'jpg'): string {
  return `${t.z}/${t.z},${t.y},${t.x}.${ext}`;
}

/**
 * Availability manifest: tileset id → level → list of "y,x" keys.
 * Produced by scripts/build-data.mjs by scanning the tile directories.
 */
export type TileManifest = Record<string, Record<string, string[]>>;

export interface TileSource {
  /** Base map tileset, e.g. "dupzor,51". */
  readonly base: string;
  /** Optional composited overlay tileset, e.g. "dupzor,51,aiwe,1,martipk,5,rickrick,2,vlad,1". */
  readonly composite?: string;
}

/**
 * Resolves the tileset directory that holds a tile, preferring the composited overlay set
 * (which only stores tiles touched by overlays) and falling back to the base set — the same
 * de-duplication scheme as gta6map.github.io.
 */
export class TileResolver {
  private readonly index = new Map<string, Map<number, Set<string>>>();

  constructor(manifest: TileManifest) {
    for (const [set, levels] of Object.entries(manifest)) {
      const m = new Map<number, Set<string>>();
      for (const [z, keys] of Object.entries(levels)) m.set(Number(z), new Set(keys));
      this.index.set(set, m);
    }
  }

  has(set: string, t: TileCoord): boolean {
    return this.index.get(set)?.get(t.z)?.has(`${t.y},${t.x}`) ?? false;
  }

  tilesets(): string[] {
    return [...this.index.keys()];
  }

  resolve(src: TileSource, t: TileCoord): string | null {
    if (src.composite && this.has(src.composite, t)) return src.composite;
    if (this.has(src.base, t)) return src.base;
    return null;
  }
}
