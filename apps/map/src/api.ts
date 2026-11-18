import type {
  CameraObs,
  CameraPose,
  ChangelogItem,
  GameStats,
  Gta6MapTuple,
  GtamaplibVcFile,
  Landmark,
  LeaderboardEntry,
  MediaTabInfo,
  NewsItem,
  NotificationItem,
  PhotogrammetryProject,
  RankInfo,
  RouteResult,
  SolDatasetKey,
  SolEntry,
  TileManifest,
  UserMarker,
  UserMarkersExport,
} from './types';

const MINE_KEY = 'leonida.userMarkers.v1';
const LOCAL_LB_KEY = 'sol.challenge.leaderboard.v1';
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

// ---------- SOL Datasets (data/*.json) ----------
function unwrapArray<T>(raw: unknown, keys: string[]): T[] {
  if (Array.isArray(raw)) return raw as T[];
  if (raw && typeof raw === 'object') {
    const rec = raw as Record<string, unknown>;
    for (const k of keys) {
      if (Array.isArray(rec[k])) return rec[k] as T[];
    }
  }
  return [];
}

export async function fetchSolDataset(key: SolDatasetKey): Promise<SolEntry[]> {
  try {
    const raw = await fetchJson<unknown>(`data/${key}.json`);
    return unwrapArray<SolEntry>(raw, ['entries', 'items', 'markers', 'data']).map((e) => ({
      ...e,
      dataset: e.dataset || key,
    }));
  } catch {
    return [];
  }
}

export async function fetchMediaTabs(): Promise<MediaTabInfo[]> {
  try {
    const raw = await fetchJson<unknown>('/get-media-files.php?action=tabs');
    const arr = unwrapArray<MediaTabInfo>(raw, ['tabs', 'data']);
    if (arr.length) return arr;
  } catch {
    // fallback
  }
  return [
    { key: 'trailer1', label: 'Trailer 1' },
    { key: 'trailer2', label: 'Trailer 2' },
    { key: 'trailer3', label: 'Trailer 3' },
    { key: 'leaks', label: 'Leaks' },
    { key: 'screenshots', label: 'Screenshots' },
  ];
}

export async function fetchMediaFiles(type: string): Promise<SolEntry[]> {
  try {
    const raw = await fetchJson<unknown>(`/get-media-files.php?type=${encodeURIComponent(type)}`);
    const arr = unwrapArray<SolEntry>(raw, ['files', 'entries', 'items', 'data']);
    if (arr.length) return arr;
  } catch {
    // fallback to static dataset
  }
  return fetchSolDataset(type as SolDatasetKey);
}

export async function fetchNews(): Promise<NewsItem[]> {
  try {
    const raw = await fetchJson<unknown>('data/news.json');
    return unwrapArray<NewsItem>(raw, ['news', 'items', 'data']);
  } catch {
    return [];
  }
}

export async function fetchChangelog(): Promise<ChangelogItem[]> {
  try {
    const raw = await fetchJson<unknown>('data/database_changelog.json');
    return unwrapArray<ChangelogItem>(raw, ['changelog', 'entries', 'items', 'data']);
  } catch {
    return [];
  }
}

export async function fetchCredits(): Promise<{ credits: string[]; supporters: string[] }> {
  const [cRaw, sRaw] = await Promise.all([
    fetchJson<unknown>('data/credits.json').catch(() => []),
    fetchJson<unknown>('data/supporters.json').catch(() => []),
  ]);
  const toNames = (raw: unknown): string[] => {
    const arr = unwrapArray<unknown>(raw, ['credits', 'supporters', 'items', 'data']);
    return arr.map((x) => (typeof x === 'string' ? x : ((x as { name?: string })?.name ?? JSON.stringify(x))));
  };
  return { credits: toNames(cRaw), supporters: toNames(sRaw) };
}

// ---------- Challenge / Leaderboard / Ranks / Notifications ----------
function loadLocalLeaderboard(): LeaderboardEntry[] {
  try {
    const raw = localStorage.getItem(LOCAL_LB_KEY);
    return raw ? (JSON.parse(raw) as LeaderboardEntry[]) : [];
  } catch {
    return [];
  }
}

function saveLocalLeaderboard(items: LeaderboardEntry[]): void {
  try {
    localStorage.setItem(LOCAL_LB_KEY, JSON.stringify(items.slice(0, 20)));
  } catch {
    // ignore
  }
}

export async function fetchLeaderboard(limit = 10): Promise<LeaderboardEntry[]> {
  try {
    const raw = await fetchJson<unknown>(
      `/auth/api.php?action=leaderboard&game=screenshot_challenge&limit=${limit}`,
    );
    const arr = unwrapArray<LeaderboardEntry>(raw, ['leaderboard', 'entries', 'data']);
    if (arr.length) return arr;
  } catch {
    // fallback
  }
  return loadLocalLeaderboard();
}

export async function fetchGameStats(): Promise<GameStats> {
  try {
    const raw = await fetchJson<GameStats | { stats: GameStats }>(
      '/auth/api.php?action=game_stats&game=screenshot_challenge',
    );
    return 'stats' in raw && raw.stats ? raw.stats : (raw as GameStats);
  } catch {
    const local = loadLocalLeaderboard();
    if (!local.length) return { total_plays: 0, best_score: 0, avg_score: 0 };
    const best = Math.max(...local.map((x) => x.score));
    const avg = Math.round(local.reduce((s, x) => s + x.score, 0) / local.length);
    return { total_plays: local.length, best_score: best, avg_score: avg };
  }
}

export async function submitChallengeScore(payload: {
  player: string;
  scene_id: string;
  score: number;
  distance_m: number;
  grade: string;
}): Promise<boolean> {
  const local = loadLocalLeaderboard();
  local.push({
    player: payload.player,
    score: payload.score,
    distance_m: payload.distance_m,
    grade: payload.grade,
    created_at: new Date().toISOString(),
  });
  local.sort((a, b) => b.score - a.score);
  saveLocalLeaderboard(local);

  try {
    await fetchJson<unknown>('/api/sol/challenge/submit', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    return true;
  } catch {
    return false;
  }
}

export async function fetchRanks(): Promise<RankInfo[]> {
  try {
    const raw = await fetchJson<unknown>('/auth/api.php?action=ranks');
    const arr = unwrapArray<RankInfo>(raw, ['ranks', 'data']);
    if (arr.length) return arr;
  } catch {
    // fallback
  }
  return [
    { name: 'Leonida Legend (S)', min_score: 4500, color: '#ff4fa3' },
    { name: 'Vice Cartographer (A)', min_score: 3500, color: '#ffb347' },
    { name: 'Scout (B)', min_score: 2200, color: '#00e5ff' },
    { name: 'Tourist (C)', min_score: 1000, color: '#22c55e' },
    { name: 'Lost in the Keys (D)', min_score: 0, color: '#8b8fa3' },
  ];
}

export async function fetchNotifications(): Promise<NotificationItem[]> {
  try {
    const raw = await fetchJson<unknown>('/auth/api.php?action=notifications');
    return unwrapArray<NotificationItem>(raw, ['notifications', 'items', 'data']);
  } catch {
    return [];
  }
}

export async function markNotificationsRead(): Promise<void> {
  try {
    await fetchJson<unknown>('/auth/api.php?action=notifications_read', { method: 'POST' });
  } catch {
    // ignore offline
  }
}

// ---------- LocalStorage Helpers ----------
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

// ---------- User Marker API client (auth/api.php + /api/markers + localStorage fallback) ----------
export class MarkerClient {
  private online = false;
  private useAuthPhp = false;

  isOnline(): boolean {
    return this.online;
  }

  async list(): Promise<UserMarker[]> {
    // 1. Try State of Leonida auth/api.php?action=marker_list
    try {
      const raw = await fetchJson<unknown>('/auth/api.php?action=marker_list');
      const markers = unwrapArray<UserMarker>(raw, ['markers', 'data']);
      if (Array.isArray(raw) || (raw && typeof raw === 'object' && 'markers' in raw)) {
        this.online = true;
        this.useAuthPhp = true;
        saveLocalMarkers(markers);
        return markers;
      }
    } catch {
      // fall through to /api/markers
    }

    // 2. Try REST /api/markers
    try {
      const data = await fetchJson<UserMarker[] | { markers: UserMarker[] }>('/api/markers');
      const markers = Array.isArray(data) ? data : data.markers;
      if (Array.isArray(markers)) {
        this.online = true;
        this.useAuthPhp = false;
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
        const url = this.useAuthPhp ? '/auth/api.php?action=marker_save' : '/api/markers';
        const res = await fetchJson<UserMarker | { marker: UserMarker }>(url, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(marker),
        });
        const created = 'marker' in res && res.marker ? res.marker : (res as UserMarker);
        const item = created?.id ? created : marker;
        const next = [...loadLocalMarkers().filter((m) => m.id !== item.id), item];
        saveLocalMarkers(next);
        return item;
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
        const url = this.useAuthPhp
          ? '/auth/api.php?action=marker_update'
          : `/api/markers/${encodeURIComponent(marker.id)}`;
        const res = await fetchJson<UserMarker | { marker: UserMarker }>(url, {
          method: this.useAuthPhp ? 'POST' : 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(marker),
        });
        const updated = 'marker' in res && res.marker ? res.marker : (res as UserMarker);
        const item = updated?.id ? updated : marker;
        const next = loadLocalMarkers().map((m) => (m.id === item.id ? item : m));
        saveLocalMarkers(next);
        return item;
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
        if (this.useAuthPhp) {
          await fetchJson<unknown>('/auth/api.php?action=marker_delete', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ id }),
          });
        } else {
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
        }
      } catch {
        this.online = false;
      }
    }
    saveLocalMarkers(loadLocalMarkers().filter((m) => m.id !== id));
  }

  async importBatch(markers: UserMarker[]): Promise<UserMarker[]> {
    if (this.online && !this.useAuthPhp) {
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
        // fall through
      }
    }
    if (this.online) {
      for (const m of markers) {
        await this.create(m);
      }
      return this.list();
    }
    const merged = [...loadLocalMarkers(), ...markers];
    saveLocalMarkers(merged);
    return merged;
  }

  async clearAll(current: readonly UserMarker[]): Promise<void> {
    if (this.online && !this.useAuthPhp) {
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
        for (const m of current) await this.remove(m.id);
      }
    } else if (this.online) {
      for (const m of current) await this.remove(m.id);
    }
    saveLocalMarkers([]);
  }

  async exportJson(current: readonly UserMarker[]): Promise<UserMarkersExport> {
    if (this.online && !this.useAuthPhp) {
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

// ---------- v0.4 Photogrammetry & Route APIs ----------
const LOCAL_PHOTO_KEY = 'sol.photogrammetry.projects.v1';

function loadLocalProjects(): PhotogrammetryProject[] {
  try {
    const raw = localStorage.getItem(LOCAL_PHOTO_KEY);
    return raw ? (JSON.parse(raw) as PhotogrammetryProject[]) : [];
  } catch {
    return [];
  }
}

function saveLocalProjects(items: PhotogrammetryProject[]): void {
  try {
    localStorage.setItem(LOCAL_PHOTO_KEY, JSON.stringify(items));
  } catch {
    // ignore
  }
}

export async function solveCameraPoseRemote(
  observations: CameraObs[],
  width: number,
  height: number,
  fov_deg = 60,
): Promise<{ pose: CameraPose; frustum?: [number, number][] } | null> {
  try {
    return await fetchJson<{ pose: CameraPose; frustum?: [number, number][] }>(
      '/api/photogrammetry/solve',
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ observations, width, height, fov_deg }),
      },
    );
  } catch {
    return null;
  }
}

export async function listPhotogrammetryProjects(): Promise<PhotogrammetryProject[]> {
  try {
    const raw = await fetchJson<unknown>('/api/photogrammetry/projects');
    const arr = unwrapArray<PhotogrammetryProject>(raw, ['projects', 'items', 'data']);
    if (Array.isArray(raw) || (raw && typeof raw === 'object' && 'projects' in raw)) {
      saveLocalProjects(arr);
      return arr;
    }
  } catch {
    // fallback
  }
  return loadLocalProjects();
}

export async function savePhotogrammetryProject(
  project: PhotogrammetryProject,
): Promise<PhotogrammetryProject> {
  const localItem: PhotogrammetryProject = {
    ...project,
    id: project.id || crypto.randomUUID(),
    created_at: project.created_at || new Date().toISOString(),
  };
  try {
    const res = await fetchJson<PhotogrammetryProject | { project: PhotogrammetryProject }>(
      '/api/photogrammetry/projects',
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(localItem),
      },
    );
    const saved = 'project' in res && res.project ? res.project : (res as PhotogrammetryProject);
    const item = saved?.id ? saved : localItem;
    saveLocalProjects([...loadLocalProjects().filter((p) => p.id !== item.id), item]);
    return item;
  } catch {
    saveLocalProjects([...loadLocalProjects().filter((p) => p.id !== localItem.id), localItem]);
    return localItem;
  }
}

export async function deletePhotogrammetryProject(id: string): Promise<void> {
  try {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), API_TIMEOUT_MS);
    try {
      await fetch(`/api/photogrammetry/projects/${encodeURIComponent(id)}`, {
        method: 'DELETE',
        signal: controller.signal,
      });
    } finally {
      clearTimeout(timer);
    }
  } catch {
    // ignore offline
  }
  saveLocalProjects(loadLocalProjects().filter((p) => p.id !== id));
}

export async function fetchRouteRemote(
  x1: number,
  y1: number,
  x2: number,
  y2: number,
): Promise<RouteResult | null> {
  try {
    return await fetchJson<RouteResult>(
      `/api/route?x1=${encodeURIComponent(x1)}&y1=${encodeURIComponent(y1)}&x2=${encodeURIComponent(x2)}&y2=${encodeURIComponent(y2)}`,
    );
  } catch {
    return null;
  }
}
