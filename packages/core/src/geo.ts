import type { LatLon, WorldPoint } from './coords.js';
import { bearingToVector } from './coords.js';

const EARTH_R = 6371008.8;

/** Great-circle distance in metres. */
export function haversine(a: LatLon, b: LatLon): number {
  const r = Math.PI / 180;
  const dLat = (b.lat - a.lat) * r;
  const dLon = (b.lon - a.lon) * r;
  const h = Math.sin(dLat / 2) ** 2 + Math.cos(a.lat * r) * Math.cos(b.lat * r) * Math.sin(dLon / 2) ** 2;
  return 2 * EARTH_R * Math.asin(Math.sqrt(h));
}

/** Local equirectangular projection (metres, x east / y north) around an origin. */
export function projectLocal(p: LatLon, origin: LatLon): WorldPoint {
  const r = Math.PI / 180;
  return {
    x: (p.lon - origin.lon) * r * EARTH_R * Math.cos(origin.lat * r),
    y: (p.lat - origin.lat) * r * EARTH_R,
  };
}

/** 2D affine transform: x' = a*x + b*y + c ; y' = d*x + e*y + f */
export interface Affine2D {
  readonly a: number;
  readonly b: number;
  readonly c: number;
  readonly d: number;
  readonly e: number;
  readonly f: number;
}

export function applyAffine(t: Affine2D, p: WorldPoint): WorldPoint {
  return { x: t.a * p.x + t.b * p.y + t.c, y: t.d * p.x + t.e * p.y + t.f };
}

/** Solve a 3x3 linear system (Cramer's rule). Returns null when singular. */
function solve3(m: number[][], v: number[]): [number, number, number] | null {
  const det = (q: number[][]) =>
    q[0]![0]! * (q[1]![1]! * q[2]![2]! - q[1]![2]! * q[2]![1]!) -
    q[0]![1]! * (q[1]![0]! * q[2]![2]! - q[1]![2]! * q[2]![0]!) +
    q[0]![2]! * (q[1]![0]! * q[2]![1]! - q[1]![1]! * q[2]![0]!);
  const D = det(m);
  if (Math.abs(D) < 1e-12) return null;
  const out: number[] = [];
  for (let c = 0; c < 3; c++) {
    const mc = m.map((row, i) => row.map((val, j) => (j === c ? v[i]! : val)));
    out.push(det(mc) / D);
  }
  return out as [number, number, number];
}

export interface PointPair {
  readonly from: WorldPoint;
  readonly to: WorldPoint;
  readonly weight?: number;
}

/** Weighted least-squares affine fit. Needs ≥ 3 non-collinear pairs. */
export function fitAffine(pairs: readonly PointPair[]): Affine2D | null {
  if (pairs.length < 3) return null;
  const N = [
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
  ];
  const bx = [0, 0, 0];
  const by = [0, 0, 0];
  for (const { from, to, weight = 1 } of pairs) {
    const row = [from.x, from.y, 1];
    for (let i = 0; i < 3; i++) {
      for (let j = 0; j < 3; j++) N[i]![j]! += weight * row[i]! * row[j]!;
      bx[i]! += weight * row[i]! * to.x;
      by[i]! += weight * row[i]! * to.y;
    }
  }
  const sx = solve3(N, bx);
  const sy = solve3(N, by);
  if (!sx || !sy) return null;
  return { a: sx[0], b: sx[1], c: sx[2], d: sy[0], e: sy[1], f: sy[2] };
}

export interface IrlToIgModel {
  readonly origin: LatLon;
  readonly pairs: readonly PointPair[];
}

/** Build an IRL→in-game model from landmarks that have both confirmed real-world and in-game positions. */
export function buildIrlToIgModel(
  anchors: ReadonlyArray<{ irl: LatLon; ig: WorldPoint }>,
): IrlToIgModel {
  if (anchors.length === 0) throw new Error('No anchors');
  const origin = {
    lat: anchors.reduce((s, a) => s + a.irl.lat, 0) / anchors.length,
    lon: anchors.reduce((s, a) => s + a.irl.lon, 0) / anchors.length,
  };
  return { origin, pairs: anchors.map((a) => ({ from: projectLocal(a.irl, origin), to: a.ig })) };
}

export interface Prediction {
  readonly point: WorldPoint;
  /** RMS residual (in-game metres) of the local fit over the neighbours used. */
  readonly rms: number;
  readonly neighbours: number;
}

/**
 * Predict the in-game position of a real-world location with a locally weighted affine fit
 * over the k nearest anchors (the game world compresses real geography non-uniformly,
 * so a single global affine is too coarse).
 */
export function predictIg(model: IrlToIgModel, irl: LatLon, k = 10): Prediction | null {
  const q = projectLocal(irl, model.origin);
  const ranked = model.pairs
    .map((p) => ({ p, d: Math.hypot(p.from.x - q.x, p.from.y - q.y) }))
    .sort((a, b) => a.d - b.d)
    .slice(0, Math.max(3, k));
  const scale = ranked[ranked.length - 1]!.d || 1;
  const weighted = ranked.map(({ p, d }) => ({ ...p, weight: 1 / (0.05 + (d / scale) ** 2) }));
  const t = fitAffine(weighted);
  if (!t) return null;
  let se = 0;
  for (const w of weighted) {
    const r = applyAffine(t, w.from);
    se += (r.x - w.to.x) ** 2 + (r.y - w.to.y) ** 2;
  }
  return { point: applyAffine(t, q), rms: Math.sqrt(se / weighted.length), neighbours: weighted.length };
}

/** A ray on the 2D map: origin + compass bearing (degrees). */
export interface Ray2D {
  readonly origin: WorldPoint;
  readonly bearing: number;
  readonly weight?: number;
}

/** Intersection of two rays; null if parallel or behind either origin. */
export function intersectRays(r1: Ray2D, r2: Ray2D): WorldPoint | null {
  const d1 = bearingToVector(r1.bearing);
  const d2 = bearingToVector(r2.bearing);
  const den = d1.x * d2.y - d1.y * d2.x;
  if (Math.abs(den) < 1e-9) return null;
  const dx = r2.origin.x - r1.origin.x;
  const dy = r2.origin.y - r1.origin.y;
  const t1 = (dx * d2.y - dy * d2.x) / den;
  const t2 = (dx * d1.y - dy * d1.x) / den;
  if (t1 < 0 || t2 < 0) return null;
  return { x: r1.origin.x + t1 * d1.x, y: r1.origin.y + t1 * d1.y };
}

/**
 * Least-squares triangulation from N ≥ 2 bearing rays: minimises the sum of squared
 * perpendicular distances to every ray. Returns point + RMS perpendicular residual.
 */
export function triangulate(rays: readonly Ray2D[]): { point: WorldPoint; rms: number } | null {
  if (rays.length < 2) return null;
  let a11 = 0, a12 = 0, a22 = 0, b1 = 0, b2 = 0;
  for (const r of rays) {
    const d = bearingToVector(r.bearing);
    const w = r.weight ?? 1;
    const m11 = 1 - d.x * d.x, m12 = -d.x * d.y, m22 = 1 - d.y * d.y;
    a11 += w * m11; a12 += w * m12; a22 += w * m22;
    b1 += w * (m11 * r.origin.x + m12 * r.origin.y);
    b2 += w * (m12 * r.origin.x + m22 * r.origin.y);
  }
  const det = a11 * a22 - a12 * a12;
  if (Math.abs(det) < 1e-9) return null;
  const point = { x: (a22 * b1 - a12 * b2) / det, y: (a11 * b2 - a12 * b1) / det };
  let se = 0;
  for (const r of rays) {
    const d = bearingToVector(r.bearing);
    const vx = point.x - r.origin.x, vy = point.y - r.origin.y;
    se += (vx * d.y - vy * d.x) ** 2;
  }
  return { point, rms: Math.sqrt(se / rays.length) };
}
