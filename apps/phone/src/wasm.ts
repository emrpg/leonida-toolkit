import initWasm, * as wasmModule from './wasm/leonida_wasm';
import type { ChallengeScoreResult, Landmark, RouteResult, TileManifest } from './types';

const { TileResolver: WasmTileResolver, score_screenshot_challenge: wasmScore } = wasmModule;

export interface PhoneWasmEngine {
  createTileResolver(manifest: TileManifest): {
    tilesets(): string[];
    resolve(base: string, composite: string | null, z: number, x: number, y: number): string | null;
  };
  scoreChallenge(tx: number, ty: number, gx: number, gy: number): ChallengeScoreResult;
  planRoute(landmarks: readonly Landmark[], x1: number, y1: number, x2: number, y2: number): RouteResult;
}

function fallbackRoute(x1: number, y1: number, x2: number, y2: number): RouteResult {
  const d = Math.hypot(x2 - x1, y2 - y1);
  return {
    waypoints: [
      [x1, y1],
      [x2, y2],
    ],
    distance_m: d,
    drive_min: d / (70_000 / 60),
    fly_min: d / (220_000 / 60),
  };
}

let cached: Promise<PhoneWasmEngine> | null = null;

export function initPhoneWasm(): Promise<PhoneWasmEngine> {
  if (!cached) {
    cached = initWasm().then(() => ({
      createTileResolver: (manifest) => {
        const inner = new WasmTileResolver(manifest);
        return {
          tilesets: () => inner.tilesets(),
          resolve: (base, composite, z, x, y) => {
            if (z < 0 || x < 0 || y < 0) return null;
            return inner.resolve(base, composite, z, x, y) ?? null;
          },
        };
      },
      scoreChallenge: (tx, ty, gx, gy) => wasmScore(tx, ty, gx, gy) as ChallengeScoreResult,
      planRoute: (_landmarks, x1, y1, x2, y2) => {
        const fn = (wasmModule as unknown as {
          plan_route?: (x1: number, y1: number, x2: number, y2: number) => RouteResult;
        }).plan_route;
        return typeof fn === 'function' ? fn(x1, y1, x2, y2) : fallbackRoute(x1, y1, x2, y2);
      },
    }));
  }
  return cached;
}
