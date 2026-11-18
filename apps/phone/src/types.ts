export interface WorldPoint {
  readonly x: number;
  readonly y: number;
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
export const MAX_TILE_Z = 6;

export type TileManifest = Record<string, Record<string, string[]>>;

export interface Landmark {
  readonly id: string;
  readonly igAddress: string;
  readonly name: string;
  readonly district: string | null;
  readonly ig: WorldPoint | null;
  readonly irlAddress: string;
  readonly color: string;
}

export interface CollectibleItem {
  id: string;
  title: string;
  category: string;
  district?: string;
  x: number;
  y: number;
  completed: boolean;
}

export interface PhoneContact {
  id: string;
  name: string;
  role: string;
  district: string;
  avatar_color: string;
  bio: string;
}

export interface PhoneMessage {
  id: string;
  contact_id: string;
  sender: string;
  text: string;
  timestamp: string;
  unread?: boolean;
}

export interface SnapmaticPhoto {
  id: string;
  title: string;
  district: string;
  x: number;
  y: number;
  filter: string;
  caption: string;
  image_data_url: string;
  created_at: string;
}

export interface PhoneNote {
  id: string;
  title: string;
  body: string;
  x?: number;
  y?: number;
  updated_at: string;
}

export interface RouteResult {
  waypoints: [number, number][];
  distance_m: number;
  drive_min: number;
  fly_min: number;
}

export interface ChallengeScoreResult {
  distance_m: number;
  score: number;
  grade: 'S' | 'A' | 'B' | 'C' | 'D';
}

export function formatDistance(m: number): string {
  return m >= 1000 ? `${(m / 1000).toFixed(2)} km` : `${m.toFixed(0)} m`;
}
