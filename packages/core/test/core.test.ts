import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import {
  TileResolver,
  bearing,
  buildIrlToIgModel,
  filterLandmarks,
  fitAffine,
  applyAffine,
  intersectRays,
  metresPerPixel,
  nearestLandmarks,
  parseGta6MapTuple,
  parseGtamaplibVc,
  pixelToWorld,
  polygonArea,
  predictIg,
  tileForWorld,
  triangulate,
  worldToPixel,
  type Gta6MapTuple,
  type Landmark,
} from '../src/index.js';

const raw = (name: string) =>
  JSON.parse(readFileSync(fileURLToPath(new URL(`../../../data/raw/${name}`, import.meta.url)), 'utf8'));

describe('tiles', () => {
  it('matches gta6map pyramid scale', () => {
    expect(metresPerPixel(0)).toBe(32);
    expect(metresPerPixel(5)).toBe(1);
    expect(metresPerPixel(6)).toBe(0.5);
  });
  it('round-trips world <-> pixel', () => {
    const p = { x: 1340, y: 2830 };
    const { px, py } = worldToPixel(p, 6);
    expect(pixelToWorld(px, py, 6)).toEqual(p);
    expect(worldToPixel({ x: -16384, y: 16384 }, 3)).toEqual({ px: 0, py: 0 });
  });
  it('finds the right tile', () => {
    expect(tileForWorld({ x: 0, y: 0 }, 1)).toEqual({ z: 1, x: 1, y: 1 });
    expect(tileForWorld({ x: -1, y: 1 }, 1)).toEqual({ z: 1, x: 0, y: 0 });
  });
  it('resolves composite before base', () => {
    const r = new TileResolver({ base: { '2': ['1,1', '1,2'] }, comp: { '2': ['1,2'] } });
    expect(r.resolve({ base: 'base', composite: 'comp' }, { z: 2, y: 1, x: 2 })).toBe('comp');
    expect(r.resolve({ base: 'base', composite: 'comp' }, { z: 2, y: 1, x: 1 })).toBe('base');
    expect(r.resolve({ base: 'base' }, { z: 2, y: 0, x: 0 })).toBeNull();
  });
});

describe('geometry', () => {
  it('bearing is compass-based', () => {
    expect(bearing({ x: 0, y: 0 }, { x: 0, y: 10 })).toBeCloseTo(0);
    expect(bearing({ x: 0, y: 0 }, { x: 10, y: 0 })).toBeCloseTo(90);
    expect(bearing({ x: 0, y: 0 }, { x: -10, y: 0 })).toBeCloseTo(270);
  });
  it('polygon area', () => {
    expect(polygonArea([{ x: 0, y: 0 }, { x: 10, y: 0 }, { x: 10, y: 5 }, { x: 0, y: 5 }])).toBe(50);
  });
  it('intersects two rays', () => {
    const p = intersectRays({ origin: { x: 0, y: 0 }, bearing: 45 }, { origin: { x: 10, y: 0 }, bearing: 315 });
    expect(p!.x).toBeCloseTo(5);
    expect(p!.y).toBeCloseTo(5);
    expect(intersectRays({ origin: { x: 0, y: 0 }, bearing: 0 }, { origin: { x: 1, y: 0 }, bearing: 0 })).toBeNull();
  });
  it('triangulates N rays', () => {
    const target = { x: 300, y: -120 };
    const origins = [{ x: 0, y: 0 }, { x: 1000, y: 50 }, { x: 200, y: -900 }];
    const res = triangulate(origins.map((o) => ({ origin: o, bearing: bearing(o, target) })));
    expect(res!.point.x).toBeCloseTo(target.x, 6);
    expect(res!.point.y).toBeCloseTo(target.y, 6);
    expect(res!.rms).toBeLessThan(1e-6);
  });
  it('fits an exact affine', () => {
    const t = { a: 0.5, b: -0.2, c: 100, d: 0.1, e: 0.7, f: -50 };
    const pts = [{ x: 0, y: 0 }, { x: 10, y: 3 }, { x: -4, y: 8 }, { x: 7, y: -6 }];
    const fit = fitAffine(pts.map((p) => ({ from: p, to: applyAffine(t, p) })))!;
    expect(fit.a).toBeCloseTo(0.5);
    expect(fit.f).toBeCloseTo(-50);
  });
});

describe('real datasets', () => {
  const landmarks: Landmark[] = (raw('gta6map-landmarks.json') as Gta6MapTuple[]).map((r) => parseGta6MapTuple(r));

  it('parses gta6map landmarks', () => {
    expect(landmarks.length).toBeGreaterThan(700);
    const b1 = landmarks.find((l) => l.id === 'b1')!;
    expect(b1.district).toBe('Vice Beach');
    expect(b1.ig).toEqual({ x: 1340, y: 2830 });
    expect(b1.irlStatus).toBe('confirmed');
    expect(b1.name).toContain('Ritz-Carlton');
  });

  it('filters and finds nearest', () => {
    const hotels = filterLandmarks(landmarks, { tags: ['hotel'], positionedOnly: true });
    expect(hotels.length).toBeGreaterThan(10);
    expect(filterLandmarks(landmarks, { text: 'ritz carlton' }).some((l) => l.id === 'b1')).toBe(true);
    expect(nearestLandmarks(landmarks, { x: 1340, y: 2830 }, 1)[0]!.landmark.id).toBe('b1');
  });

  it('parses gtamaplib-vc 3D landmarks', () => {
    const l3 = parseGtamaplibVc(raw('gtamaplib-vc-landmarks.json'));
    expect(l3.length).toBeGreaterThan(50);
    expect(l3.every((l) => Number.isFinite(l.xyz.z))).toBe(true);
  });

  it('predicts in-game position from real-world coordinates (leave-one-out)', () => {
    const anchors = landmarks
      .filter((l) => l.ig && l.irl && l.irlStatus === 'confirmed')
      .map((l) => ({ irl: l.irl!, ig: l.ig! }));
    expect(anchors.length).toBeGreaterThan(100);
    const errors: number[] = [];
    for (let i = 0; i < anchors.length; i++) {
      const rest = anchors.filter((_, j) => j !== i);
      const pred = predictIg(buildIrlToIgModel(rest), anchors[i]!.irl);
      if (pred) errors.push(Math.hypot(pred.point.x - anchors[i]!.ig.x, pred.point.y - anchors[i]!.ig.y));
    }
    errors.sort((a, b) => a - b);
    const median = errors[Math.floor(errors.length / 2)]!;
    console.log(`IRL→IG leave-one-out: n=${errors.length} median=${median.toFixed(0)}m p90=${errors[Math.floor(errors.length * 0.9)]!.toFixed(0)}m`);
    expect(median).toBeLessThan(400);
  });
});
