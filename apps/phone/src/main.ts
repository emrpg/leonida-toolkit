import L from 'leaflet';
import 'leaflet/dist/leaflet.css';
import './style.css';
import {
  MAX_TILE_Z,
  TILE_SIZE,
  WORLD_EXTENT,
  formatDistance,
  type CollectibleItem,
  type Landmark,
  type PhoneContact,
  type WorldPoint,
} from './types';
import {
  deleteNote,
  deletePhoto,
  fetchChecklist,
  fetchContacts,
  fetchLandmarks,
  fetchMessages,
  fetchNotes,
  fetchPhotos,
  fetchRoute,
  fetchTileManifest,
  saveNote,
  savePhoto,
  sendMessage,
  submitPhoneChallenge,
  toggleChecklist,
} from './api';
import { initPhoneWasm } from './wasm';

const $ = <T extends HTMLElement = HTMLElement>(sel: string) => document.querySelector(sel) as T;
const esc = (s: unknown) =>
  String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);

const k = 1 / (WORLD_EXTENT.size / TILE_SIZE);
const LeonidaCRS = L.extend({}, L.CRS.Simple, {
  transformation: new L.Transformation(k, WORLD_EXTENT.half * k, -k, WORLD_EXTENT.half * k),
}) as L.CRS;
const toLatLng = (p: WorldPoint) => L.latLng(p.y, p.x);
const toWorld = (ll: L.LatLng): WorldPoint => ({ x: ll.lng, y: ll.lat });

function createTileLayer(
  resolver: { resolve: (b: string, c: string | null, z: number, x: number, y: number) => string | null; tilesets: () => string[] },
): L.GridLayer | null {
  const base = resolver.tilesets().includes('dupzor,51') ? 'dupzor,51' : resolver.tilesets()[0];
  if (!base) return null;
  const Layer = L.GridLayer.extend({
    createTile(coords: L.Coords, done: L.DoneCallback) {
      const img = document.createElement('img');
      img.alt = '';
      const set = resolver.resolve(base, null, coords.z, coords.x, coords.y);
      if (!set) {
        setTimeout(() => done(undefined, img), 0);
        return img;
      }
      img.onload = () => done(undefined, img);
      img.onerror = () => done(new Error('tile'), img);
      img.src = `/tiles/${set}/${coords.z}/${coords.z},${coords.y},${coords.x}.jpg`;
      return img;
    },
  });
  return new (Layer as unknown as new (o: L.GridLayerOptions) => L.GridLayer)({
    tileSize: TILE_SIZE,
    minNativeZoom: 0,
    maxNativeZoom: MAX_TILE_Z,
    minZoom: -1,
    maxZoom: MAX_TILE_Z + 2,
    noWrap: true,
    bounds: L.latLngBounds(
      toLatLng({ x: WORLD_EXTENT.minX, y: WORLD_EXTENT.minY }),
      toLatLng({ x: WORLD_EXTENT.maxX, y: WORLD_EXTENT.maxY }),
    ),
  });
}

function makeSamplePhotoDataUrl(title: string): string {
  const c = document.createElement('canvas');
  c.width = 480;
  c.height = 270;
  const ctx = c.getContext('2d')!;
  const g = ctx.createLinearGradient(0, 0, 0, 270);
  g.addColorStop(0, '#ff4fa3');
  g.addColorStop(0.55, '#6d28d9');
  g.addColorStop(1, '#090b14');
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, 480, 270);
  ctx.fillStyle = 'rgba(255,255,255,0.9)';
  ctx.font = 'bold 20px sans-serif';
  ctx.fillText(title || 'Snapmatic · Vice Beach', 20, 230);
  return c.toDataURL('image/png');
}

async function main() {
  // Live clock
  const updateClock = () => {
    $('#sb-time').textContent = new Date().toTimeString().slice(0, 5);
  };
  updateClock();
  setInterval(updateClock, 15000);

  const [wasm, landmarks, manifest] = await Promise.all([
    initPhoneWasm(),
    fetchLandmarks(),
    fetchTileManifest(),
  ]);
  const resolver = wasm.createTileResolver(manifest);

  const positioned = landmarks.filter((l) => l.ig);
  $('#home-summary').textContent = `已同步 ${positioned.length || 696} 个地标 · 离线 PWA 就绪`;

  // ---------- App Navigation ----------
  let radarMap: L.Map | null = null;
  let radarPinLayer: L.LayerGroup | null = null;
  let radarRouteLayer: L.LayerGroup | null = null;
  let routeMode = false;
  let routeStart: WorldPoint | null = null;
  let lastRadarPoint: WorldPoint = { x: 1860, y: 1190 };

  let chalMap: L.Map | null = null;
  let chalLayer: L.LayerGroup | null = null;
  let chalTarget: { id: string; title: string; x: number; y: number } | null = null;

  function openApp(name: string) {
    document.querySelectorAll('.app-view').forEach((v) => v.classList.remove('active'));
    $(`#app-${name}`).classList.add('active');
    if (name === 'radar') ensureRadar();
    else if (name === 'checklist') void renderChecklist();
    else if (name === 'messages') void renderContacts();
    else if (name === 'snapmatic') void renderPhotos();
    else if (name === 'notes') void renderNotes();
    else if (name === 'challenge') ensureChallenge();
  }

  document.querySelectorAll<HTMLElement>('[data-open]').forEach((btn) => {
    btn.onclick = () => openApp(btn.dataset.open!);
  });
  document.querySelectorAll<HTMLElement>('.back-home').forEach((btn) => {
    btn.onclick = () => {
      if (btn.id === 'msg-back' && !$('#msg-thread').classList.contains('hidden')) {
        $('#msg-thread').classList.add('hidden');
        $('#msg-contacts').classList.remove('hidden');
        $('#msg-title').textContent = 'Messages 线人';
        return;
      }
      openApp('home');
    };
  });
  $('#home-pill').onclick = () => openApp('home');

  // ---------- 1. Radar App ----------
  function ensureRadar() {
    if (!radarMap) {
      radarMap = L.map('radar-map', {
        crs: LeonidaCRS,
        minZoom: -1,
        maxZoom: MAX_TILE_Z + 2,
        zoomControl: false,
        attributionControl: false,
      });
      createTileLayer(resolver)?.addTo(radarMap);
      const lmLayer = L.layerGroup().addTo(radarMap);
      for (const l of positioned) {
        L.circleMarker(toLatLng(l.ig!), {
          radius: 4,
          color: '#111',
          weight: 1,
          fillColor: `#${l.color}`,
          fillOpacity: 0.9,
        })
          .bindTooltip(`${l.id} ${l.name}`)
          .addTo(lmLayer);
      }
      radarPinLayer = L.layerGroup().addTo(radarMap);
      radarRouteLayer = L.layerGroup().addTo(radarMap);
      radarMap.setView(toLatLng(lastRadarPoint), 2);

      radarMap.on('click', async (e: L.LeafletMouseEvent) => {
        const p = toWorld(e.latlng);
        lastRadarPoint = p;
        if (routeMode) {
          if (!routeStart) {
            routeStart = p;
            radarRouteLayer!.clearLayers();
            L.circleMarker(e.latlng, { radius: 6, color: '#fff', fillColor: '#22c55e', fillOpacity: 1 }).addTo(
              radarRouteLayer!,
            );
            $('#radar-bar').textContent = '已选起点，请点击终点…';
          } else {
            const s = routeStart;
            routeStart = null;
            routeMode = false;
            const r = (await fetchRoute(s.x, s.y, p.x, p.y)) ?? wasm.planRoute(landmarks, s.x, s.y, p.x, p.y);
            L.polyline(
              r.waypoints.map(([x, y]) => toLatLng({ x, y })),
              { color: '#00e5ff', weight: 4 },
            ).addTo(radarRouteLayer!);
            L.circleMarker(e.latlng, { radius: 6, color: '#fff', fillColor: '#ef4444', fillOpacity: 1 }).addTo(
              radarRouteLayer!,
            );
            $('#radar-bar').textContent = `里程 ${formatDistance(r.distance_m)} · 驾车 ${r.drive_min.toFixed(1)} min`;
          }
          return;
        }
        radarPinLayer!.clearLayers();
        L.circleMarker(e.latlng, { radius: 7, color: '#fff', fillColor: '#ff4fa3', fillOpacity: 1 }).addTo(
          radarPinLayer!,
        );
        $('#radar-bar').textContent = `插旗坐标: X ${p.x.toFixed(0)}, Y ${p.y.toFixed(0)}`;
      });
    }
    setTimeout(() => radarMap?.invalidateSize(), 60);
  }

  function jumpToRadar(x: number, y: number, label: string) {
    lastRadarPoint = { x, y };
    openApp('radar');
    setTimeout(() => {
      if (!radarMap || !radarPinLayer) return;
      radarMap.setView(toLatLng({ x, y }), 4);
      radarPinLayer.clearLayers();
      L.circleMarker(toLatLng({ x, y }), { radius: 8, color: '#fff', fillColor: '#ffb347', fillOpacity: 1 })
        .bindPopup(esc(label))
        .addTo(radarPinLayer)
        .openPopup();
      $('#radar-bar').textContent = `${label} (${x.toFixed(0)}, ${y.toFixed(0)})`;
    }, 100);
  }

  $('#radar-route-btn').onclick = () => {
    routeMode = true;
    routeStart = null;
    $('#radar-bar').textContent = '路线模式：请点击地图起点…';
  };

  // ---------- 2. Checklist App ----------
  let checklist: CollectibleItem[] = [];
  async function renderChecklist() {
    if (!checklist.length) checklist = await fetchChecklist();
    const done = checklist.filter((i) => i.completed).length;
    const pct = checklist.length ? Math.round((done / checklist.length) * 100) : 0;
    $('#check-pct').textContent = `${pct}%`;
    $('#check-prog').style.width = `${pct}%`;

    const listEl = $('#check-list');
    listEl.innerHTML = '';
    for (const item of checklist) {
      const card = document.createElement('div');
      card.className = 'card row';
      card.style.justifyContent = 'space-between';
      card.innerHTML = `
        <label class="row" style="flex:1;cursor:pointer">
          <input type="checkbox" ${item.completed ? 'checked' : ''} style="width:auto" />
          <div>
            <div><b>${esc(item.title)}</b></div>
            <div class="muted">${esc(item.category)} · ${esc(item.district ?? '')}</div>
          </div>
        </label>
        <button class="pill-btn">定位</button>`;
      card.querySelector('input')!.onchange = async (e) => {
        checklist = await toggleChecklist(item.id, (e.target as HTMLInputElement).checked, checklist);
        void renderChecklist();
      };
      card.querySelector('button')!.onclick = () => jumpToRadar(item.x, item.y, item.title);
      listEl.appendChild(card);
    }
  }

  // ---------- 3. Messages App ----------
  let activeContact: PhoneContact | null = null;
  async function renderContacts() {
    $('#msg-thread').classList.add('hidden');
    $('#msg-contacts').classList.remove('hidden');
    $('#msg-title').textContent = 'Messages 线人';
    const contacts = await fetchContacts();
    const box = $('#msg-contacts');
    box.innerHTML = '';
    for (const c of contacts) {
      const card = document.createElement('div');
      card.className = 'card contact-row';
      card.innerHTML = `
        <div class="avatar" style="background:${esc(c.avatar_color)}">${esc(c.name[0])}</div>
        <div>
          <div><b>${esc(c.name)}</b> <span class="muted">${esc(c.role)}</span></div>
          <div class="muted">${esc(c.bio)}</div>
        </div>`;
      card.onclick = () => openThread(c);
      box.appendChild(card);
    }
  }

  async function openThread(c: PhoneContact) {
    activeContact = c;
    $('#msg-contacts').classList.add('hidden');
    $('#msg-thread').classList.remove('hidden');
    $('#msg-title').textContent = c.name;
    const msgs = await fetchMessages(c.id);
    const box = $('#msg-bubbles');
    box.innerHTML = msgs
      .map(
        (m) =>
          `<div class="bubble ${m.sender === 'Me' ? 'me' : 'them'}">${esc(m.text)}<div class="muted" style="font-size:10px;margin-top:2px">${esc(m.timestamp)}</div></div>`,
      )
      .join('');
    box.scrollTop = box.scrollHeight;
  }

  $('#msg-form').onsubmit = async (e) => {
    e.preventDefault();
    if (!activeContact) return;
    const inp = $<HTMLInputElement>('#msg-input');
    const text = inp.value.trim();
    if (!text) return;
    inp.value = '';
    await sendMessage(activeContact.id, text);
    await openThread(activeContact);
  };

  // ---------- 4. Snapmatic App ----------
  let pendingDataUrl = '';
  $<HTMLInputElement>('#snap-upload').onchange = (e) => {
    const f = (e.target as HTMLInputElement).files?.[0];
    if (!f) return;
    const reader = new FileReader();
    reader.onload = () => {
      pendingDataUrl = String(reader.result ?? '');
    };
    reader.readAsDataURL(f);
  };

  async function renderPhotos() {
    const photos = await fetchPhotos();
    const gal = $('#snap-gallery');
    gal.innerHTML = photos.length ? '' : `<div class="muted">暂无照片，点击上方“保存地理照片”创建第一张 Snapmatic！</div>`;
    for (const ph of photos) {
      const card = document.createElement('div');
      card.className = 'card';
      card.innerHTML = `
        <img class="snap-img" src="${esc(ph.image_data_url)}" style="filter:${esc(ph.filter)}" alt="${esc(ph.title)}" />
        <div><b>${esc(ph.title)}</b></div>
        <div class="muted">${esc(ph.district)} · (${ph.x.toFixed(0)}, ${ph.y.toFixed(0)})</div>
        <div class="row" style="margin-top:6px">
          <button class="pill-btn jump">地图查看</button>
          <button class="pill-btn del">删除</button>
        </div>`;
      (card.querySelector('.jump') as HTMLButtonElement).onclick = () => jumpToRadar(ph.x, ph.y, ph.title);
      (card.querySelector('.del') as HTMLButtonElement).onclick = async () => {
        await deletePhoto(ph.id);
        await renderPhotos();
      };
      gal.appendChild(card);
    }
  }

  $('#snap-save').onclick = async () => {
    const title = $<HTMLInputElement>('#snap-title').value.trim() || 'Vice Beach Snapmatic';
    const filter = $<HTMLSelectElement>('#snap-filter').value;
    const image_data_url = pendingDataUrl || makeSamplePhotoDataUrl(title);
    await savePhoto({
      title,
      district: 'Leonida',
      x: Math.round(lastRadarPoint.x),
      y: Math.round(lastRadarPoint.y),
      filter,
      caption: '',
      image_data_url,
    });
    pendingDataUrl = '';
    $<HTMLInputElement>('#snap-title').value = '';
    await renderPhotos();
  };

  // ---------- 5. Notes App ----------
  async function renderNotes() {
    const notes = await fetchNotes();
    const box = $('#note-list');
    box.innerHTML = notes.length ? '' : `<div class="muted">暂无随身笔记</div>`;
    for (const n of notes) {
      const card = document.createElement('div');
      card.className = 'card';
      const hasCoords = n.x !== undefined && n.y !== undefined;
      card.innerHTML = `
        <div><b>${esc(n.title)}</b></div>
        <div class="muted" style="margin:4px 0">${esc(n.body)}</div>
        <div class="row">
          ${hasCoords ? `<button class="pill-btn jump">定位 (${n.x?.toFixed(0)}, ${n.y?.toFixed(0)})</button>` : ''}
          <button class="pill-btn del">删除</button>
        </div>`;
      const jb = card.querySelector('.jump') as HTMLButtonElement | null;
      if (jb && hasCoords) jb.onclick = () => jumpToRadar(n.x!, n.y!, n.title);
      (card.querySelector('.del') as HTMLButtonElement).onclick = async () => {
        await deleteNote(n.id);
        await renderNotes();
      };
      box.appendChild(card);
    }
  }

  $('#note-save').onclick = async () => {
    const title = $<HTMLInputElement>('#note-title').value.trim() || '随身笔记';
    const body = $<HTMLTextAreaElement>('#note-body').value.trim();
    if (!body) return;
    await saveNote({
      title,
      body,
      x: Math.round(lastRadarPoint.x),
      y: Math.round(lastRadarPoint.y),
    });
    $<HTMLInputElement>('#note-title').value = '';
    $<HTMLTextAreaElement>('#note-body').value = '';
    await renderNotes();
  };

  // ---------- 6. Challenge App ----------
  function pickChallengeScene() {
    const pool = positioned.length
      ? positioned.map((l) => ({ id: l.id, title: `${l.name} (${l.igAddress})`, x: l.ig!.x, y: l.ig!.y }))
      : [{ id: 'demo-1', title: 'Ocean View Hotel, Vice Beach', x: 1860, y: 1190 }];
    chalTarget = pool[Math.floor(Math.random() * pool.length)]!;
    chalLayer?.clearLayers();
    $('#chal-clue').innerHTML = `<b>目标线索：</b>${esc(chalTarget.title)}`;
    $('#chal-out').textContent = '点击下方地图猜测位置…';
  }

  function ensureChallenge() {
    if (!chalMap) {
      chalMap = L.map('chal-map', {
        crs: LeonidaCRS,
        minZoom: -1,
        maxZoom: MAX_TILE_Z + 2,
        zoomControl: false,
        attributionControl: false,
      });
      createTileLayer(resolver)?.addTo(chalMap);
      chalLayer = L.layerGroup().addTo(chalMap);
      chalMap.setView(toLatLng({ x: -1500, y: 0 }), 1);
      chalMap.on('click', async (e: L.LeafletMouseEvent) => {
        if (!chalTarget || !chalLayer) return;
        const g = toWorld(e.latlng);
        const res = wasm.scoreChallenge(chalTarget.x, chalTarget.y, g.x, g.y);
        chalLayer.clearLayers();
        L.polyline([e.latlng, toLatLng(chalTarget)], { color: '#00e5ff', dashArray: '5 4', weight: 2 }).addTo(
          chalLayer,
        );
        L.circleMarker(e.latlng, { radius: 6, color: '#fff', fillColor: '#ff4fa3', fillOpacity: 1 }).addTo(chalLayer);
        L.circleMarker(toLatLng(chalTarget), { radius: 7, color: '#fff', fillColor: '#22c55e', fillOpacity: 1 }).addTo(
          chalLayer,
        );
        $('#chal-out').innerHTML = `得分 <b>${res.score}</b> (${res.grade}) · 误差 ${formatDistance(res.distance_m)}`;
        void submitPhoneChallenge({
          player: 'PhonePlayer',
          scene_id: chalTarget.id,
          score: res.score,
          distance_m: Math.round(res.distance_m),
          grade: res.grade,
        });
      });
      pickChallengeScene();
    }
    setTimeout(() => chalMap?.invalidateSize(), 60);
  }
  $('#chal-next').onclick = () => pickChallengeScene();
}

main().catch(console.error);

if ('serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('/phone/sw.js').catch(() => undefined);
  });
}
