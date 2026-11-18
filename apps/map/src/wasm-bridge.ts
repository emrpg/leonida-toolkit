import initWasm, * as wasmModule from './wasm/leonida_wasm';
import type {
  CameraObs,
  CameraPose,
  ChallengeScoreResult,
  Gta6MapTuple,
  GtamaplibVcFile,
  Landmark,
  Landmark3D,
  LandmarkFilter,
  LandmarkStats,
  LatLon,
  NearestLandmark,
  Prediction,
  Ray2D,
  RouteResult,
  TileCoord,
  TileManifest,
  TileSource,
  TriangulationResult,
  WorldPoint,
} from './types';

const {
  Landmarks: WasmLandmarks,
  TileResolver: WasmTileResolver,
  path_length: wasmPathLength,
  polygon_area: wasmPolygonArea,
  triangulate: wasmTriangulate,
} = wasmModule;

export interface WasmLandmarksHandle {
  all(): Landmark[];
  filter(f: LandmarkFilter): Landmark[];
  nearest(p: WorldPoint, n?: number): NearestLandmark[];
  stats(): LandmarkStats;
  geojson(list?: readonly Landmark[]): object;
  predictIg(irl: LatLon, k?: number): Prediction | null;
  free(): void;
}

export interface WasmTileResolverHandle {
  tilesets(): string[];
  resolve(src: TileSource, t: TileCoord): string | null;
  free(): void;
}

export interface LeonidaWasmEngine {
  createLandmarksFromTuples(rows: Gta6MapTuple[]): WasmLandmarksHandle;
  createLandmarksFromNormalized(landmarks: Landmark[]): WasmLandmarksHandle;
  createTileResolver(manifest: TileManifest): WasmTileResolverHandle;
  parseGtamaplibVc(file: GtamaplibVcFile): Landmark3D[];
  triangulate(rays: readonly Ray2D[]): TriangulationResult | null;
  pathLength(points: readonly WorldPoint[]): number;
  polygonArea(points: readonly WorldPoint[]): number;
  scoreScreenshotChallenge(
    targetX: number,
    targetY: number,
    guessX: number,
    guessY: number,
  ): ChallengeScoreResult;
  solveCameraPose(
    obs: readonly CameraObs[],
    width: number,
    height: number,
    fovDeg?: number,
  ): CameraPose | null;
  frustumPolygon(
    x: number,
    y: number,
    yawDeg: number,
    fovDeg: number,
    rangeM?: number,
  ): [number, number][];
  planRoute(
    landmarks: readonly Landmark[],
    x1: number,
    y1: number,
    x2: number,
    y2: number,
  ): RouteResult;
}

function wrapLandmarksHandle(inner: InstanceType<typeof WasmLandmarks>): WasmLandmarksHandle {
  return {
    all: () => inner.all() as Landmark[],
    filter: (f) => inner.filter(f) as Landmark[],
    nearest: (p, n = 5) => inner.nearest(p.x, p.y, n) as NearestLandmark[],
    stats: () => inner.stats() as LandmarkStats,
    geojson: (list) => {
      if (!list) return inner.geojson() as object;
      const subset = WasmLandmarks.load_normalised(JSON.stringify(list));
      try {
        return subset.geojson() as object;
      } finally {
        subset.free();
      }
    },
    predictIg: (irl, k = 10) => (inner.predict_ig(irl.lat, irl.lon, k) as Prediction | null) ?? null,
    free: () => inner.free(),
  };
}

function parseGtamaplibVc(file: GtamaplibVcFile, source = 'gtamaplib-vc'): Landmark3D[] {
  if (!file.schema.startsWith('gtamaplib-vvc-landmarks')) {
    throw new Error(`Unsupported gtamaplib-vc schema: ${file.schema}`);
  }
  return file.landmarks
    .filter((l) => l.xyz.length >= 3)
    .map((l) => ({
      id: l.id,
      name: l.name,
      xyz: { x: l.xyz[0]!, y: l.xyz[1]!, z: l.xyz[2]! },
      shape: l.shape ?? 'box',
      width: l.width ?? 10,
      height: l.height ?? l.xyz[2]!,
      source,
    }));
}

type ScoreFn = (tx: number, ty: number, gx: number, gy: number) => ChallengeScoreResult;

function fallbackScoreChallenge(
  targetX: number,
  targetY: number,
  guessX: number,
  guessY: number,
): ChallengeScoreResult {
  const distance_m = Math.hypot(guessX - targetX, guessY - targetY);
  const score = Math.round(5000 * Math.exp(-distance_m / 1200));
  const grade =
    score >= 4500 ? 'S' : score >= 3500 ? 'A' : score >= 2200 ? 'B' : score >= 1000 ? 'C' : 'D';
  return { distance_m, score, grade };
}

function fallbackFrustumPolygon(
  x: number,
  y: number,
  yawDeg: number,
  fovDeg: number,
  rangeM = 2500,
): [number, number][] {
  const pts: [number, number][] = [[x, y]];
  const steps = 16;
  const half = fovDeg / 2;
  for (let i = 0; i <= steps; i++) {
    const a = ((yawDeg - half + (fovDeg * i) / steps) * Math.PI) / 180;
    pts.push([x + Math.sin(a) * rangeM, y + Math.cos(a) * rangeM]);
  }
  return pts;
}

function fallbackSolveCameraPose(
  obs: readonly CameraObs[],
  width: number,
  height: number,
  fovDeg = 60,
): CameraPose | null {
  if (obs.length < 3) return null;
  // Initial centroid estimate offset south by ~1000 m
  const cx = obs.reduce((s, o) => s + o.world_x, 0) / obs.length;
  const cy = obs.reduce((s, o) => s + o.world_y, 0) / obs.length;
  const cz = obs.reduce((s, o) => s + o.world_z, 0) / obs.length;
  let camX = cx;
  let camY = cy - 1000;
  let camZ = Math.max(50, cz + 80);
  let yaw = 0;
  let pitch = -5;
  const f = (width / 2) / Math.tan(((fovDeg / 2) * Math.PI) / 180);

  const project = (o: CameraObs, x: number, y: number, z: number, yawD: number, pitchD: number) => {
    const yr = (yawD * Math.PI) / 180;
    const pr = (pitchD * Math.PI) / 180;
    const dx = o.world_x - x;
    const dy = o.world_y - y;
    const dz = o.world_z - z;
    // Camera forward = (sin(yaw)*cos(pitch), cos(yaw)*cos(pitch), sin(pitch))
    const rx = dx * Math.cos(yr) - dy * Math.sin(yr);
    const fwd0 = dx * Math.sin(yr) + dy * Math.cos(yr);
    const fwd = fwd0 * Math.cos(pr) + dz * Math.sin(pr);
    const up = -fwd0 * Math.sin(pr) + dz * Math.cos(pr);
    const depth = Math.max(10, fwd);
    return { u: width / 2 + (rx / depth) * f, v: height / 2 - (up / depth) * f };
  };

  // Simple coordinate-descent refinement
  const steps = [200, 50, 10, 2];
  const angSteps = [15, 4, 1, 0.25];
  const errOf = (x: number, y: number, z: number, yw: number, pt: number) =>
    obs.reduce((s, o) => {
      const p = project(o, x, y, z, yw, pt);
      return s + (p.u - o.px) ** 2 + (p.v - o.py) ** 2;
    }, 0);

  for (let pass = 0; pass < steps.length; pass++) {
    const st = steps[pass]!;
    const ast = angSteps[pass]!;
    for (let iter = 0; iter < 4; iter++) {
      for (const d of [-st, st]) {
        if (errOf(camX + d, camY, camZ, yaw, pitch) < errOf(camX, camY, camZ, yaw, pitch)) camX += d;
        if (errOf(camX, camY + d, camZ, yaw, pitch) < errOf(camX, camY, camZ, yaw, pitch)) camY += d;
        if (camZ + d >= 5 && errOf(camX, camY, camZ + d, yaw, pitch) < errOf(camX, camY, camZ, yaw, pitch))
          camZ += d;
      }
      for (const da of [-ast, ast]) {
        if (errOf(camX, camY, camZ, yaw + da, pitch) < errOf(camX, camY, camZ, yaw, pitch)) yaw += da;
        if (
          Math.abs(pitch + da) <= 80 &&
          errOf(camX, camY, camZ, yaw, pitch + da) < errOf(camX, camY, camZ, yaw, pitch)
        )
          pitch += da;
      }
    }
  }
  const rmse_px = Math.sqrt(errOf(camX, camY, camZ, yaw, pitch) / obs.length);
  return {
    x: camX,
    y: camY,
    z: camZ,
    yaw_deg: ((yaw % 360) + 360) % 360,
    pitch_deg: pitch,
    fov_deg: fovDeg,
    rmse_px,
  };
}

function fallbackPlanRoute(
  landmarks: readonly Landmark[],
  x1: number,
  y1: number,
  x2: number,
  y2: number,
): RouteResult {
  const nodes: WorldPoint[] = [{ x: x1, y: y1 }];
  for (const l of landmarks) {
    if (l.ig) nodes.push(l.ig);
  }
  const endIdx = nodes.length;
  nodes.push({ x: x2, y: y2 });

  const n = nodes.length;
  const dist = new Float64Array(n).fill(Infinity);
  const prev = new Int32Array(n).fill(-1);
  const visited = new Uint8Array(n);
  dist[0] = 0;

  // Build sparse KNN edges (k = 6) + direct start-end fallback
  const adj: Array<Array<[number, number]>> = Array.from({ length: n }, () => []);
  for (let i = 0; i < n; i++) {
    const a = nodes[i]!;
    const nbrs: Array<[number, number]> = [];
    for (let j = 0; j < n; j++) {
      if (i === j) continue;
      const b = nodes[j]!;
      const d = Math.hypot(b.x - a.x, b.y - a.y);
      if (d <= 2200 || j === endIdx || i === 0) nbrs.push([j, d]);
    }
    nbrs.sort((u, v) => u[1] - v[1]);
    for (const [j, d] of nbrs.slice(0, 8)) {
      adj[i]!.push([j, d]);
      adj[j]!.push([i, d]);
    }
  }

  for (let step = 0; step < n; step++) {
    let u = -1;
    let best = Infinity;
    for (let i = 0; i < n; i++) {
      if (!visited[i] && dist[i]! < best) {
        best = dist[i]!;
        u = i;
      }
    }
    if (u === -1 || u === endIdx) break;
    visited[u] = 1;
    for (const [v, w] of adj[u]!) {
      const nd = best + w;
      if (nd < dist[v]!) {
        dist[v] = nd;
        prev[v] = u;
      }
    }
  }

  const waypoints: [number, number][] = [];
  if (Number.isFinite(dist[endIdx]!)) {
    let cur = endIdx;
    while (cur !== -1) {
      const p = nodes[cur]!;
      waypoints.push([p.x, p.y]);
      cur = prev[cur]!;
    }
    waypoints.reverse();
  } else {
    waypoints.push([x1, y1], [x2, y2]);
  }
  let total = 0;
  for (let i = 1; i < waypoints.length; i++) {
    const a = waypoints[i - 1]!;
    const b = waypoints[i]!;
    total += Math.hypot(b[0] - a[0], b[1] - a[1]);
  }
  return {
    waypoints,
    distance_m: total,
    drive_min: total / (70_000 / 60),
    fly_min: total / (220_000 / 60),
  };
}

let initPromise: Promise<LeonidaWasmEngine> | null = null;

export function initLeonidaWasm(): Promise<LeonidaWasmEngine> {
  if (!initPromise) {
    initPromise = initWasm().then(() => {
      const maybeWasmScore = (wasmModule as unknown as { score_screenshot_challenge?: ScoreFn })
        .score_screenshot_challenge;
      return {
        createLandmarksFromTuples: (rows) =>
          wrapLandmarksHandle(WasmLandmarks.load(JSON.stringify(rows))),
        createLandmarksFromNormalized: (landmarks) =>
          wrapLandmarksHandle(WasmLandmarks.load_normalised(JSON.stringify(landmarks))),
        createTileResolver: (manifest) => {
          const inner = new WasmTileResolver(manifest);
          return {
            tilesets: () => inner.tilesets(),
            resolve: (src, t) => {
              if (t.z < 0 || t.x < 0 || t.y < 0) return null;
              return inner.resolve(src.base, src.composite ?? null, t.z, t.x, t.y) ?? null;
            },
            free: () => inner.free(),
          };
        },
        parseGtamaplibVc,
        triangulate: (rays) => (wasmTriangulate(rays) as TriangulationResult | null) ?? null,
        pathLength: (points) => wasmPathLength(points),
        polygonArea: (points) => wasmPolygonArea(points),
        scoreScreenshotChallenge: (tx, ty, gx, gy) =>
          typeof maybeWasmScore === 'function'
            ? maybeWasmScore(tx, ty, gx, gy)
            : fallbackScoreChallenge(tx, ty, gx, gy),
        solveCameraPose: (obs, width, height, fovDeg = 60) => {
          const fn = (wasmModule as unknown as {
            solve_camera_pose?: (o: unknown, w: number, h: number, f?: number) => CameraPose | null;
          }).solve_camera_pose;
          return typeof fn === 'function'
            ? (fn(obs, width, height, fovDeg) ?? null)
            : fallbackSolveCameraPose(obs, width, height, fovDeg);
        },
        frustumPolygon: (x, y, yawDeg, fovDeg, rangeM = 2500) => {
          const fn = (wasmModule as unknown as {
            frustum_polygon?: (
              x: number,
              y: number,
              yaw: number,
              fov: number,
              r?: number,
            ) => [number, number][];
          }).frustum_polygon;
          return typeof fn === 'function'
            ? fn(x, y, yawDeg, fovDeg, rangeM)
            : fallbackFrustumPolygon(x, y, yawDeg, fovDeg, rangeM);
        },
        planRoute: (landmarks, x1, y1, x2, y2) => {
          const fn = (wasmModule as unknown as {
            plan_route?: (x1: number, y1: number, x2: number, y2: number) => RouteResult;
          }).plan_route;
          return typeof fn === 'function'
            ? fn(x1, y1, x2, y2)
            : fallbackPlanRoute(landmarks, x1, y1, x2, y2);
        },
      };
    });
  }
  return initPromise;
}
