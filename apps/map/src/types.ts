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
  color?: string;
  createdAt: number;
}

export interface UserMarkersExport {
  schema: 'leonida-user-markers-v1';
  markers: UserMarker[];
}

// ---------- State of Leonida v0.3 Types ----------
export type SolDatasetKey =
  | 'markers'
  | 'trailer1'
  | 'trailer2'
  | 'trailer3'
  | 'leaks'
  | 'screenshots'
  | 'el_screenshots';

export interface SolEntry {
  readonly id: string;
  readonly title: string;
  readonly category?: string;
  readonly dataset: string;
  readonly x: number;
  readonly y: number;
  readonly z?: number;
  readonly district?: string;
  readonly status?: string;
  readonly description?: string;
  readonly timestampSec?: number;
  readonly mediaUrl?: string;
  readonly realWorld?: string;
}

export interface ChallengeScoreResult {
  readonly distance_m: number;
  readonly score: number;
  readonly grade: 'S' | 'A' | 'B' | 'C' | 'D';
}

export interface LeaderboardEntry {
  readonly rank?: number;
  readonly player: string;
  readonly score: number;
  readonly distance_m?: number;
  readonly grade?: string;
  readonly rounds?: number;
  readonly created_at?: string | number;
}

export interface GameStats {
  readonly total_plays?: number;
  readonly best_score?: number;
  readonly avg_score?: number;
}

export interface RankInfo {
  readonly name: string;
  readonly min_score?: number;
  readonly badge?: string;
  readonly color?: string;
}

export interface NotificationItem {
  readonly id: string;
  readonly title: string;
  readonly message: string;
  readonly read?: boolean;
  readonly created_at?: string;
}

export interface NewsItem {
  readonly id?: string;
  readonly title: string;
  readonly date?: string;
  readonly summary?: string;
  readonly url?: string;
  readonly tag?: string;
  readonly x?: number;
  readonly y?: number;
}

export interface ChangelogItem {
  readonly version?: string;
  readonly date?: string;
  readonly title?: string;
  readonly changes?: readonly string[];
  readonly description?: string;
}

export interface MediaTabInfo {
  readonly key: string;
  readonly label: string;
  readonly count?: number;
}

export interface MapSettings {
  showGrid: boolean;
  showLabels: boolean;
  show3dByDefault: boolean;
  autoFly: boolean;
}

export interface ClusterCustomColors {
  landmarks: string;
  trailer1: string;
  trailer2: string;
  trailer3: string;
  leaks: string;
  screenshots: string;
}

export interface StudioSettings {
  mapSettings: MapSettings;
  clusterCustomColors: ClusterCustomColors;
  markerCustomColor: string;
  bgEffects: boolean;
}

// ---------- v0.4 Types: Photogrammetry & Route Planning ----------
export interface CameraObs {
  world_x: number;
  world_y: number;
  world_z: number;
  px: number;
  py: number;
  label?: string;
}

export interface CameraPose {
  x: number;
  y: number;
  z: number;
  yaw_deg: number;
  pitch_deg: number;
  fov_deg: number;
  rmse_px: number;
}

export interface PhotogrammetryProject {
  id?: string;
  name: string;
  image_name?: string;
  width: number;
  height: number;
  observations: CameraObs[];
  pose?: CameraPose;
  frustum?: [number, number][];
  created_at?: string;
}

export interface RouteResult {
  waypoints: [number, number][];
  distance_m: number;
  drive_min: number;
  fly_min: number;
}

// ---------- v0.5 Types: Companion PWA & RAGE Tools ----------
export interface CollectibleItem {
  id: string;
  title: string;
  category: string;
  district?: string;
  x: number;
  y: number;
  completed: boolean;
}

export interface CompanionStats {
  total: number;
  completed: number;
  percent: number;
}

export interface HandlingItem {
  handlingName: string;
  fMass: number;
  fInitialDragCoeff: number;
  fDriveBiasFront: number;
  nInitialDriveGears: number;
  fInitialDriveForce: number;
  fDriveMaxFlatVel: number;
  fBrakeForce: number;
  fTractionCurveMax: number;
  fSuspensionForce: number;
}

export interface HeaderInspectResult {
  kind: string;
  magic?: string;
  entryCount?: number;
  namesLength?: number;
  encryption?: string;
  systemFlags?: number;
  graphicsFlags?: number;
  systemSize?: number;
  graphicsSize?: number;
  details?: string;
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
