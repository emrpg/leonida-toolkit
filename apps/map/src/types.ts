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

export const WORLD_EXTENT = Object.freeze({
  size: 32768,
  half: 16384,
  minX: -16384,
  maxX: 16384,
  minY: -16384,
  maxY: 16384,
});

export const TILE_SIZE = 1024;
export const MIN_TILE_Z = 0;
export const MAX_TILE_Z = 6;

export interface TileCoord {
  readonly z: number;
  readonly x: number;
  readonly y: number;
}

export type TileManifest = Record<string, Record<string, string[]>>;

export interface TileSource {
  readonly base: string;
  readonly composite?: string;
}

export type IrlStatus = 'confirmed' | 'unconfirmed' | 'unknown';

export interface Landmark {
  readonly id: string;
  readonly igAddress: string;
  readonly name: string;
  readonly district: string | null;
  readonly ig: WorldPoint | null;
  readonly irlAddress: string;
  readonly irl: LatLon | null;
  readonly irlStatus: IrlStatus;
  readonly tags: readonly string[];
  readonly color: string;
  readonly editedAt: number;
  readonly igPhoto: boolean;
  readonly irlPhoto: boolean;
  readonly source: string;
}

export interface Landmark3D {
  readonly id: string;
  readonly name: string;
  readonly xyz: WorldPoint3;
  readonly shape: string;
  readonly width: number;
  readonly height: number;
  readonly source: string;
}

export type Gta6MapTuple = readonly [
  id: string,
  igAddress: string,
  igCoordinates: readonly number[],
  igPhoto: number,
  irlAddress: string,
  irlCoordinates: readonly number[],
  irlPhoto: number,
  tags: readonly string[],
  color: string,
  editedAt: number,
];

export interface GtamaplibVcFile {
  readonly schema: string;
  readonly landmarks: ReadonlyArray<{
    name: string;
    id: string;
    xyz: readonly number[];
    shape?: string;
    width?: number;
    height?: number;
  }>;
}

export interface LandmarkStats {
  total: number;
  positioned: number;
  byStatus: Record<IrlStatus, number>;
  byTag: Record<string, number>;
  byDistrict: Record<string, number>;
}

export interface LandmarkFilter {
  text?: string;
  tags?: readonly string[];
  statuses?: readonly IrlStatus[];
  districts?: readonly string[];
  positionedOnly?: boolean;
}

export interface NearestLandmark {
  readonly landmark: Landmark;
  readonly distance: number;
}

export interface Ray2D {
  readonly origin: WorldPoint;
  readonly bearing: number;
  readonly weight?: number;
}

export interface TriangulationResult {
  readonly point: WorldPoint;
  readonly rms: number;
}

export interface Prediction {
  readonly point: WorldPoint;
  readonly rms: number;
  readonly neighbours: number;
}

export interface UserMarker {
  id: string;
  name: string;
  x: number;
  y: number;
  note?: string;
  createdAt: number;
}

export interface UserMarkersExport {
  schema: 'leonida-user-markers-v1';
  markers: UserMarker[];
}

export function formatDistance(m: number): string {
  if (m >= 1000) return `${(m / 1000).toFixed(2)} km`;
  return `${m.toFixed(0)} m`;
}

export function formatWorld(p: WorldPoint, digits = 0): string {
  return `${p.x.toFixed(digits)}, ${p.y.toFixed(digits)}`;
}

export function bearing(a: WorldPoint, b: WorldPoint): number {
  const deg = (Math.atan2(b.x - a.x, b.y - a.y) * 180) / Math.PI;
  return (deg + 360) % 360;
}

export function idSortKey(id: string): string {
  return id.replace(/\d+/g, (d) => d.padStart(6, '0'));
}
