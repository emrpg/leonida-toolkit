/**
 * Leonida world coordinate frames.
 *
 * WORLD frame (canonical for this toolkit):
 *   - units: metres (community photogrammetry calibration, NOT an official Rockstar measurement)
 *   - +x = east, +y = north, +z = up (height above sea level)
 *   - origin and scale are shared with gtamaplib / gtamaplib-vc / gta6map.github.io (all public domain),
 *     so their datasets can be overlaid without re-projection.
 */

export interface WorldPoint {
  readonly x: number;
  readonly y: number;
}

export interface WorldPoint3 extends WorldPoint {
  readonly z: number;
}

export interface LatLon {
  readonly lat: number;
  readonly lon: number;
}

/** The square region covered by the gta6map tile pyramid (32 768 m wide, centred on 0,0). */
export const WORLD_EXTENT = Object.freeze({
  size: 32768,
  half: 16384,
  minX: -16384,
  maxX: 16384,
  minY: -16384,
  maxY: 16384,
});

export function distance(a: WorldPoint, b: WorldPoint): number {
  return Math.hypot(b.x - a.x, b.y - a.y);
}

export function distance3(a: WorldPoint3, b: WorldPoint3): number {
  return Math.hypot(b.x - a.x, b.y - a.y, b.z - a.z);
}

/** Compass bearing in degrees (0 = north, 90 = east) from a to b. */
export function bearing(a: WorldPoint, b: WorldPoint): number {
  const deg = (Math.atan2(b.x - a.x, b.y - a.y) * 180) / Math.PI;
  return (deg + 360) % 360;
}

/** Unit direction vector for a compass bearing. */
export function bearingToVector(bearingDeg: number): WorldPoint {
  const r = (bearingDeg * Math.PI) / 180;
  return { x: Math.sin(r), y: Math.cos(r) };
}

export function pathLength(points: readonly WorldPoint[]): number {
  let total = 0;
  for (let i = 1; i < points.length; i++) total += distance(points[i - 1]!, points[i]!);
  return total;
}

/** Polygon area (m²) via the shoelace formula; vertices in either winding. */
export function polygonArea(points: readonly WorldPoint[]): number {
  let s = 0;
  for (let i = 0; i < points.length; i++) {
    const a = points[i]!;
    const b = points[(i + 1) % points.length]!;
    s += a.x * b.y - b.x * a.y;
  }
  return Math.abs(s) / 2;
}

export function formatDistance(m: number): string {
  if (m >= 1000) return `${(m / 1000).toFixed(2)} km`;
  return `${m.toFixed(0)} m`;
}

export function formatWorld(p: WorldPoint, digits = 0): string {
  return `${p.x.toFixed(digits)}, ${p.y.toFixed(digits)}`;
}

/**
 * Presentation frame used by gta6-leonida-atlas (SVG / three.js XZ plane), derived from the REA
 * reconstruction in reports/11. Provided only for interop with exported atlas coordinates.
 * The atlas "canonical" unit is a GTADB unit; svg = canonical * 2 with north flipped.
 */
export const AtlasFrame = Object.freeze({
  canonicalBounds: Object.freeze({ west: -16000, east: 4000, south: -8000, north: 12000 }),
  canonicalToSvg(p: { east: number; north: number }): { x: number; z: number } {
    return { x: p.east * 2, z: p.north * -2 };
  },
  svgToCanonical(p: { x: number; z: number }): { east: number; north: number } {
    return { east: p.x / 2, north: p.z / -2 };
  },
});
