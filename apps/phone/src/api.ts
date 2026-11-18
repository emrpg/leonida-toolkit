import type {
  CollectibleItem,
  Landmark,
  PhoneContact,
  PhoneMessage,
  PhoneNote,
  RouteResult,
  SnapmaticPhoto,
  TileManifest,
} from './types';

const TIMEOUT_MS = 3000;

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

function readLocal<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function writeLocal(key: string, val: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(val));
  } catch {
    // ignore
  }
}

// ---------- Map Data ----------
export async function fetchLandmarks(): Promise<Landmark[]> {
  try {
    const raw = await fetchJson<unknown>('/api/landmarks');
    const arr = unwrap<Landmark>(raw, ['landmarks', 'data']);
    if (arr.length) return arr;
  } catch {
    // fallback
  }
  return [];
}

export async function fetchTileManifest(): Promise<TileManifest> {
  try {
    return await fetchJson<TileManifest>('/api/tile-manifest');
  } catch {
    return {};
  }
}

export async function fetchRoute(x1: number, y1: number, x2: number, y2: number): Promise<RouteResult | null> {
  try {
    const raw = await fetchJson<RouteResult & { distanceM?: number; driveMin?: number; flyMin?: number }>(
      `/api/route?x1=${x1}&y1=${y1}&x2=${x2}&y2=${y2}`,
    );
    return {
      waypoints: raw.waypoints ?? [],
      distance_m: raw.distance_m ?? raw.distanceM ?? 0,
      drive_min: raw.drive_min ?? raw.driveMin ?? 0,
      fly_min: raw.fly_min ?? raw.flyMin ?? 0,
    };
  } catch {
    return null;
  }
}

// ---------- Checklist ----------
const COMP_KEY = 'sol.companion.checklist.v1';
const DEFAULT_COMP: CollectibleItem[] = [
  { id: 'col-1', title: 'Ocean Drive 霓虹明信片', category: '明信片', district: 'Vice Beach', x: 1860, y: 1190, completed: false },
  { id: 'col-2', title: 'Venetian Causeway 隐藏包裹', category: '隐藏包裹', district: 'Vice Beach', x: 750, y: 860, completed: false },
  { id: 'col-3', title: 'Port Vice 货运飞跃点', category: '特技飞跃', district: 'Port Vice', x: -120, y: -420, completed: false },
  { id: 'col-4', title: 'Wynwood 涂鸦墙打卡', category: '街头涂鸦', district: 'Wynwood', x: -1450, y: 1180, completed: false },
];

export async function fetchChecklist(): Promise<CollectibleItem[]> {
  try {
    const raw = await fetchJson<unknown>('/api/companion/checklist');
    const items = unwrap<CollectibleItem>(raw, ['items', 'checklist', 'data']);
    if (items.length) {
      writeLocal(COMP_KEY, items);
      return items;
    }
  } catch {
    // fallback
  }
  return readLocal(COMP_KEY, DEFAULT_COMP);
}

export async function toggleChecklist(id: string, completed: boolean, cur: CollectibleItem[]): Promise<CollectibleItem[]> {
  const next = cur.map((x) => (x.id === id ? { ...x, completed } : x));
  writeLocal(COMP_KEY, next);
  try {
    await fetchJson<unknown>('/api/companion/toggle', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id, completed }),
    });
  } catch {
    // offline
  }
  return next;
}

// ---------- Contacts & Messages ----------
const MSG_KEY = 'phone.messages.v1';
const DEFAULT_CONTACTS: PhoneContact[] = [
  { id: 'lucia', name: 'Lucia', role: '线人 · Vice Beach', district: 'Vice Beach', avatar_color: '#ff4fa3', bio: '熟悉 Ocean Drive 与南部码头动向。' },
  { id: 'jason', name: 'Jason', role: '勘察员 · Port Vice', district: 'Port Vice', avatar_color: '#00e5ff', bio: '专注港口与机场摄影测量标定。' },
  { id: 'dre', name: 'DreQuan', role: '情报商 · Wynwood', district: 'Wynwood', avatar_color: '#ffb347', bio: '掌握社区最新 Leaks 与涂鸦收集点。' },
];
const DEFAULT_MESSAGES: PhoneMessage[] = [
  { id: 'm1', contact_id: 'lucia', sender: 'Lucia', text: 'Ocean View Hotel 附近刚刷新了新地标坐标，记得用 Radar 标记一下。', timestamp: '18:10', unread: true },
  { id: 'm2', contact_id: 'jason', sender: 'Jason', text: 'Port Vice 起重机高度已录入 3D 库，可以去试试路线规划。', timestamp: '18:22', unread: false },
];

export async function fetchContacts(): Promise<PhoneContact[]> {
  try {
    const raw = await fetchJson<unknown>('/api/phone/contacts');
    const arr = unwrap<PhoneContact>(raw, ['contacts', 'data']);
    if (arr.length) return arr;
  } catch {
    // fallback
  }
  return DEFAULT_CONTACTS;
}

export async function fetchMessages(contactId: string): Promise<PhoneMessage[]> {
  try {
    const raw = await fetchJson<unknown>(`/api/phone/messages?contact_id=${encodeURIComponent(contactId)}`);
    const arr = unwrap<PhoneMessage>(raw, ['messages', 'data']);
    if (Array.isArray(raw) || (raw && typeof raw === 'object' && 'messages' in raw)) {
      return arr;
    }
  } catch {
    // fallback
  }
  return readLocal<PhoneMessage[]>(MSG_KEY, DEFAULT_MESSAGES).filter((m) => m.contact_id === contactId);
}

export async function sendMessage(contactId: string, text: string): Promise<PhoneMessage> {
  const local: PhoneMessage = {
    id: crypto.randomUUID(),
    contact_id: contactId,
    sender: 'Me',
    text,
    timestamp: new Date().toTimeString().slice(0, 5),
    unread: false,
  };
  const all = readLocal<PhoneMessage[]>(MSG_KEY, DEFAULT_MESSAGES);
  writeLocal(MSG_KEY, [...all, local]);
  try {
    await fetchJson<unknown>('/api/phone/messages', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ contact_id: contactId, sender: 'Me', text }),
    });
  } catch {
    // offline
  }
  return local;
}

// ---------- Snapmatic Photos ----------
const PHOTO_KEY = 'phone.photos.v1';
export async function fetchPhotos(): Promise<SnapmaticPhoto[]> {
  try {
    const raw = await fetchJson<unknown>('/api/phone/photos');
    const arr = unwrap<SnapmaticPhoto>(raw, ['photos', 'data']);
    if (Array.isArray(raw) || (raw && typeof raw === 'object' && 'photos' in raw)) {
      writeLocal(PHOTO_KEY, arr);
      return arr;
    }
  } catch {
    // fallback
  }
  return readLocal<SnapmaticPhoto[]>(PHOTO_KEY, []);
}

export async function savePhoto(photo: Omit<SnapmaticPhoto, 'id' | 'created_at'>): Promise<SnapmaticPhoto> {
  const local: SnapmaticPhoto = {
    ...photo,
    id: crypto.randomUUID(),
    created_at: new Date().toISOString(),
  };
  writeLocal(PHOTO_KEY, [local, ...readLocal<SnapmaticPhoto[]>(PHOTO_KEY, [])]);
  try {
    const res = await fetchJson<{ photo?: SnapmaticPhoto } & Partial<SnapmaticPhoto>>('/api/phone/photos', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(local),
    });
    return res.photo ?? (res.id ? (res as SnapmaticPhoto) : local);
  } catch {
    return local;
  }
}

export async function deletePhoto(id: string): Promise<void> {
  writeLocal(PHOTO_KEY, readLocal<SnapmaticPhoto[]>(PHOTO_KEY, []).filter((p) => p.id !== id));
  try {
    await fetch(`/api/phone/photos?id=${encodeURIComponent(id)}`, { method: 'DELETE' });
  } catch {
    // offline
  }
}

// ---------- Notes ----------
const NOTE_KEY = 'phone.notes.v1';
export async function fetchNotes(): Promise<PhoneNote[]> {
  try {
    const raw = await fetchJson<unknown>('/api/phone/notes');
    const arr = unwrap<PhoneNote>(raw, ['notes', 'data']);
    if (Array.isArray(raw) || (raw && typeof raw === 'object' && 'notes' in raw)) {
      writeLocal(NOTE_KEY, arr);
      return arr;
    }
  } catch {
    // fallback
  }
  return readLocal<PhoneNote[]>(NOTE_KEY, []);
}

export async function saveNote(note: Omit<PhoneNote, 'id' | 'updated_at'>): Promise<PhoneNote> {
  const local: PhoneNote = {
    ...note,
    id: crypto.randomUUID(),
    updated_at: new Date().toISOString(),
  };
  writeLocal(NOTE_KEY, [local, ...readLocal<PhoneNote[]>(NOTE_KEY, [])]);
  try {
    const res = await fetchJson<{ note?: PhoneNote } & Partial<PhoneNote>>('/api/phone/notes', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(local),
    });
    return res.note ?? (res.id ? (res as PhoneNote) : local);
  } catch {
    return local;
  }
}

export async function deleteNote(id: string): Promise<void> {
  writeLocal(NOTE_KEY, readLocal<PhoneNote[]>(NOTE_KEY, []).filter((n) => n.id !== id));
  try {
    await fetch(`/api/phone/notes?id=${encodeURIComponent(id)}`, { method: 'DELETE' });
  } catch {
    // offline
  }
}

export async function submitPhoneChallenge(payload: {
  player: string;
  scene_id: string;
  score: number;
  distance_m: number;
  grade: string;
}): Promise<boolean> {
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
