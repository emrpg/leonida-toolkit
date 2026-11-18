import type {
  Gta6MapTuple,
  GtamaplibVcFile,
  Landmark,
  TileManifest,
  UserMarker,
  UserMarkersExport,
} from './types';

const MINE_KEY = 'leonida.userMarkers.v1';
const API_TIMEOUT_MS = 3000;

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), API_TIMEOUT_MS);
  try {
    const res = await fetch(url, { ...init, signal: controller.signal });
    if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
    return (await res.json()) as T;
  } finally {
    clearTimeout(timer);
  }
}

export type LandmarksPayload =
  | { kind: 'normalized'; landmarks: Landmark[]; fromServer: boolean }
  | { kind: 'tuples'; rows: Gta6MapTuple[]; fromServer: false };

export async function fetchLandmarksData(): Promise<LandmarksPayload> {
  try {
    const data = await fetchJson<Landmark[] | { landmarks: Landmark[] }>('/api/landmarks');
    const landmarks = Array.isArray(data) ? data : data.landmarks;
    if (Array.isArray(landmarks)) {
      return { kind: 'normalized', landmarks, fromServer: true };
    }
  } catch {
    // Offline or static preview: fall back to static JSON tuples
  }
  const rows = await fetchJson<Gta6MapTuple[]>('data/gta6map-landmarks.json');
  return { kind: 'tuples', rows, fromServer: false };
}

export async function fetchTileManifest(): Promise<TileManifest> {
  try {
    return await fetchJson<TileManifest>('/api/tile-manifest');
  } catch {
    return await fetchJson<TileManifest>('data/tile-manifest.json').catch(() => ({}) as TileManifest);
  }
}

export async function fetchLandmarks3D(): Promise<GtamaplibVcFile | null> {
  return fetchJson<GtamaplibVcFile>('data/gtamaplib-vc-landmarks.json').catch(() => null);
}

// ---------- LocalStorage helpers ----------
export function loadLocalMarkers(): UserMarker[] {
  try {
    const raw = localStorage.getItem(MINE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    return Array.isArray(parsed) ? (parsed as UserMarker[]) : [];
  } catch {
    return [];
  }
}

export function saveLocalMarkers(markers: readonly UserMarker[]): void {
  try {
    localStorage.setItem(MINE_KEY, JSON.stringify(markers));
  } catch {
    // Ignore storage quota errors
  }
}

// ---------- User Marker API client with localStorage fallback ----------
export class MarkerClient {
  private online = false;

  isOnline(): boolean {
    return this.online;
  }

  async list(): Promise<UserMarker[]> {
    try {
      const data = await fetchJson<UserMarker[] | { markers: UserMarker[] }>('/api/markers');
      const markers = Array.isArray(data) ? data : data.markers;
      if (Array.isArray(markers)) {
        this.online = true;
        saveLocalMarkers(markers);
        return markers;
      }
    } catch {
      this.online = false;
    }
    return loadLocalMarkers();
  }

  async create(marker: UserMarker): Promise<UserMarker> {
    if (this.online) {
      try {
        const created = await fetchJson<UserMarker>('/api/markers', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(marker),
        });
        const next = [...loadLocalMarkers().filter((m) => m.id !== created.id), created];
        saveLocalMarkers(next);
        return created;
      } catch {
        this.online = false;
      }
    }
    const next = [...loadLocalMarkers(), marker];
    saveLocalMarkers(next);
    return marker;
  }

  async update(marker: UserMarker): Promise<UserMarker> {
    if (this.online) {
      try {
        const updated = await fetchJson<UserMarker>(`/api/markers/${encodeURIComponent(marker.id)}`, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(marker),
        });
        const next = loadLocalMarkers().map((m) => (m.id === updated.id ? updated : m));
        saveLocalMarkers(next);
        return updated;
      } catch {
        this.online = false;
      }
    }
    const next = loadLocalMarkers().map((m) => (m.id === marker.id ? marker : m));
    saveLocalMarkers(next);
    return marker;
  }

  async remove(id: string): Promise<void> {
    if (this.online) {
      try {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), API_TIMEOUT_MS);
        try {
          const res = await fetch(`/api/markers/${encodeURIComponent(id)}`, {
            method: 'DELETE',
            signal: controller.signal,
          });
          if (!res.ok) throw new Error(`HTTP ${res.status}`);
        } finally {
          clearTimeout(timer);
        }
      } catch {
        this.online = false;
      }
    }
    saveLocalMarkers(loadLocalMarkers().filter((m) => m.id !== id));
  }

  async importBatch(markers: UserMarker[]): Promise<UserMarker[]> {
    if (this.online) {
      try {
        const payload: UserMarkersExport = { schema: 'leonida-user-markers-v1', markers };
        const res = await fetchJson<UserMarker[] | { markers: UserMarker[] }>('/api/markers/import', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload),
        });
        const imported = Array.isArray(res) ? res : res.markers;
        if (Array.isArray(imported)) {
          saveLocalMarkers(imported);
          return imported;
        }
      } catch {
        // Fallback: try creating individually if /import isn't a separate endpoint
        try {
          for (const m of markers) {
            await this.create(m);
          }
          return await this.list();
        } catch {
          this.online = false;
        }
      }
    }
    const merged = [...loadLocalMarkers(), ...markers];
    saveLocalMarkers(merged);
    return merged;
  }

  async clearAll(current: readonly UserMarker[]): Promise<void> {
    if (this.online) {
      try {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), API_TIMEOUT_MS);
        try {
          const res = await fetch('/api/markers', {
            method: 'DELETE',
            signal: controller.signal,
          });
          if (!res.ok) throw new Error(`HTTP ${res.status}`);
        } finally {
          clearTimeout(timer);
        }
      } catch {
        try {
          for (const m of current) {
            await this.remove(m.id);
          }
        } catch {
          this.online = false;
        }
      }
    }
    saveLocalMarkers([]);
  }

  async exportJson(current: readonly UserMarker[]): Promise<UserMarkersExport> {
    if (this.online) {
      try {
        const data = await fetchJson<UserMarkersExport>('/api/markers/export');
        if (data && Array.isArray(data.markers)) return data;
      } catch {
        // Fall back to in-memory/local export
      }
    }
    return { schema: 'leonida-user-markers-v1', markers: [...current] };
  }
}
