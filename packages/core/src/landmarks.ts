import type { LatLon, WorldPoint, WorldPoint3 } from './coords.js';

export type IrlStatus = 'confirmed' | 'unconfirmed' | 'unknown';

/** Normalised landmark record used across all Leonida Toolkit apps. */
export interface Landmark {
  readonly id: string;
  /** In-game address, e.g. "?, Vice Beach" (unknown building name in district Vice Beach). */
  readonly igAddress: string;
  /** Display name: building part of the in-game address, or the real-world name when unknown. */
  readonly name: string;
  readonly district: string | null;
  readonly ig: WorldPoint | null;
  readonly irlAddress: string;
  readonly irl: LatLon | null;
  readonly irlStatus: IrlStatus;
  readonly tags: readonly string[];
  /** 6-digit hex colour without '#'. */
  readonly color: string;
  /** Unix seconds. */
  readonly editedAt: number;
  readonly igPhoto: boolean;
  readonly irlPhoto: boolean;
  readonly source: string;
}

/** 3D landmark from gtamaplib-vc (triangulated position incl. height). */
export interface Landmark3D {
  readonly id: string;
  readonly name: string;
  readonly xyz: WorldPoint3;
  readonly shape: string;
  readonly width: number;
  readonly height: number;
  readonly source: string;
}

/** Row tuple of gta6map.github.io `data/landmarks.json` (public domain). */
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

function splitAddress(igAddress: string): { building: string | null; district: string | null } {
  const idx = igAddress.lastIndexOf(',');
  if (idx < 0) return { building: igAddress.trim() || null, district: null };
  const building = igAddress.slice(0, idx).trim();
  const district = igAddress.slice(idx + 1).trim();
  return { building: building && building !== '?' ? building : null, district: district || null };
}

function irlName(irlAddress: string): string {
  return irlAddress.split(',')[0]?.trim() ?? irlAddress;
}

export function parseGta6MapTuple(row: Gta6MapTuple, source = 'gta6map.github.io'): Landmark {
  const [id, igAddress, igC, igPhoto, irlRaw, irlC, irlPhoto, tags, color, editedAt] = row;
  const irlStatus: IrlStatus = irlRaw === '?' ? 'unknown' : irlRaw.endsWith('?') ? 'unconfirmed' : 'confirmed';
  const irlAddress = irlRaw.replace(/(?<=.)\?$/, '');
  const { building, district } = splitAddress(igAddress);
  const name = building ?? (irlStatus === 'unknown' ? `Unknown ${id}` : irlName(irlAddress));
  return {
    id,
    igAddress,
    name,
    district,
    ig: igC.length >= 2 ? { x: igC[0]!, y: igC[1]! } : null,
    irlAddress,
    irl: irlC.length >= 2 ? { lat: irlC[0]!, lon: irlC[1]! } : null,
    irlStatus,
    tags: [...tags],
    color: /^[0-9a-f]{6}$/i.test(color) ? color.toLowerCase() : '888888',
    editedAt,
    igPhoto: Boolean(igPhoto),
    irlPhoto: Boolean(irlPhoto),
    source,
  };
}

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

export function parseGtamaplibVc(file: GtamaplibVcFile, source = 'gtamaplib-vc'): Landmark3D[] {
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

/** Natural sort key for ids like "b12" / "L264". */
export function idSortKey(id: string): string {
  return id.replace(/\d+/g, (d) => d.padStart(6, '0'));
}

export interface LandmarkStats {
  total: number;
  positioned: number;
  byStatus: Record<IrlStatus, number>;
  byTag: Record<string, number>;
  byDistrict: Record<string, number>;
}

export function landmarkStats(list: readonly Landmark[]): LandmarkStats {
  const s: LandmarkStats = {
    total: list.length,
    positioned: 0,
    byStatus: { confirmed: 0, unconfirmed: 0, unknown: 0 },
    byTag: {},
    byDistrict: {},
  };
  for (const l of list) {
    if (l.ig) s.positioned++;
    s.byStatus[l.irlStatus]++;
    for (const t of l.tags) s.byTag[t] = (s.byTag[t] ?? 0) + 1;
    const d = l.district ?? '(none)';
    s.byDistrict[d] = (s.byDistrict[d] ?? 0) + 1;
  }
  return s;
}
