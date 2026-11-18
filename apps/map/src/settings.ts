import type {
  AccentPreset,
  ClusterCustomColors,
  ListDensity,
  MapSettings,
  PanelStyle,
  StudioSettings,
} from './types';

const KEYS = {
  mapSettings: 'mapSettings',
  clusterCustomColors: 'clusterCustomColors',
  markerCustomColor: 'markerCustomColor',
  bgEffects: 'bgEffects',
  isPreloaderStyle: 'isPreloaderStyle',
  accentColor: 'accentColor',
  panelStyle: 'panelStyle',
  listDensity: 'listDensity',
  selectedBackgroundColor: 'selectedBackgroundColor',
} as const;

export const ACCENT_HEX: Record<AccentPreset, string> = {
  pink: '#ff4fa3',
  cyan: '#00e5ff',
  gold: '#ffb347',
  emerald: '#22c55e',
};

export const DEFAULT_SETTINGS: StudioSettings = {
  mapSettings: {
    showGrid: true,
    showLabels: true,
    show3dByDefault: false,
    autoFly: true,
    heatmapEnabled: false,
    gridEnabled: false,
    clusteringEnabled: false,
    markerMotionEnabled: false,
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
  isPreloaderStyle: true,
  accentColor: 'pink',
  panelStyle: 'glass',
  listDensity: 'comfortable',
  selectedBackgroundColor: '#0c0e16',
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

function readStr<T extends string>(key: string, fallback: T): T {
  try {
    return (localStorage.getItem(key) as T | null) ?? fallback;
  } catch {
    return fallback;
  }
}

export function loadStudioSettings(): StudioSettings {
  let bgEffects = DEFAULT_SETTINGS.bgEffects;
  let isPreloaderStyle = DEFAULT_SETTINGS.isPreloaderStyle;
  try {
    const rawBg = localStorage.getItem(KEYS.bgEffects);
    if (rawBg !== null) bgEffects = rawBg === 'true';
    const rawPre = localStorage.getItem(KEYS.isPreloaderStyle);
    if (rawPre !== null) isPreloaderStyle = rawPre === 'true';
  } catch {
    // ignore
  }
  return {
    mapSettings: readJson<MapSettings>(KEYS.mapSettings, DEFAULT_SETTINGS.mapSettings),
    clusterCustomColors: readJson<ClusterCustomColors>(
      KEYS.clusterCustomColors,
      DEFAULT_SETTINGS.clusterCustomColors,
    ),
    markerCustomColor: readStr(KEYS.markerCustomColor, DEFAULT_SETTINGS.markerCustomColor),
    bgEffects,
    isPreloaderStyle,
    accentColor: readStr<AccentPreset>(KEYS.accentColor, DEFAULT_SETTINGS.accentColor),
    panelStyle: readStr<PanelStyle>(KEYS.panelStyle, DEFAULT_SETTINGS.panelStyle),
    listDensity: readStr<ListDensity>(KEYS.listDensity, DEFAULT_SETTINGS.listDensity),
    selectedBackgroundColor: readStr(
      KEYS.selectedBackgroundColor,
      DEFAULT_SETTINGS.selectedBackgroundColor,
    ),
  };
}

export function saveStudioSettings(s: StudioSettings): void {
  try {
    localStorage.setItem(KEYS.mapSettings, JSON.stringify(s.mapSettings));
    localStorage.setItem(KEYS.clusterCustomColors, JSON.stringify(s.clusterCustomColors));
    localStorage.setItem(KEYS.markerCustomColor, s.markerCustomColor);
    localStorage.setItem(KEYS.bgEffects, String(s.bgEffects));
    localStorage.setItem(KEYS.isPreloaderStyle, String(s.isPreloaderStyle));
    localStorage.setItem(KEYS.accentColor, s.accentColor);
    localStorage.setItem(KEYS.panelStyle, s.panelStyle);
    localStorage.setItem(KEYS.listDensity, s.listDensity);
    localStorage.setItem(KEYS.selectedBackgroundColor, s.selectedBackgroundColor);
  } catch {
    // ignore quota errors
  }
}
