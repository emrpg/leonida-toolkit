import type { ClusterCustomColors, MapSettings, StudioSettings } from './types';

const KEYS = {
  mapSettings: 'mapSettings',
  clusterCustomColors: 'clusterCustomColors',
  markerCustomColor: 'markerCustomColor',
  bgEffects: 'bgEffects',
} as const;

export const DEFAULT_SETTINGS: StudioSettings = {
  mapSettings: {
    showGrid: true,
    showLabels: true,
    show3dByDefault: false,
    autoFly: true,
  },
  clusterCustomColors: {
    landmarks: '#ff4fa3',
    trailer1: '#ffb347',
    trailer2: '#00e5ff',
    trailer3: '#a855f7',
    leaks: '#ef4444',
    screenshots: '#22c55e',
  },
  markerCustomColor: '#ff4fa3',
  bgEffects: true,
};

function readJson<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return fallback;
    return { ...fallback, ...(JSON.parse(raw) as Partial<T>) };
  } catch {
    return fallback;
  }
}

export function loadStudioSettings(): StudioSettings {
  let bgEffects = DEFAULT_SETTINGS.bgEffects;
  try {
    const rawBg = localStorage.getItem(KEYS.bgEffects);
    if (rawBg !== null) bgEffects = rawBg === 'true';
  } catch {
    // ignore
  }
  let markerColor = DEFAULT_SETTINGS.markerCustomColor;
  try {
    const rawMc = localStorage.getItem(KEYS.markerCustomColor);
    if (rawMc) markerColor = rawMc;
  } catch {
    // ignore
  }
  return {
    mapSettings: readJson<MapSettings>(KEYS.mapSettings, DEFAULT_SETTINGS.mapSettings),
    clusterCustomColors: readJson<ClusterCustomColors>(
      KEYS.clusterCustomColors,
      DEFAULT_SETTINGS.clusterCustomColors,
    ),
    markerCustomColor: markerColor,
    bgEffects,
  };
}

export function saveStudioSettings(s: StudioSettings): void {
  try {
    localStorage.setItem(KEYS.mapSettings, JSON.stringify(s.mapSettings));
    localStorage.setItem(KEYS.clusterCustomColors, JSON.stringify(s.clusterCustomColors));
    localStorage.setItem(KEYS.markerCustomColor, s.markerCustomColor);
    localStorage.setItem(KEYS.bgEffects, String(s.bgEffects));
  } catch {
    // ignore quota errors
  }
}
