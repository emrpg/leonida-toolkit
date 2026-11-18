import initWasm, {
  Landmarks as WasmLandmarks,
  TileResolver as WasmTileResolver,
  path_length as wasmPathLength,
  polygon_area as wasmPolygonArea,
  triangulate as wasmTriangulate,
} from './wasm/leonida_wasm';
import type {
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
  TileCoord,
  TileManifest,
  TileSource,
  TriangulationResult,
  WorldPoint,
} from './types';

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
}

function wrapLandmarksHandle(inner: WasmLandmarks): WasmLandmarksHandle {
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

let initPromise: Promise<LeonidaWasmEngine> | null = null;

export function initLeonidaWasm(): Promise<LeonidaWasmEngine> {
  if (!initPromise) {
    initPromise = initWasm().then(() => ({
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
    }));
  }
  return initPromise;
}
