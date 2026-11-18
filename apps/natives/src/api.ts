import type { NativeBookmark, NativeEntry, NativeNamespaceStat } from './types';

const TIMEOUT_MS = 3000;
const BM_KEY = 'natives.bookmarks.v1';

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const c = new AbortController();
  const t = setTimeout(() => c.abort(), TIMEOUT_MS);
  try {
    const res = await fetch(url, { ...init, signal: c.signal });
    if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
    return (await res.json()) as T;
  } finally {
    clearTimeout(t);
  }
}

function unwrap<T>(raw: unknown, keys: string[]): T[] {
  if (Array.isArray(raw)) return raw as T[];
  if (raw && typeof raw === 'object') {
    for (const k of keys) {
      const v = (raw as Record<string, unknown>)[k];
      if (Array.isArray(v)) return v as T[];
    }
  }
  return [];
}

const FALLBACK_NATIVES: NativeEntry[] = [
  { namespace: 'PLAYER', name: 'PLAYER_PED_ID', hash: '0xD80958FC74E988A6', jhash: '0xFA92E226', params: [], return_type: 'Ped', description: 'Returns the current player ped handle.' },
  { namespace: 'PLAYER', name: 'SET_PLAYER_WANTED_LEVEL', hash: '0x39FF19C64EF7DA5B', jhash: '0xB7A0914B', params: [{ name: 'player', param_type: 'Player' }, { name: 'wantedLevel', param_type: 'int' }, { name: 'disableNoMission', param_type: 'BOOL' }], return_type: 'void', description: 'Sets the player wanted level (0-5).' },
  { namespace: 'ENTITY', name: 'GET_ENTITY_COORDS', hash: '0x3FEF770D40960D5A', jhash: '0x1647F1CB', params: [{ name: 'entity', param_type: 'Entity' }, { name: 'alive', param_type: 'BOOL' }], return_type: 'Vector3', description: 'Gets the world coordinates of an entity.' },
  { namespace: 'ENTITY', name: 'SET_ENTITY_COORDS', hash: '0x06843DA7060A026B', jhash: '0xDF70B41B', params: [{ name: 'entity', param_type: 'Entity' }, { name: 'xPos', param_type: 'float' }, { name: 'yPos', param_type: 'float' }, { name: 'zPos', param_type: 'float' }, { name: 'xAxis', param_type: 'BOOL' }, { name: 'yAxis', param_type: 'BOOL' }, { name: 'zAxis', param_type: 'BOOL' }, { name: 'clearArea', param_type: 'BOOL' }], return_type: 'void', description: 'Teleports an entity to world coordinates.' },
  { namespace: 'VEHICLE', name: 'CREATE_VEHICLE', hash: '0xAF35D0D2583051B0', jhash: '0xDD75460A', params: [{ name: 'modelHash', param_type: 'Hash' }, { name: 'x', param_type: 'float' }, { name: 'y', param_type: 'float' }, { name: 'z', param_type: 'float' }, { name: 'heading', param_type: 'float' }, { name: 'isNetwork', param_type: 'BOOL' }, { name: 'bScriptHostVeh', param_type: 'BOOL' }], return_type: 'Vehicle', description: 'Spawns a vehicle at the given coordinates.' },
  { namespace: 'PED', name: 'SET_PED_INTO_VEHICLE', hash: '0xF75B0D629E1C063D', jhash: '0x07500C79', params: [{ name: 'ped', param_type: 'Ped' }, { name: 'vehicle', param_type: 'Vehicle' }, { name: 'seatIndex', param_type: 'int' }], return_type: 'void', description: 'Places a ped into a vehicle seat.' },
  { namespace: 'HUD', name: 'SET_NEW_WAYPOINT', hash: '0xFE43368D2AA4F2FC', jhash: '0x8A76E1F2', params: [{ name: 'x', param_type: 'float' }, { name: 'y', param_type: 'float' }], return_type: 'void', description: 'Sets the radar GPS waypoint.' },
  { namespace: 'CAM', name: 'GET_GAMEPLAY_CAM_COORD', hash: '0x14D6F5678D8F1B37', jhash: '0x9388CF79', params: [], return_type: 'Vector3', description: 'Returns the active gameplay camera position.' },
  { namespace: 'STREAMING', name: 'REQUEST_MODEL', hash: '0x963D27A58DF860AC', jhash: '0xFFF1B500', params: [{ name: 'model', param_type: 'Hash' }], return_type: 'void', description: 'Requests a model to be loaded into memory.' },
  { namespace: 'MISC', name: 'GET_HASH_KEY', hash: '0xD24D37CC275948CC', jhash: '0x98EFF6F1', params: [{ name: 'string', param_type: 'const char*' }], return_type: 'Hash', description: 'Computes the JOAAT hash of a string.' },
];

export async function loadAllNatives(): Promise<NativeEntry[]> {
  try {
    const raw = await fetchJson<unknown>('/api/natives?limit=8000');
    const arr = unwrap<NativeEntry>(raw, ['entries', 'natives', 'items', 'data']);
    if (arr.length) return arr;
  } catch {
    // try static data/natives.json
  }
  try {
    const raw = await fetchJson<unknown>('/natives/data/natives.json');
    const arr = unwrap<NativeEntry>(raw, ['entries', 'natives', 'items', 'data']);
    if (arr.length) return arr;
  } catch {
    // fallback
  }
  return FALLBACK_NATIVES;
}

export function computeNamespaceStats(entries: readonly NativeEntry[]): NativeNamespaceStat[] {
  const map = new Map<string, number>();
  for (const e of entries) {
    map.set(e.namespace, (map.get(e.namespace) ?? 0) + 1);
  }
  return [...map.entries()]
    .map(([namespace, count]) => ({ namespace, count }))
    .sort((a, b) => b.count - a.count || a.namespace.localeCompare(b.namespace));
}

function readLocalBookmarks(): NativeBookmark[] {
  try {
    const raw = localStorage.getItem(BM_KEY);
    return raw ? (JSON.parse(raw) as NativeBookmark[]) : [];
  } catch {
    return [];
  }
}

function writeLocalBookmarks(items: NativeBookmark[]): void {
  try {
    localStorage.setItem(BM_KEY, JSON.stringify(items));
  } catch {
    // ignore
  }
}

export async function fetchBookmarks(): Promise<NativeBookmark[]> {
  try {
    const raw = await fetchJson<unknown>('/api/natives/bookmarks');
    const arr = unwrap<NativeBookmark>(raw, ['bookmarks', 'items', 'data']);
    if (Array.isArray(raw) || (raw && typeof raw === 'object' && 'bookmarks' in raw)) {
      writeLocalBookmarks(arr);
      return arr;
    }
  } catch {
    // offline
  }
  return readLocalBookmarks();
}

export async function saveBookmark(bm: NativeBookmark): Promise<NativeBookmark[]> {
  const next = [bm, ...readLocalBookmarks().filter((x) => x.hash !== bm.hash)];
  writeLocalBookmarks(next);
  try {
    await fetchJson<unknown>('/api/natives/bookmarks', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(bm),
    });
  } catch {
    // offline
  }
  return next;
}

export async function deleteBookmark(hash: string): Promise<NativeBookmark[]> {
  const next = readLocalBookmarks().filter((x) => x.hash !== hash);
  writeLocalBookmarks(next);
  try {
    await fetch(`/api/natives/bookmarks?hash=${encodeURIComponent(hash)}`, { method: 'DELETE' });
  } catch {
    // offline
  }
  return next;
}
