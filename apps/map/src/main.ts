import L from 'leaflet';
import 'leaflet/dist/leaflet.css';
import './style.css';
import type {
  AccentPreset,
  CameraObs,
  CameraPose,
  ChallengeScoreResult,
  CollectibleItem,
  HandlingItem,
  IrlStatus,
  Landmark,
  Landmark3D,
  ListDensity,
  PanelStyle,
  PhotogrammetryProject,
  Ray2D,
  SolEntry,
  UserMarker,
  WorldPoint,
} from './types';
import {
  MAX_TILE_Z,
  TILE_SIZE,
  WORLD_EXTENT,
  bearing,
  formatDistance,
  formatWorld,
  idSortKey,
} from './types';
import {
  MarkerClient,
  checkAdminIntegrity,
  computeCompanionStats,
  deletePhotogrammetryProject,
  exportAdminDb,
  exportHandlingRemote,
  fetchAdminOverview,
  fetchChangelog,
  fetchCompanionChecklist,
  fetchCredits,
  fetchGameStats,
  fetchLandmarks3D,
  fetchLandmarksData,
  fetchLeaderboard,
  fetchMarkerComments,
  fetchMediaFiles,
  fetchMediaTabs,
  fetchNews,
  fetchNotifications,
  fetchRanks,
  fetchRouteRemote,
  fetchSolDataset,
  fetchTileManifest,
  fetchWikiCache,
  importCompanionChecklist,
  inspectRageHeaderRemote,
  listPhotogrammetryProjects,
  markNotificationsRead,
  parseHandlingRemote,
  postMarkerComment,
  publishAdminNews,
  savePhotogrammetryProject,
  solveCameraPoseRemote,
  submitChallengeScore,
  toggleCompanionItem,
} from './api';
import { loadLang, saveLang, tr, translateDistrict, type I18nKey, type Lang } from './i18n';
import { ACCENT_HEX, DEFAULT_SETTINGS, loadStudioSettings, saveStudioSettings } from './settings';
import { Leonida3DView } from './view3d';
import { initLeonidaWasm, type WasmTileResolverHandle } from './wasm-bridge';

// ---------- CRS: Leonida world metres, matching the gta6map tile pyramid ----------
const k = 1 / (WORLD_EXTENT.size / TILE_SIZE);
const LeonidaCRS = L.extend({}, L.CRS.Simple, {
  transformation: new L.Transformation(k, WORLD_EXTENT.half * k, -k, WORLD_EXTENT.half * k),
}) as L.CRS;

const toLatLng = (p: WorldPoint) => L.latLng(p.y, p.x);
const toWorld = (ll: L.LatLng): WorldPoint => ({ x: ll.lng, y: ll.lat });

const $ = <T extends HTMLElement = HTMLElement>(sel: string) => document.querySelector(sel) as T;
const esc = (s: unknown) =>
  String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);

const STATUS_LABEL: Record<IrlStatus, string> = {
  confirmed: '现实已确认',
  unconfirmed: '待确认',
  unknown: '未知原型',
};
const BASEMAPS: Record<string, { base: string; label: string }> = {
  dupzor: { base: 'dupzor,51', label: '社区底图 · dupzor v51' },
  yanis: { base: 'yanis,6', label: '社区底图 · yanis v6' },
};
const OVERLAY_SUFFIX = 'aiwe,1,martipk,5,rickrick,2,vlad,1';

// ---------- Tile layer with composite → base fallback ----------
function createBasemap(resolver: WasmTileResolverHandle, base: string, overlays: boolean): L.GridLayer {
  const composite = `${base},${OVERLAY_SUFFIX}`;
  const Layer = L.GridLayer.extend({
    createTile(coords: L.Coords, done: L.DoneCallback) {
      const img = document.createElement('img');
      img.alt = '';
      const t = { z: coords.z, x: coords.x, y: coords.y };
      const set = resolver.resolve({ base, composite: overlays ? composite : undefined }, t);
      if (!set) {
        setTimeout(() => done(undefined, img), 0);
        return img;
      }
      img.onload = () => done(undefined, img);
      img.onerror = () => done(new Error('tile'), img);
      img.src = `tiles/${set}/${t.z}/${t.z},${t.y},${t.x}.jpg`;
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

// ---------- Countdown (GTA VI target release) ----------
function startCountdown(getLang: () => Lang) {
  const targetMs = Date.UTC(2026, 10, 19, 0, 0, 0);
  const el = $('#countdown-val');
  const tick = () => {
    const diff = targetMs - Date.now();
    if (diff <= 0) {
      el.textContent = tr(getLang(), 'released');
      return;
    }
    const d = Math.floor(diff / 86400000);
    const h = Math.floor((diff % 86400000) / 3600000);
    const m = Math.floor((diff % 3600000) / 60000);
    const s = Math.floor((diff % 60000) / 1000);
    el.textContent = `${d}d ${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  };
  tick();
  setInterval(tick, 1000);
}

async function main() {
  let lang: Lang = loadLang();
  let settings = loadStudioSettings();

  const applyI18n = () => {
    document.querySelectorAll<HTMLElement>('[data-i18n]').forEach((el) => {
      const key = el.dataset.i18n as I18nKey;
      el.textContent = tr(lang, key);
    });
    $<HTMLInputElement>('#search').placeholder = tr(lang, 'searchPlaceholder');
    $<HTMLInputElement>('#irl-input').placeholder = tr(lang, 'irlPlaceholder');
    $<HTMLInputElement>('#challenge-player').placeholder = tr(lang, 'challengePlayerPlaceholder');
    $('#lang-toggle').textContent = tr(lang, 'langBtn');
  };

  const applyTheme = () => {
    document.body.classList.toggle('bg-effects', settings.bgEffects);
    document.documentElement.style.setProperty('--accent', ACCENT_HEX[settings.accentColor] ?? '#ff4fa3');
    document.documentElement.style.setProperty('--bg', settings.selectedBackgroundColor || '#0c0e16');
    document.body.dataset.panelStyle = settings.panelStyle;
    document.body.dataset.density = settings.listDensity;
  };

  applyI18n();
  applyTheme();
  startCountdown(() => lang);

  // Preloader
  const preloader = $('#preloader');
  if (settings.isPreloaderStyle) {
    preloader.classList.remove('hidden');
    const hidePre = () => preloader.classList.add('hidden');
    $('#preloader-skip').onclick = hidePre;
    setTimeout(hidePre, 900);
  }

  // Load WASM + core data + SOL datasets in parallel
  const [
    wasm,
    landmarksPayload,
    vc,
    manifest,
    solMarkers,
    solT1,
    solT2,
    solT3,
    solLeaks,
    solShots,
    solElShots,
    wikiCache,
  ] = await Promise.all([
    initLeonidaWasm(),
    fetchLandmarksData(),
    fetchLandmarks3D(),
    fetchTileManifest(),
    fetchSolDataset('markers'),
    fetchSolDataset('trailer1'),
    fetchSolDataset('trailer2'),
    fetchSolDataset('trailer3'),
    fetchSolDataset('leaks'),
    fetchSolDataset('screenshots'),
    fetchSolDataset('el_screenshots'),
    fetchWikiCache(),
  ]);

  const landmarkStore =
    landmarksPayload.kind === 'normalized'
      ? wasm.createLandmarksFromNormalized(landmarksPayload.landmarks)
      : wasm.createLandmarksFromTuples(landmarksPayload.rows);

  const landmarks = [...landmarkStore.all()].sort((a, b) => idSortKey(a.id).localeCompare(idSortKey(b.id)));
  const landmarks3d: Landmark3D[] = vc ? wasm.parseGtamaplibVc(vc) : [];
  const resolver = wasm.createTileResolver(manifest);
  const stats = landmarkStore.stats();

  const datasetEntries: Record<'trailer1' | 'trailer2' | 'trailer3' | 'leaks' | 'screenshots', SolEntry[]> = {
    trailer1: solT1,
    trailer2: solT2,
    trailer3: solT3,
    leaks: solLeaks,
    screenshots: [...solShots, ...solElShots],
  };

  // ---------- Map ----------
  const map = L.map('map', {
    crs: LeonidaCRS,
    minZoom: -1,
    maxZoom: MAX_TILE_Z + 2,
    zoomSnap: 0.25,
    attributionControl: true,
  });
  map.attributionControl
    .setPrefix('Leaflet')
    .addAttribution('地标与底图: State of Leonida / gta6map / gtamaplib (Public Domain)');

  const basemaps: Record<string, L.GridLayer> = {};
  const availableTilesets = resolver.tilesets();
  for (const [key, b] of Object.entries(BASEMAPS)) {
    if (availableTilesets.includes(b.base)) {
      basemaps[`${b.label} + 叠加`] = createBasemap(resolver, b.base, true);
      basemaps[b.label] = createBasemap(resolver, b.base, false);
    } else if (key === 'dupzor') {
      console.warn('No tiles found; check apps/map/public/tiles or GTA6MAP_TILES');
    }
  }
  const firstBase = Object.values(basemaps)[0];
  if (firstBase) firstBase.addTo(map);

  const landmarkLayer = L.layerGroup().addTo(map);
  const clusterLayer = L.layerGroup().addTo(map);
  const gridLayer = L.layerGroup();
  const layer3d = L.layerGroup();
  if (settings.mapSettings.show3dByDefault) layer3d.addTo(map);
  const mineLayer = L.layerGroup().addTo(map);
  const toolLayer = L.layerGroup().addTo(map);
  const challengeLayer = L.layerGroup().addTo(map);

  // Build 1000m World Grid
  for (let v = -16000; v <= 16000; v += 1000) {
    const major = v % 4000 === 0;
    const style = { color: major ? '#00e5ff' : '#ffffff', weight: major ? 1 : 0.5, opacity: major ? 0.28 : 0.1 };
    L.polyline([toLatLng({ x: v, y: -16384 }), toLatLng({ x: v, y: 16384 })], style).addTo(gridLayer);
    L.polyline([toLatLng({ x: -16384, y: v }), toLatLng({ x: 16384, y: v })], style).addTo(gridLayer);
  }
  if (settings.mapSettings.gridEnabled) gridLayer.addTo(map);

  // Heatmap Canvas Overlay
  const heatmapCanvas = document.createElement('canvas');
  heatmapCanvas.className = 'heatmap-canvas';
  map.getContainer().appendChild(heatmapCanvas);
  function drawHeatmap() {
    const size = map.getSize();
    heatmapCanvas.width = size.x;
    heatmapCanvas.height = size.y;
    const ctx = heatmapCanvas.getContext('2d')!;
    ctx.clearRect(0, 0, size.x, size.y);
    if (!settings.mapSettings.heatmapEnabled) return;
    const cells = wasm.computeHeatmapCells(current, 800);
    const radius = Math.max(18, 22 * Math.pow(2, Math.min(3, map.getZoom() - 1)));
    for (const c of cells) {
      const pt = map.latLngToContainerPoint(toLatLng({ x: c.x, y: c.y }));
      if (pt.x < -radius || pt.y < -radius || pt.x > size.x + radius || pt.y > size.y + radius) continue;
      const g = ctx.createRadialGradient(pt.x, pt.y, 0, pt.x, pt.y, radius);
      g.addColorStop(0, `rgba(255, 79, 163, ${0.25 + 0.55 * c.weight})`);
      g.addColorStop(0.5, `rgba(255, 179, 71, ${0.15 + 0.35 * c.weight})`);
      g.addColorStop(1, 'rgba(0, 229, 255, 0)');
      ctx.fillStyle = g;
      ctx.beginPath();
      ctx.arc(pt.x, pt.y, radius, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  const solLayers: Record<keyof typeof datasetEntries, L.LayerGroup> = {
    trailer1: L.layerGroup(),
    trailer2: L.layerGroup(),
    trailer3: L.layerGroup(),
    leaks: L.layerGroup(),
    screenshots: L.layerGroup(),
  };

  L.control
    .layers(
      basemaps,
      {
        地标: landmarkLayer,
        '3D 地标 (gtamaplib-vc)': layer3d,
        我的标记: mineLayer,
        'Trailer 1': solLayers.trailer1,
        'Trailer 2': solLayers.trailer2,
        'Trailer 3': solLayers.trailer3,
        Leaks: solLayers.leaks,
        Screenshots: solLayers.screenshots,
      },
      { collapsed: true },
    )
    .addTo(map);
  L.control.scale({ metric: true, imperial: false }).addTo(map);

  // initial view from hash "#x,y,z"
  const [hx, hy, hz] = location.hash.slice(1).split(',').map(Number);
  if (Number.isFinite(hx) && Number.isFinite(hy) && Number.isFinite(hz)) {
    map.setView(toLatLng({ x: hx!, y: hy! }), hz!);
  } else {
    map.setView(toLatLng({ x: -2500, y: 0 }), 1.5);
  }
  map.on('moveend', () => {
    const c = toWorld(map.getCenter());
    history.replaceState(null, '', `#${c.x.toFixed(0)},${c.y.toFixed(0)},${map.getZoom()}`);
    drawHeatmap();
    if (settings.mapSettings.clusteringEnabled) renderClusters();
  });
  map.on('mousemove', (e: L.LeafletMouseEvent) => {
    $('#coords').textContent = `X ${e.latlng.lng.toFixed(0)}  Y ${e.latlng.lat.toFixed(0)}  (m) · zoom ${map.getZoom()}`;
  });

  // ---------- Landmarks ----------
  const markerById = new Map<string, L.CircleMarker>();
  const popupHtml = (l: Landmark) => {
    const near = l.ig ? landmarkStore.nearest(l.ig, 4).slice(1) : [];
    const gmaps = l.irl ? `https://www.google.com/maps?q=${l.irl.lat},${l.irl.lon}` : null;
    const wiki = wikiCache[l.id] ?? wikiCache[l.name.toLowerCase()];
    return `<div class="pop" data-lm-id="${esc(l.id)}">
      <div class="pop-id" style="border-color:#${l.color}">${esc(l.id)} · ${esc(STATUS_LABEL[l.irlStatus])}</div>
      <div class="pop-name">${esc(l.name)}</div>
      <div><b>游戏地址</b> ${esc(l.igAddress)}</div>
      ${l.ig ? `<div><b>游戏坐标</b> ${formatWorld(l.ig)}</div>` : ''}
      <div><b>现实原型</b> ${esc(l.irlAddress)}</div>
      ${gmaps ? `<div><a href="${gmaps}" target="_blank" rel="noopener">在 Google 地图中查看现实位置 ↗</a></div>` : ''}
      <div><b>标签</b> ${l.tags.map(esc).join(', ') || '—'}</div>
      ${near.length ? `<div class="muted">附近: ${near.map((n) => `${esc(n.landmark.id)} (${formatDistance(n.distance)})`).join(', ')}</div>` : ''}
      ${wiki ? `<div class="pop-wiki"><b>Wiki:</b> ${esc(wiki.summary)}</div>` : ''}
      <div class="pop-disc">
        <div class="pop-disc-list" id="disc-list-${esc(l.id)}">加载讨论中…</div>
        <div class="row"><input id="disc-in-${esc(l.id)}" placeholder="发表评论…" style="padding:3px 6px;font-size:11px" /><button id="disc-btn-${esc(l.id)}" class="mini" style="position:static">发送</button></div>
      </div>
    </div>`;
  };
  map.on('popupopen', (ev) => {
    const el = ev.popup.getElement()?.querySelector<HTMLElement>('.pop[data-lm-id]');
    const lmId = el?.dataset.lmId;
    if (!lmId) return;
    const renderComments = (list: Awaited<ReturnType<typeof fetchMarkerComments>>) => {
      const box = document.getElementById(`disc-list-${lmId}`);
      if (!box) return;
      box.innerHTML = list.length
        ? list.map((c) => `<div><b>${esc(c.author)}:</b> ${esc(c.body)}</div>`).join('')
        : '<span class="muted">暂无讨论，来发第一条评论！</span>';
    };
    void fetchMarkerComments(lmId).then(renderComments);
    const btn = document.getElementById(`disc-btn-${lmId}`);
    if (btn) {
      btn.onclick = async () => {
        const inp = document.getElementById(`disc-in-${lmId}`) as HTMLInputElement | null;
        const body = inp?.value.trim();
        if (!body) return;
        inp!.value = '';
        const updated = await postMarkerComment(lmId, 'Explorer', body);
        renderComments(updated);
      };
    }
  });
  for (const l of landmarks) {
    if (!l.ig) continue;
    const m = L.circleMarker(toLatLng(l.ig), {
      radius: 6,
      color: '#111',
      weight: 1.5,
      fillColor: `#${l.color}`,
      fillOpacity: 0.95,
      dashArray: l.irlStatus === 'confirmed' ? undefined : '2 2',
    }).bindPopup(() => popupHtml(l), { maxWidth: 340 });
    if (settings.mapSettings.showLabels) {
      m.bindTooltip(`${l.id} ${l.name}`, { direction: 'top', offset: [0, -6] });
    }
    markerById.set(l.id, m);
  }
  for (const l of landmarks3d) {
    L.circleMarker(toLatLng(l.xyz), {
      radius: Math.min(14, 4 + l.height / 40),
      color: '#00e5ff',
      weight: 2,
      fillOpacity: 0.15,
    })
      .bindTooltip(`${l.id} ${l.name} · 高度 ${l.height.toFixed(0)} m`)
      .addTo(layer3d);
  }

  function renderSolLayers() {
    (Object.keys(datasetEntries) as Array<keyof typeof datasetEntries>).forEach((key) => {
      const grp = solLayers[key];
      grp.clearLayers();
      const color = settings.clusterCustomColors[key];
      for (const item of datasetEntries[key]) {
        if (!Number.isFinite(item.x) || !Number.isFinite(item.y)) continue;
        L.circleMarker(toLatLng({ x: item.x, y: item.y }), {
          radius: 7,
          color: '#0d0f17',
          weight: 1.5,
          fillColor: color,
          fillOpacity: 0.92,
        })
          .bindPopup(
            `<div class="pop">
              <div class="pop-id" style="border-color:${color}">${esc(item.dataset)} · ${esc(item.id)}</div>
              <div class="pop-name">${esc(item.title)}</div>
              <div>${formatWorld({ x: item.x, y: item.y })}</div>
              ${item.description ? `<div class="muted">${esc(item.description)}</div>` : ''}
            </div>`,
          )
          .bindTooltip(item.title, { direction: 'top', offset: [0, -6] })
          .addTo(grp);
      }
    });
  }
  renderSolLayers();

  // ---------- Dataset Toggles Tab ----------
  function renderDatasetToggles() {
    const box = $('#dataset-toggles');
    box.innerHTML = '';
    const items: Array<{
      key: string;
      labelKey: I18nKey;
      count: number;
      color: string;
      layer: L.LayerGroup;
    }> = [
      {
        key: 'landmarks',
        labelKey: 'dsAllMarkers',
        count: solMarkers.length || landmarks.length,
        color: settings.clusterCustomColors.landmarks,
        layer: landmarkLayer,
      },
      {
        key: 'trailer1',
        labelKey: 'dsTrailer1',
        count: datasetEntries.trailer1.length,
        color: settings.clusterCustomColors.trailer1,
        layer: solLayers.trailer1,
      },
      {
        key: 'trailer2',
        labelKey: 'dsTrailer2',
        count: datasetEntries.trailer2.length,
        color: settings.clusterCustomColors.trailer2,
        layer: solLayers.trailer2,
      },
      {
        key: 'trailer3',
        labelKey: 'dsTrailer3',
        count: datasetEntries.trailer3.length,
        color: settings.clusterCustomColors.trailer3,
        layer: solLayers.trailer3,
      },
      {
        key: 'leaks',
        labelKey: 'dsLeaks',
        count: datasetEntries.leaks.length,
        color: settings.clusterCustomColors.leaks,
        layer: solLayers.leaks,
      },
      {
        key: 'screenshots',
        labelKey: 'dsScreenshots',
        count: datasetEntries.screenshots.length,
        color: settings.clusterCustomColors.screenshots,
        layer: solLayers.screenshots,
      },
      {
        key: '3d',
        labelKey: 'ds3d',
        count: landmarks3d.length,
        color: '#00e5ff',
        layer: layer3d,
      },
    ];
    for (const item of items) {
      const label = document.createElement('label');
      label.className = 'ds-row';
      const checked = map.hasLayer(item.layer);
      label.innerHTML = `
        <span class="ds-left">
          <span class="ds-dot" style="background:${item.color}"></span>
          <span>${esc(tr(lang, item.labelKey))}</span>
          <span class="muted">(${item.count})</span>
        </span>
        <input type="checkbox" ${checked ? 'checked' : ''} />`;
      const cb = label.querySelector('input')!;
      cb.onchange = () => {
        if (cb.checked) item.layer.addTo(map);
        else map.removeLayer(item.layer);
      };
      box.appendChild(label);
    }
  }
  renderDatasetToggles();

  // ---------- Filters ----------
  const state = { text: '', tags: new Set<string>(), statuses: new Set<IrlStatus>() };
  const statusBox = $('#status-filters');
  for (const s of Object.keys(STATUS_LABEL) as IrlStatus[]) {
    const b = document.createElement('button');
    b.className = 'chip';
    b.textContent = `${STATUS_LABEL[s]} ${stats.byStatus[s]}`;
    b.onclick = () => {
      state.statuses.has(s) ? state.statuses.delete(s) : state.statuses.add(s);
      b.classList.toggle('on');
      refresh();
    };
    statusBox.appendChild(b);
  }
  const tagBox = $('#tag-filters');
  for (const [t, n] of Object.entries(stats.byTag).sort((a, b) => b[1] - a[1])) {
    const b = document.createElement('button');
    b.className = 'chip';
    b.textContent = `${t} ${n}`;
    b.onclick = () => {
      state.tags.has(t) ? state.tags.delete(t) : state.tags.add(t);
      b.classList.toggle('on');
      refresh();
    };
    tagBox.appendChild(b);
  }
  $<HTMLInputElement>('#search').addEventListener('input', (e) => {
    state.text = (e.target as HTMLInputElement).value;
    refresh();
  });

  let current: Landmark[] = landmarks;
  function renderClusters() {
    clusterLayer.clearLayers();
    if (!settings.mapSettings.clusteringEnabled) return;
    const cell = 4000 / Math.pow(2, Math.max(0, map.getZoom()));
    const buckets = new Map<string, Landmark[]>();
    for (const l of current) {
      if (!l.ig) continue;
      const key = `${Math.floor(l.ig.x / cell)},${Math.floor(l.ig.y / cell)}`;
      const arr = buckets.get(key) ?? [];
      arr.push(l);
      buckets.set(key, arr);
    }
    const color = settings.clusterCustomColors.landmarks;
    for (const grp of buckets.values()) {
      if (grp.length === 1) {
        markerById.get(grp[0]!.id)?.addTo(clusterLayer);
        continue;
      }
      const cx = grp.reduce((s, l) => s + l.ig!.x, 0) / grp.length;
      const cy = grp.reduce((s, l) => s + l.ig!.y, 0) / grp.length;
      const icon = L.divIcon({
        className: '',
        html: `<div class="cluster-badge" style="width:30px;height:30px;background:${color}">${grp.length}</div>`,
        iconSize: [30, 30],
      });
      L.marker(toLatLng({ x: cx, y: cy }), { icon })
        .on('click', () => map.flyTo(toLatLng({ x: cx, y: cy }), map.getZoom() + 1.5))
        .addTo(clusterLayer);
    }
  }

  function refresh() {
    current = [...landmarkStore.filter({ text: state.text, tags: [...state.tags], statuses: [...state.statuses] })].sort(
      (a, b) => idSortKey(a.id).localeCompare(idSortKey(b.id)),
    );
    landmarkLayer.clearLayers();
    clusterLayer.clearLayers();
    if (settings.mapSettings.clusteringEnabled) {
      renderClusters();
    } else {
      for (const l of current) markerById.get(l.id)?.addTo(landmarkLayer);
    }
    drawHeatmap();
    $('#result-count').textContent = `显示 ${current.length} / ${stats.total} 个地标（已定位 ${current.filter((l) => l.ig).length}）`;
    const list = $('#landmark-list');
    list.innerHTML = '';
    for (const l of current.slice(0, 400)) {
      const li = document.createElement('li');
      li.style.borderLeftColor = `#${l.color}`;
      li.innerHTML = `<div class="li-top"><span class="li-id">${esc(l.id)}</span> ${esc(l.name)}</div><div class="muted">${esc(translateDistrict(l.district, lang))} · ${esc(l.irlAddress)}</div>`;
      if (!l.ig) li.classList.add('unpositioned');
      li.onclick = () => focusLandmark(l);
      list.appendChild(li);
    }
  }
  function focusLandmark(l: Landmark) {
    if (!l.ig) return;
    if (settings.mapSettings.autoFly) {
      map.flyTo(toLatLng(l.ig), Math.max(map.getZoom(), 5), { duration: 0.6 });
    } else {
      map.setView(toLatLng(l.ig), Math.max(map.getZoom(), 5));
    }
    markerById.get(l.id)?.openPopup();
  }
  refresh();

  function jumpToCoords(x: number, y: number) {
    switchModule('map');
    setTimeout(() => {
      map.invalidateSize();
      map.flyTo(toLatLng({ x, y }), 5, { duration: 0.6 });
    }, 50);
  }

  // ---------- Sidebar Tabs ----------
  document.querySelectorAll<HTMLButtonElement>('#tabs button').forEach((btn) => {
    btn.onclick = () => {
      document.querySelectorAll('#tabs button, .tab').forEach((el) => el.classList.remove('active'));
      btn.classList.add('active');
      $(`#tab-${btn.dataset.tab}`).classList.add('active');
    };
  });

  // Layer toggles (Heatmap / Grid / Clustering)
  const cbHeat = $<HTMLInputElement>('#layer-heatmap');
  const cbGrid = $<HTMLInputElement>('#layer-grid');
  const cbClus = $<HTMLInputElement>('#layer-cluster');
  cbHeat.checked = settings.mapSettings.heatmapEnabled;
  cbGrid.checked = settings.mapSettings.gridEnabled;
  cbClus.checked = settings.mapSettings.clusteringEnabled;
  cbHeat.onchange = () => {
    settings.mapSettings.heatmapEnabled = cbHeat.checked;
    saveStudioSettings(settings);
    drawHeatmap();
  };
  cbGrid.onchange = () => {
    settings.mapSettings.gridEnabled = cbGrid.checked;
    saveStudioSettings(settings);
    if (cbGrid.checked) gridLayer.addTo(map);
    else map.removeLayer(gridLayer);
  };
  cbClus.onchange = () => {
    settings.mapSettings.clusteringEnabled = cbClus.checked;
    saveStudioSettings(settings);
    refresh();
  };

  // ---------- Tools: click-mode state machine ----------
  type Mode =
    | 'none'
    | 'measure'
    | 'area'
    | 'ray-origin'
    | 'ray-target'
    | 'route-start'
    | 'route-end'
    | 'motion-rec'
    | 'challenge';
  let mode: Mode = 'none';
  let measurePts: WorldPoint[] = [];
  const measureLayer = L.layerGroup().addTo(toolLayer);
  const rayLayer = L.layerGroup().addTo(toolLayer);
  const routeLayer = L.layerGroup().addTo(toolLayer);
  const photoLayer = L.layerGroup().addTo(toolLayer);
  let routeStart: WorldPoint | null = null;
  const rays: Ray2D[] = [];
  let rayOrigin: WorldPoint | null = null;
  const setMode = (m: Mode) => {
    mode = m;
    map.getContainer().style.cursor = m === 'none' ? '' : 'crosshair';
  };

  function drawMeasure() {
    measureLayer.clearLayers();
    if (!measurePts.length) {
      $('#measure-out').textContent = '';
      return;
    }
    const lls = measurePts.map(toLatLng);
    if (mode === 'area' || (measurePts.length > 2 && $('#measure-out').dataset.kind === 'area')) {
      L.polygon(lls, { color: '#ffcc00', weight: 2 }).addTo(measureLayer);
      const closed = [...measurePts, measurePts[0]!];
      $('#measure-out').textContent = `周长 ${formatDistance(wasm.pathLength(closed))} · 面积 ${(wasm.polygonArea(measurePts) / 1e6).toFixed(3)} km²`;
    } else {
      L.polyline(lls, { color: '#ffcc00', weight: 3 }).addTo(measureLayer);
      $('#measure-out').textContent = `总长 ${formatDistance(wasm.pathLength(measurePts))}（${measurePts.length} 点）`;
    }
    lls.forEach((ll) => L.circleMarker(ll, { radius: 3, color: '#ffcc00' }).addTo(measureLayer));
  }
  $('#tool-measure').onclick = () => {
    measurePts = [];
    $('#measure-out').dataset.kind = 'line';
    setMode('measure');
    drawMeasure();
  };
  $('#tool-area').onclick = () => {
    measurePts = [];
    $('#measure-out').dataset.kind = 'area';
    setMode('area');
    drawMeasure();
  };
  $('#tool-clear-measure').onclick = () => {
    measurePts = [];
    setMode('none');
    drawMeasure();
  };

  function drawRays() {
    rayLayer.clearLayers();
    for (const r of rays) {
      const len = 20000;
      const rad = (r.bearing * Math.PI) / 180;
      L.polyline(
        [toLatLng(r.origin), toLatLng({ x: r.origin.x + Math.sin(rad) * len, y: r.origin.y + Math.cos(rad) * len })],
        { color: '#ff4d6d', weight: 2 },
      ).addTo(rayLayer);
      L.circleMarker(toLatLng(r.origin), { radius: 4, color: '#ff4d6d' }).addTo(rayLayer);
    }
    const res = wasm.triangulate(rays);
    if (res) {
      L.circleMarker(toLatLng(res.point), { radius: 8, color: '#fff', fillColor: '#ff4d6d', fillOpacity: 1 }).addTo(
        rayLayer,
      );
      const near = landmarkStore.nearest(res.point, 3);
      $('#ray-out').innerHTML = `交点 <b>${formatWorld(res.point)}</b> · RMS 残差 ${res.rms.toFixed(1)} m<br/>最近地标: ${near.map((n) => `${esc(n.landmark.id)} ${esc(n.landmark.name)} (${formatDistance(n.distance)})`).join('; ')}`;
    } else {
      $('#ray-out').textContent = rays.length ? `已添加 ${rays.length} 条射线，至少需要 2 条非平行射线` : '';
    }
  }
  $('#tool-ray').onclick = () => {
    rayOrigin = null;
    setMode('ray-origin');
    $('#ray-out').textContent = '点击观察点…';
  };
  $('#tool-clear-rays').onclick = () => {
    rays.length = 0;
    setMode('none');
    drawRays();
  };

  // ---------- Route Planning ----------
  $('#tool-route').onclick = () => {
    routeStart = null;
    routeLayer.clearLayers();
    setMode('route-start');
    $('#route-out').textContent = '请点击地图选择起点…';
  };
  $('#tool-clear-route').onclick = () => {
    routeStart = null;
    routeLayer.clearLayers();
    setMode('none');
    $('#route-out').textContent = '';
  };
  async function runRoutePlan(start: WorldPoint, end: WorldPoint) {
    routeLayer.clearLayers();
    L.circleMarker(toLatLng(start), { radius: 6, color: '#fff', fillColor: '#22c55e', fillOpacity: 1 })
      .bindTooltip('起点')
      .addTo(routeLayer);
    L.circleMarker(toLatLng(end), { radius: 6, color: '#fff', fillColor: '#ef4444', fillOpacity: 1 })
      .bindTooltip('终点')
      .addTo(routeLayer);
    const route =
      (await fetchRouteRemote(start.x, start.y, end.x, end.y)) ??
      wasm.planRoute(landmarks, start.x, start.y, end.x, end.y);
    L.polyline(
      route.waypoints.map(([x, y]) => toLatLng({ x, y })),
      { color: '#00e5ff', weight: 4, opacity: 0.9 },
    ).addTo(routeLayer);
    $('#route-out').innerHTML = `总里程 <b>${formatDistance(route.distance_m)}</b>（${route.waypoints.length} 节点）<br/>驾车约 <b>${route.drive_min.toFixed(1)} min</b> · 飞行约 <b>${route.fly_min.toFixed(1)} min</b>`;
  }

  // ---------- Screenshot Challenge ----------
  interface ChallengeScene {
    id: string;
    title: string;
    subtitle: string;
    x: number;
    y: number;
    mediaUrl?: string;
  }
  const challengePool: ChallengeScene[] = [
    ...datasetEntries.screenshots,
    ...datasetEntries.trailer1,
    ...datasetEntries.trailer2,
    ...datasetEntries.trailer3,
    ...datasetEntries.leaks,
  ]
    .filter((e) => Number.isFinite(e.x) && Number.isFinite(e.y))
    .map((e) => ({
      id: e.id,
      title: e.title,
      subtitle: `${e.dataset}${e.district ? ` · ${translateDistrict(e.district, lang)}` : ''}`,
      x: e.x,
      y: e.y,
      mediaUrl: e.mediaUrl,
    }));

  if (!challengePool.length) {
    for (const l of landmarks) {
      if (l.ig) {
        challengePool.push({
          id: l.id,
          title: l.name,
          subtitle: `${l.igAddress} · ${l.irlAddress}`,
          x: l.ig.x,
          y: l.ig.y,
        });
      }
    }
  }

  let activeScene: ChallengeScene | null = null;
  let lastScore: ChallengeScoreResult | null = null;

  async function refreshMiniLeaderboard() {
    const lb = await fetchLeaderboard(5);
    const ol = $('#challenge-lb-mini');
    ol.innerHTML = lb.length
      ? lb.map((r) => `<li><b>${esc(r.player)}</b> · ${r.score} pts (${esc(r.grade ?? '-')})</li>`).join('')
      : `<li class="muted">暂无记录</li>`;
  }

  function startChallengeRound() {
    challengeLayer.clearLayers();
    lastScore = null;
    ($<HTMLButtonElement>('#challenge-submit')).disabled = true;
    $('#challenge-result').textContent = tr(lang, 'challengeClickPrompt');
    if (!challengePool.length) return;
    activeScene = challengePool[Math.floor(Math.random() * challengePool.length)]!;
    $('#challenge-scene').innerHTML = `
      ${activeScene.mediaUrl ? `<img src="${esc(activeScene.mediaUrl)}" alt="${esc(activeScene.title)}" />` : ''}
      <div><b>${esc(activeScene.title)}</b></div>
      <div class="muted">${esc(activeScene.subtitle)}</div>
      <div class="muted">ID: ${esc(activeScene.id)}</div>`;
    setMode('challenge');
  }

  $('#challenge-next').onclick = () => startChallengeRound();
  $('#challenge-submit').onclick = async () => {
    if (!activeScene || !lastScore) return;
    const player = $<HTMLInputElement>('#challenge-player').value.trim() || 'Anonymous';
    ($<HTMLButtonElement>('#challenge-submit')).disabled = true;
    const synced = await submitChallengeScore({
      player,
      scene_id: activeScene.id,
      score: lastScore.score,
      distance_m: Math.round(lastScore.distance_m),
      grade: lastScore.grade,
    });
    $('#challenge-result').innerHTML += `<div class="muted">${synced ? '已提交至服务端排行榜' : '离线已保存至本地排行榜'}</div>`;
    await refreshMiniLeaderboard();
  };

  // ---------- Marker Motion ----------
  const motionLayer = L.layerGroup().addTo(toolLayer);
  let motionPts: WorldPoint[] = [];
  let motionRaf = 0;
  function drawMotionPath() {
    motionLayer.clearLayers();
    if (!motionPts.length) return;
    L.polyline(motionPts.map(toLatLng), { color: '#ff4fa3', dashArray: '6 4', weight: 3 }).addTo(motionLayer);
    motionPts.forEach((pt) => L.circleMarker(toLatLng(pt), { radius: 4, color: '#fff', fillColor: '#ff4fa3', fillOpacity: 1 }).addTo(motionLayer));
  }
  $('#tool-motion-rec').onclick = () => {
    cancelAnimationFrame(motionRaf);
    motionPts = [];
    drawMotionPath();
    setMode('motion-rec');
    $('#motion-out').textContent = '点击地图录制路径点（按 ESC 结束录制）…';
  };
  $('#tool-motion-clear').onclick = () => {
    cancelAnimationFrame(motionRaf);
    motionPts = [];
    drawMotionPath();
    setMode('none');
    $('#motion-out').textContent = '';
  };
  $('#tool-motion-play').onclick = () => {
    if (motionPts.length < 2) {
      $('#motion-out').textContent = '请先录制至少 2 个路径点';
      return;
    }
    setMode('none');
    cancelAnimationFrame(motionRaf);
    drawMotionPath();
    const actor = L.circleMarker(toLatLng(motionPts[0]!), {
      radius: 9,
      color: '#fff',
      weight: 2,
      fillColor: '#00e5ff',
      fillOpacity: 1,
    }).addTo(motionLayer);
    const startTs = performance.now();
    const duration = Math.max(2500, motionPts.length * 1200);
    const step = (now: number) => {
      const t = Math.min(1, (now - startTs) / duration);
      const sample = wasm.interpolateMotionPath(motionPts, t);
      if (sample) {
        actor.setLatLng(toLatLng(sample));
        $('#motion-out').textContent = `进度 ${(t * 100).toFixed(0)}% · 坐标 ${formatWorld(sample)} · 航向 ${sample.bearing.toFixed(0)}°`;
      }
      if (t < 1) motionRaf = requestAnimationFrame(step);
    };
    motionRaf = requestAnimationFrame(step);
  };

  map.on('click', (e: L.LeafletMouseEvent) => {
    const p = toWorld(e.latlng);
    if (mode === 'measure' || mode === 'area') {
      measurePts.push(p);
      drawMeasure();
    } else if (mode === 'motion-rec') {
      motionPts.push(p);
      drawMotionPath();
      $('#motion-out').textContent = `已录制 ${motionPts.length} 个路径点（点击“播放”或按 ESC 结束）`;
    } else if (mode === 'ray-origin') {
      rayOrigin = p;
      setMode('ray-target');
      $('#ray-out').textContent = '点击视线方向上的一点…';
    } else if (mode === 'ray-target' && rayOrigin) {
      rays.push({ origin: rayOrigin, bearing: bearing(rayOrigin, p) });
      rayOrigin = null;
      setMode('none');
      drawRays();
    } else if (mode === 'route-start') {
      routeStart = p;
      setMode('route-end');
      $('#route-out').textContent = '请点击地图选择终点…';
    } else if (mode === 'route-end' && routeStart) {
      const s = routeStart;
      routeStart = null;
      setMode('none');
      void runRoutePlan(s, p);
    } else if (mode === 'challenge' && activeScene) {
      challengeLayer.clearLayers();
      const targetPt = { x: activeScene.x, y: activeScene.y };
      const res = wasm.scoreScreenshotChallenge(targetPt.x, targetPt.y, p.x, p.y);
      lastScore = res;
      L.polyline([toLatLng(p), toLatLng(targetPt)], { color: '#00e5ff', dashArray: '6 4', weight: 2 }).addTo(
        challengeLayer,
      );
      L.circleMarker(toLatLng(p), { radius: 7, color: '#fff', fillColor: '#ff4fa3', fillOpacity: 1 })
        .bindTooltip('你的猜测')
        .addTo(challengeLayer);
      L.circleMarker(toLatLng(targetPt), { radius: 8, color: '#fff', fillColor: '#22c55e', fillOpacity: 1 })
        .bindTooltip(`真实位置: ${activeScene.title}`)
        .addTo(challengeLayer);
      $('#challenge-result').innerHTML = `得分 <b>${res.score} / 5000</b>（评级 <b>${res.grade}</b>）<br/>距离误差: <b>${formatDistance(res.distance_m)}</b>`;
      ($<HTMLButtonElement>('#challenge-submit')).disabled = false;
      setMode('none');
    }
  });
  map.on('dblclick', () => {
    if (mode === 'measure' || mode === 'area') setMode('none');
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') setMode('none');
  });

  // ---------- IRL → IG ----------
  const irlLayer = L.layerGroup().addTo(toolLayer);
  $('#irl-predict').onclick = () => {
    const m = /(-?\d+(?:\.\d+)?)\s*[, ]\s*(-?\d+(?:\.\d+)?)/.exec($<HTMLInputElement>('#irl-input').value);
    if (!m) {
      $('#irl-out').textContent = '格式: 纬度, 经度';
      return;
    }
    const pred = landmarkStore.predictIg({ lat: Number(m[1]), lon: Number(m[2]) });
    irlLayer.clearLayers();
    if (!pred) {
      $('#irl-out').textContent = '无法拟合';
      return;
    }
    const r = Math.max(50, pred.rms * 2);
    L.circle(toLatLng(pred.point), { radius: r, color: '#7c4dff', weight: 2, fillOpacity: 0.15 }).addTo(irlLayer);
    L.circleMarker(toLatLng(pred.point), { radius: 6, color: '#fff', fillColor: '#7c4dff', fillOpacity: 1 }).addTo(
      irlLayer,
    );
    map.flyTo(toLatLng(pred.point), 4.5);
    const near = landmarkStore.nearest(pred.point, 3);
    $('#irl-out').innerHTML = `预测游戏坐标 <b>${formatWorld(pred.point)}</b> · 局部拟合残差 ±${pred.rms.toFixed(0)} m（${pred.neighbours} 个锚点）<br/>最近地标: ${near.map((n) => `${esc(n.landmark.id)} ${esc(n.landmark.name)}`).join('; ')}`;
  };

  // ---------- Export ----------
  const download = (name: string, data: unknown) => {
    const a = document.createElement('a');
    a.href = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' }));
    a.download = name;
    a.click();
    URL.revokeObjectURL(a.href);
  };
  $('#export-geojson').onclick = () => download('leonida-landmarks.geojson', landmarkStore.geojson(current));

  // ---------- User markers (auth/api.php + /api/markers + localStorage fallback) ----------
  const markerClient = new MarkerClient();
  let mine: UserMarker[] = await markerClient.list();

  function drawMine() {
    mineLayer.clearLayers();
    const list = $('#mine-list');
    list.innerHTML = '';
    for (const m of mine) {
      const color = m.color || settings.markerCustomColor;
      L.circleMarker(toLatLng(m), {
        radius: 8,
        color: '#ffffff',
        weight: 2,
        fillColor: color,
        fillOpacity: 0.95,
      })
        .bindPopup(`<b>${esc(m.name)}</b><br/>${formatWorld(m)}${m.note ? `<br/>${esc(m.note)}` : ''}`)
        .addTo(mineLayer);
      const li = document.createElement('li');
      li.style.borderLeftColor = color;
      li.innerHTML = `<div class="li-top">${esc(m.name)}</div><div class="muted">${formatWorld(m)}</div>`;
      const del = document.createElement('button');
      del.textContent = '删除';
      del.className = 'mini';
      del.onclick = async (ev) => {
        ev.stopPropagation();
        await markerClient.remove(m.id);
        mine = mine.filter((x) => x.id !== m.id);
        drawMine();
      };
      li.appendChild(del);
      li.onclick = () => map.flyTo(toLatLng(m), Math.max(map.getZoom(), 5));
      list.appendChild(li);
    }
  }
  map.on('contextmenu', async (e: L.LeafletMouseEvent) => {
    if (mode === 'challenge') return;
    const name = prompt('标记名称', `标记 ${mine.length + 1}`);
    if (!name) return;
    const p = toWorld(e.latlng);
    const created = await markerClient.create({
      id: crypto.randomUUID(),
      name,
      x: Math.round(p.x),
      y: Math.round(p.y),
      color: settings.markerCustomColor,
      createdAt: Date.now(),
    });
    mine.push(created);
    drawMine();
  });
  $('#mine-export').onclick = async () => {
    const data = await markerClient.exportJson(mine);
    download('leonida-my-markers.json', data);
  };
  $<HTMLInputElement>('#mine-import').onchange = async (e) => {
    const f = (e.target as HTMLInputElement).files?.[0];
    if (!f) return;
    try {
      const data = JSON.parse(await f.text()) as { markers?: UserMarker[] };
      const valid = (data.markers ?? [])
        .filter((m) => typeof m.name === 'string' && Number.isFinite(m.x) && Number.isFinite(m.y))
        .map((m) => ({
          ...m,
          id: m.id ?? crypto.randomUUID(),
          createdAt: typeof m.createdAt === 'number' ? m.createdAt : Date.now(),
        }));
      mine = await markerClient.importBatch(valid);
      drawMine();
    } catch {
      alert('导入失败：不是有效 JSON');
    }
  };
  $('#mine-clear').onclick = async () => {
    if (confirm('确定清除全部个人标记？')) {
      await markerClient.clearAll(mine);
      mine = [];
      drawMine();
    }
  };
  drawMine();

  // ---------- Media Library ----------
  let mediaLoaded = false;
  async function loadMediaView(activeType?: string) {
    const tabs = await fetchMediaTabs();
    const currentType = activeType || tabs[0]?.key || 'trailer1';
    const tabsEl = $('#media-tabs');
    tabsEl.innerHTML = '';
    for (const t of tabs) {
      const b = document.createElement('button');
      b.className = `chip ${t.key === currentType ? 'on' : ''}`;
      b.textContent = t.label + (t.count !== undefined ? ` (${t.count})` : '');
      b.onclick = () => loadMediaView(t.key);
      tabsEl.appendChild(b);
    }
    const items = await fetchMediaFiles(currentType);
    const grid = $('#media-grid');
    if (!items.length) {
      grid.innerHTML = `<div class="glass-card muted">当前分类暂无媒体条目（等待 build-sol-data 生成或服务端数据）。</div>`;
      return;
    }
    grid.innerHTML = '';
    for (const item of items.slice(0, 120)) {
      const card = document.createElement('div');
      card.className = 'glass-card media-card';
      const hasCoords = Number.isFinite(item.x) && Number.isFinite(item.y);
      card.innerHTML = `
        ${item.mediaUrl ? `<img src="${esc(item.mediaUrl)}" alt="${esc(item.title)}" loading="lazy" />` : ''}
        <div class="li-top"><b>${esc(item.title)}</b></div>
        <div class="muted">${esc(item.dataset)} · ${esc(item.district ? translateDistrict(item.district, lang) : item.category ?? '')}</div>
        ${item.description ? `<p class="muted">${esc(item.description)}</p>` : ''}
        ${hasCoords ? `<div class="row"><button class="jump-btn">${esc(tr(lang, 'jumpToMap'))} (${item.x.toFixed(0)}, ${item.y.toFixed(0)})</button></div>` : ''}`;
      const jb = card.querySelector('.jump-btn') as HTMLButtonElement | null;
      if (jb) jb.onclick = () => jumpToCoords(item.x, item.y);
      grid.appendChild(card);
    }
    mediaLoaded = true;
  }

  // ---------- News & Changelog ----------
  let newsLoaded = false;
  async function loadNewsView() {
    if (newsLoaded) return;
    const [news, logs, cred] = await Promise.all([fetchNews(), fetchChangelog(), fetchCredits()]);
    const newsEl = $('#news-list');
    newsEl.innerHTML = news.length
      ? ''
      : `<div class="glass-card muted">暂无新闻数据（data/news.json）。</div>`;
    for (const n of news) {
      const c = document.createElement('div');
      c.className = 'glass-card';
      const hasCoords = Number.isFinite(n.x) && Number.isFinite(n.y);
      c.innerHTML = `
        <div class="li-top"><b>${esc(n.title)}</b> <span class="muted">${esc(n.date ?? '')}</span></div>
        ${n.summary ? `<p class="muted">${esc(n.summary)}</p>` : ''}
        ${hasCoords ? `<button class="jump-btn">${esc(tr(lang, 'jumpToMap'))}</button>` : ''}`;
      const jb = c.querySelector('.jump-btn') as HTMLButtonElement | null;
      if (jb && n.x !== undefined && n.y !== undefined) jb.onclick = () => jumpToCoords(n.x!, n.y!);
      newsEl.appendChild(c);
    }

    const logEl = $('#changelog-list');
    logEl.innerHTML = logs.length
      ? ''
      : `<div class="glass-card muted">暂无更新日志（data/database_changelog.json）。</div>`;
    for (const l of logs.slice(0, 20)) {
      const c = document.createElement('div');
      c.className = 'glass-card';
      c.innerHTML = `
        <div class="li-top"><b>${esc(l.version ?? l.title ?? 'Update')}</b> <span class="muted">${esc(l.date ?? '')}</span></div>
        ${l.description ? `<p class="muted">${esc(l.description)}</p>` : ''}
        ${l.changes?.length ? `<ul class="muted">${l.changes.map((x) => `<li>${esc(x)}</li>`).join('')}</ul>` : ''}`;
      logEl.appendChild(c);
    }

    $('#credits-box').innerHTML = `
      <div><b>Credits:</b> ${cred.credits.map(esc).join(', ') || 'gta6map, gtamaplib-vc, State of Leonida'}</div>
      <div class="muted" style="margin-top:6px"><b>Supporters:</b> ${cred.supporters.map(esc).join(', ') || 'Community Contributors'}</div>`;
    newsLoaded = true;
  }

  // ---------- Leaderboard & Notifications ----------
  async function loadCommunityView() {
    const [lb, gstats, ranks, notifs] = await Promise.all([
      fetchLeaderboard(10),
      fetchGameStats(),
      fetchRanks(),
      fetchNotifications(),
    ]);
    $('#game-stats-bar').innerHTML = `
      <div class="stat-box">总游玩: <b>${gstats.total_plays ?? 0}</b></div>
      <div class="stat-box">最高分: <b>${gstats.best_score ?? 0}</b></div>
      <div class="stat-box">平均分: <b>${gstats.avg_score ?? 0}</b></div>`;
    $('#leaderboard-table').innerHTML = lb.length
      ? `<table class="lb-table"><thead><tr><th>#</th><th>Player</th><th>Score</th><th>Grade</th><th>Error</th></tr></thead><tbody>
        ${lb.map((r, i) => `<tr><td>${r.rank ?? i + 1}</td><td>${esc(r.player)}</td><td><b>${r.score}</b></td><td>${esc(r.grade ?? '-')}</td><td>${r.distance_m !== undefined ? formatDistance(r.distance_m) : '-'}</td></tr>`).join('')}
        </tbody></table>`
      : `<div class="muted">暂无排行榜记录，快去“猜图挑战”创造第一条纪录！</div>`;
    $('#ranks-list').innerHTML = ranks
      .map((r) => `<span class="chip" style="border-color:${esc(r.color ?? '#8b8fa3')}">${esc(r.name)}</span>`)
      .join('');
    $('#notifications-list').innerHTML = notifs.length
      ? notifs
          .map(
            (n) => `<div class="glass-card">
              <div class="li-top"><b>${esc(n.title)}</b> ${n.read ? '' : '<span class="chip on">NEW</span>'}</div>
              <div class="muted">${esc(n.message)}</div>
            </div>`,
          )
          .join('')
      : `<div class="glass-card muted">暂无新通知</div>`;
  }
  $('#notif-read-btn').onclick = async () => {
    await markNotificationsRead();
    await loadCommunityView();
  };

  // ---------- 3D View ----------
  let view3d: Leonida3DView | null = null;
  function ensure3DView() {
    if (!view3d) {
      view3d = new Leonida3DView({
        container: $('#three-canvas-wrap'),
        landmarks3d,
        landmarks2d: landmarks,
        onSelectLandmark: (info) => {
          $('#three-info').innerHTML = `
            <div><b>${esc(info.id)} · ${esc(info.name)}</b></div>
            <div class="muted">坐标: ${info.x.toFixed(0)}, ${info.y.toFixed(0)}${info.height ? ` · 高度 ${info.height.toFixed(0)} m` : ''}</div>
            <div class="row"><button id="three-jump-2d" class="mini" style="position:static">跳转 2D 地图</button></div>`;
          $('#three-jump-2d').onclick = () => jumpToCoords(info.x, info.y);
        },
      });
    }
    setTimeout(() => view3d?.resize(), 50);
  }
  $('#three-mode-orbit').onclick = () => {
    view3d?.setRoamMode(false);
    $('#three-mode-orbit').classList.add('on');
    $('#three-mode-roam').classList.remove('on');
  };
  $('#three-mode-roam').onclick = () => {
    view3d?.setRoamMode(true);
    $('#three-mode-roam').classList.add('on');
    $('#three-mode-orbit').classList.remove('on');
  };

  // ---------- Photogrammetry Workbench ----------
  const photoCanvas = $<HTMLCanvasElement>('#photo-canvas');
  const photoCtx = photoCanvas.getContext('2d')!;
  let photoBgImage: HTMLImageElement | null = null;
  let photoImageName = 'sample-leonida.png';
  let photoObs: CameraObs[] = [];
  let lastSolvedPose: CameraPose | null = null;
  let lastFrustum: [number, number][] | null = null;

  interface LandmarkOption {
    label: string;
    x: number;
    y: number;
    z: number;
  }
  const photoChoices: LandmarkOption[] = [
    ...landmarks3d.map((l) => ({
      label: `[3D] ${l.id} ${l.name} (${l.xyz.x.toFixed(0)}, ${l.xyz.y.toFixed(0)}, z=${l.height.toFixed(0)})`,
      x: l.xyz.x,
      y: l.xyz.y,
      z: l.height,
    })),
    ...landmarks
      .filter((l) => l.ig)
      .map((l) => ({
        label: `[2D] ${l.id} ${l.name} (${l.ig!.x.toFixed(0)}, ${l.ig!.y.toFixed(0)})`,
        x: l.ig!.x,
        y: l.ig!.y,
        z: 20,
      })),
  ];
  const photoSelect = $<HTMLSelectElement>('#photo-landmark-select');
  photoSelect.innerHTML = photoChoices.map((c, i) => `<option value="${i}">${esc(c.label)}</option>`).join('');

  function drawPhotoCanvas() {
    const w = photoCanvas.width;
    const h = photoCanvas.height;
    photoCtx.clearRect(0, 0, w, h);
    if (photoBgImage) {
      photoCtx.drawImage(photoBgImage, 0, 0, w, h);
    } else {
      const grad = photoCtx.createLinearGradient(0, 0, 0, h);
      grad.addColorStop(0, '#181030');
      grad.addColorStop(0.55, '#2b193d');
      grad.addColorStop(1, '#0d1526');
      photoCtx.fillStyle = grad;
      photoCtx.fillRect(0, 0, w, h);
      photoCtx.strokeStyle = 'rgba(0, 229, 255, 0.16)';
      for (let gx = 0; gx < w; gx += 40) {
        photoCtx.beginPath();
        photoCtx.moveTo(gx, 0);
        photoCtx.lineTo(gx, h);
        photoCtx.stroke();
      }
      for (let gy = 0; gy < h; gy += 40) {
        photoCtx.beginPath();
        photoCtx.moveTo(0, gy);
        photoCtx.lineTo(w, gy);
        photoCtx.stroke();
      }
      photoCtx.fillStyle = 'rgba(255,255,255,0.55)';
      photoCtx.font = '14px sans-serif';
      photoCtx.fillText('点击画面添加像素观测点（可先上传真实截图或点击下方控制点）', 20, 32);
    }
    photoObs.forEach((o, idx) => {
      photoCtx.beginPath();
      photoCtx.arc(o.px, o.py, 7, 0, Math.PI * 2);
      photoCtx.fillStyle = '#ff4fa3';
      photoCtx.fill();
      photoCtx.lineWidth = 2;
      photoCtx.strokeStyle = '#ffffff';
      photoCtx.stroke();
      photoCtx.fillStyle = '#ffffff';
      photoCtx.font = 'bold 12px monospace';
      photoCtx.fillText(`${idx + 1}`, o.px + 10, o.py + 4);
    });
  }

  function renderPhotoObsList() {
    const ul = $('#photo-obs-list');
    ul.innerHTML = photoObs.length
      ? ''
      : `<li class="muted">暂无控制点，请在左侧画布点击并绑定地标（至少 3 个）。</li>`;
    photoObs.forEach((o, i) => {
      const li = document.createElement('li');
      li.innerHTML = `<span>#${i + 1} (${o.px.toFixed(0)}, ${o.py.toFixed(0)}) → ${esc(o.label ?? '')}</span>`;
      const del = document.createElement('button');
      del.textContent = '删除';
      del.onclick = () => {
        photoObs.splice(i, 1);
        renderPhotoObsList();
        drawPhotoCanvas();
      };
      li.appendChild(del);
      ul.appendChild(li);
    });
  }

  async function renderPhotoProjects() {
    const projects = await listPhotogrammetryProjects();
    const ul = $('#photo-projects-list');
    ul.innerHTML = projects.length ? '' : `<li class="muted">暂无已保存的标定工程。</li>`;
    for (const pr of projects) {
      const li = document.createElement('li');
      li.innerHTML = `<span><b>${esc(pr.name)}</b> (${pr.observations.length} 点)</span>`;
      const row = document.createElement('span');
      const loadBtn = document.createElement('button');
      loadBtn.textContent = '载入';
      loadBtn.onclick = () => {
        photoObs = [...pr.observations];
        lastSolvedPose = pr.pose ?? null;
        lastFrustum = pr.frustum ?? null;
        renderPhotoObsList();
        drawPhotoCanvas();
        if (lastSolvedPose) showSolvedPoseOnUI(lastSolvedPose, lastFrustum ?? undefined);
      };
      const delBtn = document.createElement('button');
      delBtn.textContent = '删除';
      delBtn.onclick = async () => {
        if (pr.id) await deletePhotogrammetryProject(pr.id);
        await renderPhotoProjects();
      };
      row.append(loadBtn, delBtn);
      li.appendChild(row);
      ul.appendChild(li);
    }
  }

  function showSolvedPoseOnUI(pose: CameraPose, frustum?: [number, number][]) {
    lastSolvedPose = pose;
    lastFrustum =
      frustum ?? wasm.frustumPolygon(pose.x, pose.y, pose.yaw_deg, pose.fov_deg, 2500);
    $('#photo-out').innerHTML = `相机位置 <b>${formatWorld(pose)}</b> · Z=${pose.z.toFixed(1)} m<br/>Yaw ${pose.yaw_deg.toFixed(1)}° · Pitch ${pose.pitch_deg.toFixed(1)}° · FOV ${pose.fov_deg.toFixed(1)}° · RMSE ${pose.rmse_px.toFixed(1)} px`;
    ($<HTMLButtonElement>('#photo-show-on-map')).disabled = false;
    photoLayer.clearLayers();
    L.polygon(
      lastFrustum.map(([x, y]) => toLatLng({ x, y })),
      { color: '#ffb347', weight: 2, fillColor: '#ffb347', fillOpacity: 0.2 },
    ).addTo(photoLayer);
    L.circleMarker(toLatLng(pose), { radius: 7, color: '#fff', fillColor: '#ffb347', fillOpacity: 1 })
      .bindTooltip(`相机位置 (${pose.yaw_deg.toFixed(0)}°)`)
      .addTo(photoLayer);
  }

  photoCanvas.addEventListener('click', (e) => {
    const rect = photoCanvas.getBoundingClientRect();
    const px = ((e.clientX - rect.left) / rect.width) * photoCanvas.width;
    const py = ((e.clientY - rect.top) / rect.height) * photoCanvas.height;
    const choice = photoChoices[Number(photoSelect.value)] ?? photoChoices[0];
    if (!choice) return;
    photoObs.push({
      world_x: choice.x,
      world_y: choice.y,
      world_z: choice.z,
      px,
      py,
      label: choice.label.slice(0, 28),
    });
    renderPhotoObsList();
    drawPhotoCanvas();
  });

  $<HTMLInputElement>('#photo-upload').onchange = (e) => {
    const f = (e.target as HTMLInputElement).files?.[0];
    if (!f) return;
    photoImageName = f.name;
    const img = new Image();
    img.onload = () => {
      photoBgImage = img;
      photoCanvas.width = Math.min(1280, img.naturalWidth || 800);
      photoCanvas.height = Math.round(
        photoCanvas.width * ((img.naturalHeight || 450) / (img.naturalWidth || 800)),
      );
      drawPhotoCanvas();
    };
    img.src = URL.createObjectURL(f);
  };
  $('#photo-sample').onclick = () => {
    photoBgImage = null;
    photoImageName = 'sample-leonida.png';
    photoCanvas.width = 800;
    photoCanvas.height = 450;
    drawPhotoCanvas();
  };
  $('#photo-solve').onclick = async () => {
    if (photoObs.length < 3) {
      $('#photo-out').textContent = '请至少标定 3 个控制点';
      return;
    }
    const fov = Number($<HTMLInputElement>('#photo-fov').value) || 60;
    const remote = await solveCameraPoseRemote(photoObs, photoCanvas.width, photoCanvas.height, fov);
    if (remote?.pose) {
      showSolvedPoseOnUI(remote.pose, remote.frustum);
      return;
    }
    const localPose = wasm.solveCameraPose(photoObs, photoCanvas.width, photoCanvas.height, fov);
    if (!localPose) {
      $('#photo-out').textContent = '解算失败，请检查控制点分布';
      return;
    }
    showSolvedPoseOnUI(localPose);
  };
  $('#photo-show-on-map').onclick = () => {
    if (!lastSolvedPose) return;
    jumpToCoords(lastSolvedPose.x, lastSolvedPose.y);
  };
  $('#photo-save').onclick = async () => {
    const name = $<HTMLInputElement>('#photo-project-name').value.trim() || `标定工程 ${Date.now()}`;
    await savePhotogrammetryProject({
      name,
      image_name: photoImageName,
      width: photoCanvas.width,
      height: photoCanvas.height,
      observations: photoObs,
      pose: lastSolvedPose ?? undefined,
      frustum: lastFrustum ?? undefined,
    });
    await renderPhotoProjects();
  };
  drawPhotoCanvas();
  renderPhotoObsList();

  // ---------- Companion Checklist (PWA + SQLite + localStorage) ----------
  let companionItems: CollectibleItem[] = [];
  let companionCat = 'ALL';

  async function loadCompanionView() {
    if (!companionItems.length) {
      companionItems = await fetchCompanionChecklist();
    }
    renderCompanionView();
  }

  function renderCompanionView() {
    const st = computeCompanionStats(companionItems);
    $('#comp-progress-text').textContent = `已完成 ${st.completed} / ${st.total} (${st.percent}%)`;
    $('#comp-progress-bar').style.width = `${st.percent}%`;

    const cats = ['ALL', ...new Set(companionItems.map((i) => i.category))];
    const catsEl = $('#comp-cats');
    catsEl.innerHTML = '';
    for (const c of cats) {
      const b = document.createElement('button');
      b.className = `chip ${c === companionCat ? 'on' : ''}`;
      b.textContent = c === 'ALL' ? '全部' : c;
      b.onclick = () => {
        companionCat = c;
        renderCompanionView();
      };
      catsEl.appendChild(b);
    }

    const listEl = $('#comp-list');
    listEl.innerHTML = '';
    const filtered =
      companionCat === 'ALL'
        ? companionItems
        : companionItems.filter((i) => i.category === companionCat);

    for (const item of filtered) {
      const card = document.createElement('div');
      card.className = `glass-card comp-card ${item.completed ? 'done' : ''}`;
      card.innerHTML = `
        <label class="toggle-row">
          <input type="checkbox" ${item.completed ? 'checked' : ''} />
          <b>${esc(item.title)}</b>
        </label>
        <div class="muted">${esc(item.category)}${item.district ? ` · ${esc(translateDistrict(item.district, lang))}` : ''} · (${item.x.toFixed(0)}, ${item.y.toFixed(0)})</div>
        <div class="row"><button class="jump-btn">定位到地图</button></div>`;
      const cb = card.querySelector('input')!;
      cb.onchange = async () => {
        companionItems = await toggleCompanionItem(item.id, cb.checked, companionItems);
        renderCompanionView();
      };
      (card.querySelector('.jump-btn') as HTMLButtonElement).onclick = () =>
        jumpToCoords(item.x, item.y);
      listEl.appendChild(card);
    }
  }

  $('#comp-export').onclick = () =>
    download('leonida-companion-progress.json', {
      schema: 'leonida-companion-v1',
      items: companionItems,
    });
  $<HTMLInputElement>('#comp-import').onchange = async (e) => {
    const f = (e.target as HTMLInputElement).files?.[0];
    if (!f) return;
    try {
      const raw = JSON.parse(await f.text()) as { items?: CollectibleItem[] } | CollectibleItem[];
      const items = Array.isArray(raw) ? raw : (raw.items ?? []);
      if (!items.length) throw new Error('empty');
      companionItems = await importCompanionChecklist(items);
      renderCompanionView();
    } catch {
      alert('导入失败：无效的进度 JSON');
    }
  };

  // ---------- RAGE Tools Workbench ----------
  const SAMPLE_HANDLING: Record<string, HandlingItem> = {
    BANSHEE: {
      handlingName: 'BANSHEE',
      fMass: 1450,
      fInitialDragCoeff: 8.5,
      fDriveBiasFront: 0,
      nInitialDriveGears: 6,
      fInitialDriveForce: 0.34,
      fDriveMaxFlatVel: 168,
      fBrakeForce: 0.95,
      fTractionCurveMax: 2.55,
      fSuspensionForce: 2.3,
    },
    CHEETAH: {
      handlingName: 'CHEETAH',
      fMass: 1320,
      fInitialDragCoeff: 7.2,
      fDriveBiasFront: 0.15,
      nInitialDriveGears: 7,
      fInitialDriveForce: 0.39,
      fDriveMaxFlatVel: 185,
      fBrakeForce: 1.1,
      fTractionCurveMax: 2.8,
      fSuspensionForce: 2.5,
    },
    ELEGY2: {
      handlingName: 'ELEGY2',
      fMass: 1520,
      fInitialDragCoeff: 9.0,
      fDriveBiasFront: 0.3,
      nInitialDriveGears: 6,
      fInitialDriveForce: 0.33,
      fDriveMaxFlatVel: 162,
      fBrakeForce: 1.0,
      fTractionCurveMax: 2.65,
      fSuspensionForce: 2.2,
    },
  };
  let activeHandling: HandlingItem = { ...SAMPLE_HANDLING.BANSHEE! };

  function renderHandlingForm() {
    const form = $('#rage-handling-form');
    form.innerHTML = '';
    const fields: Array<[keyof HandlingItem, string, string]> = [
      ['handlingName', 'handlingName', 'text'],
      ['fMass', 'fMass (kg)', 'number'],
      ['fInitialDragCoeff', 'fInitialDragCoeff', 'number'],
      ['fDriveBiasFront', 'fDriveBiasFront (0-1)', 'number'],
      ['nInitialDriveGears', 'nInitialDriveGears', 'number'],
      ['fInitialDriveForce', 'fInitialDriveForce', 'number'],
      ['fDriveMaxFlatVel', 'fDriveMaxFlatVel', 'number'],
      ['fBrakeForce', 'fBrakeForce', 'number'],
      ['fTractionCurveMax', 'fTractionCurveMax', 'number'],
      ['fSuspensionForce', 'fSuspensionForce', 'number'],
    ];
    for (const [key, label, type] of fields) {
      const lbl = document.createElement('label');
      lbl.textContent = label;
      const inp = document.createElement('input');
      inp.type = type;
      if (type === 'number') inp.step = 'any';
      inp.value = String(activeHandling[key]);
      inp.oninput = () => {
        if (key === 'handlingName') activeHandling.handlingName = inp.value;
        else activeHandling[key] = Number(inp.value) || 0;
      };
      lbl.appendChild(inp);
      form.appendChild(lbl);
    }
  }
  renderHandlingForm();
  $<HTMLTextAreaElement>('#rage-handling-xml').value = wasm.exportHandlingXml([activeHandling]);

  $('#rage-load-sample').onclick = () => {
    const key = $<HTMLSelectElement>('#rage-sample-select').value;
    activeHandling = { ...(SAMPLE_HANDLING[key] ?? SAMPLE_HANDLING.BANSHEE!) };
    renderHandlingForm();
    $<HTMLTextAreaElement>('#rage-handling-xml').value = wasm.exportHandlingXml([activeHandling]);
  };
  $('#rage-parse-xml').onclick = async () => {
    const xml = $<HTMLTextAreaElement>('#rage-handling-xml').value;
    const items = (await parseHandlingRemote(xml)) ?? wasm.parseHandlingXml(xml);
    if (items.length) {
      activeHandling = { ...items[0]! };
      renderHandlingForm();
    } else {
      alert('未能从 XML 中解析出 <Item> 节点');
    }
  };
  $('#rage-export-xml').onclick = async () => {
    const xml =
      (await exportHandlingRemote([activeHandling])) ?? wasm.exportHandlingXml([activeHandling]);
    $<HTMLTextAreaElement>('#rage-handling-xml').value = xml;
  };

  // RPF7/RPF8 & RSC7 Header Inspector
  const runHeaderInspect = async () => {
    const hex = $<HTMLTextAreaElement>('#rage-header-hex').value;
    const res = (await inspectRageHeaderRemote(hex)) ?? wasm.inspectRageHeader(hex);
    $('#rage-header-out').innerHTML = `类型 <b>${esc(res.kind)}</b>${res.magic ? ` · 魔数 ${esc(res.magic)}` : ''}<br/>${
      res.entryCount !== undefined
        ? `条目数: ${res.entryCount} · 名称表: ${res.namesLength} B · 加密: ${esc(res.encryption ?? '-')}`
        : res.systemFlags !== undefined
          ? `SystemFlags: 0x${res.systemFlags.toString(16)} (${res.systemSize} B) · GraphicsFlags: 0x${(res.graphicsFlags ?? 0).toString(16)} (${res.graphicsSize} B)`
          : esc(res.details ?? '')
    }`;
  };
  $('#rage-sample-rpf7').onclick = () => {
    $<HTMLTextAreaElement>('#rage-header-hex').value =
      '37 46 50 52 2A 00 00 00 00 08 00 00 4F 50 45 4E';
    void runHeaderInspect();
  };
  $('#rage-sample-rsc7').onclick = () => {
    $<HTMLTextAreaElement>('#rage-header-hex').value =
      '52 53 43 37 0D 00 00 00 12 00 00 00 21 00 00 00';
    void runHeaderInspect();
  };
  $('#rage-inspect-btn').onclick = () => void runHeaderInspect();

  // JOAAT Calculator
  const runJoaat = () => {
    const text = $<HTMLInputElement>('#rage-joaat-in').value;
    const h = wasm.joaat(text);
    $('#rage-joaat-out').innerHTML = `HEX: <b>0x${h.toString(16).toUpperCase().padStart(8, '0')}</b> · DEC: <b>${h}</b>`;
  };
  $('#rage-joaat-btn').onclick = runJoaat;
  $('#rage-joaat-in').addEventListener('input', runJoaat);
  runJoaat();

  // ---------- Admin Studio ----------
  async function loadAdminView() {
    const [ov, cred] = await Promise.all([fetchAdminOverview(), fetchCredits()]);
    $('#admin-overview').innerHTML = `
      <div class="stat-box">地标: <b>${ov.landmarks ?? stats.total}</b></div>
      <div class="stat-box">个人标记: <b>${ov.markers ?? mine.length}</b></div>
      <div class="stat-box">标定工程: <b>${ov.projects ?? 0}</b></div>
      <div class="stat-box">评论: <b>${ov.comments ?? 0}</b></div>`;
    $('#admin-credits-box').innerHTML = `
      <div><b>Credits:</b> ${cred.credits.map(esc).join(', ') || 'State of Leonida, gta6map, gtamaplib-vc'}</div>
      <div class="muted" style="margin-top:8px"><b>Supporters:</b> ${cred.supporters.map(esc).join(', ') || 'Community Contributors'}</div>`;
  }
  $('#admin-integrity-btn').onclick = async () => {
    $('#admin-integrity-out').textContent = `完整性检查结果: ${await checkAdminIntegrity()}`;
  };
  $('#admin-export-btn').onclick = async () => {
    download('leonida-admin-db-export.json', await exportAdminDb());
  };
  $('#admin-news-publish').onclick = async () => {
    const title = $<HTMLInputElement>('#admin-news-title').value.trim();
    const summary = $<HTMLTextAreaElement>('#admin-news-summary').value.trim();
    if (!title) return;
    const ok = await publishAdminNews(title, summary);
    $('#admin-news-out').textContent = ok ? '新闻已发布至服务端 SQLite' : '离线模式：未能连接服务端';
    $<HTMLInputElement>('#admin-news-title').value = '';
    $<HTMLTextAreaElement>('#admin-news-summary').value = '';
  };

  // ---------- Topbar Module Switching ----------
  type ModuleName =
    | 'map'
    | 'view3d'
    | 'photo'
    | 'companion'
    | 'rage'
    | 'challenge'
    | 'media'
    | 'news'
    | 'community'
    | 'admin';
  function switchModule(mod: ModuleName) {
    document.querySelectorAll('#studio-nav button').forEach((b) => {
      b.classList.toggle('active', (b as HTMLElement).dataset.module === mod);
    });
    document.querySelectorAll('.module-view').forEach((v) => v.classList.remove('active'));

    if (mod === 'map' || mod === 'challenge') {
      $('#map-workspace').classList.add('active');
      $('#sidebar').classList.toggle('hidden', mod === 'challenge');
      $('#challenge-panel').classList.toggle('hidden', mod !== 'challenge');
      if (mod === 'challenge') {
        if (!activeScene) startChallengeRound();
        else setMode('challenge');
        refreshMiniLeaderboard();
      } else {
        challengeLayer.clearLayers();
        setMode('none');
      }
      setTimeout(() => map.invalidateSize(), 50);
    } else if (mod === 'view3d') {
      $('#view-3d').classList.add('active');
      ensure3DView();
    } else if (mod === 'photo') {
      $('#view-photo').classList.add('active');
      void renderPhotoProjects();
    } else if (mod === 'companion') {
      $('#view-companion').classList.add('active');
      void loadCompanionView();
    } else if (mod === 'rage') {
      $('#view-rage').classList.add('active');
    } else if (mod === 'media') {
      $('#view-media').classList.add('active');
      if (!mediaLoaded) loadMediaView();
    } else if (mod === 'news') {
      $('#view-news').classList.add('active');
      loadNewsView();
    } else if (mod === 'community') {
      $('#view-community').classList.add('active');
      loadCommunityView();
    } else if (mod === 'admin') {
      $('#view-admin').classList.add('active');
      void loadAdminView();
    }
  }

  document.querySelectorAll<HTMLButtonElement>('#studio-nav button').forEach((btn) => {
    btn.onclick = () => switchModule(btn.dataset.module as ModuleName);
  });

  // ---------- Language Toggle ----------
  $('#lang-toggle').onclick = () => {
    lang = lang === 'zh' ? 'en' : 'zh';
    saveLang(lang);
    applyI18n();
    renderDatasetToggles();
  };

  // ---------- Settings Modal ----------
  const modal = $('#settings-modal');
  const syncModalInputs = () => {
    $<HTMLInputElement>('#set-bg-effects').checked = settings.bgEffects;
    $<HTMLInputElement>('#set-preloader').checked = settings.isPreloaderStyle;
    $<HTMLInputElement>('#set-show-labels').checked = settings.mapSettings.showLabels;
    $<HTMLInputElement>('#set-show-3d').checked = settings.mapSettings.show3dByDefault;
    $<HTMLInputElement>('#set-auto-fly').checked = settings.mapSettings.autoFly;
    $<HTMLSelectElement>('#set-accent').value = settings.accentColor;
    $<HTMLSelectElement>('#set-panel-style').value = settings.panelStyle;
    $<HTMLSelectElement>('#set-density').value = settings.listDensity;
    $<HTMLInputElement>('#set-bg-color').value = settings.selectedBackgroundColor;
    $<HTMLInputElement>('#set-marker-color').value = settings.markerCustomColor;
    $<HTMLInputElement>('#col-landmarks').value = settings.clusterCustomColors.landmarks;
    $<HTMLInputElement>('#col-trailer1').value = settings.clusterCustomColors.trailer1;
    $<HTMLInputElement>('#col-trailer2').value = settings.clusterCustomColors.trailer2;
    $<HTMLInputElement>('#col-trailer3').value = settings.clusterCustomColors.trailer3;
    $<HTMLInputElement>('#col-leaks').value = settings.clusterCustomColors.leaks;
    $<HTMLInputElement>('#col-screenshots').value = settings.clusterCustomColors.screenshots;
  };
  $('#btn-settings').onclick = () => {
    syncModalInputs();
    modal.classList.remove('hidden');
  };
  $('#settings-close').onclick = () => modal.classList.add('hidden');
  $('#settings-reset').onclick = () => {
    settings = structuredClone(DEFAULT_SETTINGS);
    syncModalInputs();
  };
  $('#settings-save').onclick = () => {
    settings = {
      ...settings,
      bgEffects: $<HTMLInputElement>('#set-bg-effects').checked,
      isPreloaderStyle: $<HTMLInputElement>('#set-preloader').checked,
      accentColor: $<HTMLSelectElement>('#set-accent').value as AccentPreset,
      panelStyle: $<HTMLSelectElement>('#set-panel-style').value as PanelStyle,
      listDensity: $<HTMLSelectElement>('#set-density').value as ListDensity,
      selectedBackgroundColor: $<HTMLInputElement>('#set-bg-color').value,
      markerCustomColor: $<HTMLInputElement>('#set-marker-color').value,
      mapSettings: {
        ...settings.mapSettings,
        showGrid: true,
        showLabels: $<HTMLInputElement>('#set-show-labels').checked,
        show3dByDefault: $<HTMLInputElement>('#set-show-3d').checked,
        autoFly: $<HTMLInputElement>('#set-auto-fly').checked,
      },
      clusterCustomColors: {
        landmarks: $<HTMLInputElement>('#col-landmarks').value,
        trailer1: $<HTMLInputElement>('#col-trailer1').value,
        trailer2: $<HTMLInputElement>('#col-trailer2').value,
        trailer3: $<HTMLInputElement>('#col-trailer3').value,
        leaks: $<HTMLInputElement>('#col-leaks').value,
        screenshots: $<HTMLInputElement>('#col-screenshots').value,
      },
    };
    saveStudioSettings(settings);
    applyTheme();
    renderSolLayers();
    renderDatasetToggles();
    drawMine();
    modal.classList.add('hidden');
  };

  // ---------- About ----------
  const modeLabel = landmarksPayload.fromServer ? 'Rust 服务端 (/api) + WASM' : '离线静态模式 (localStorage) + WASM';
  $('#about').innerHTML = `
    <p><b>State of Leonida · Map Studio v0.3</b>（${esc(modeLabel)}）</p>
    <p>地标 ${stats.total} 个（已定位 ${stats.positioned}，现实已确认 ${stats.byStatus.confirmed}），3D 地标 ${landmarks3d.length} 个，底图瓦片集 ${availableTilesets.length} 个。</p>
    <p>坐标系：Leonida 世界米制（x 向东、y 向北），与 gtamaplib / gta6map 社区摄影测量一致；为社区估算，<b>非官方测量</b>。</p>
    <p>数据来源：<a href="https://github.com/gta6map/gta6map.github.io" target="_blank" rel="noopener">gta6map.github.io</a>、<a href="https://github.com/rolux/gtamaplib-vc" target="_blank" rel="noopener">gtamaplib-vc</a>（均为 Public Domain）。</p>
    <p class="muted">非 Rockstar Games / Take-Two 官方产品。不包含任何游戏二进制或受版权保护的游戏资产。</p>`;
}

main().catch((err) => {
  console.error(err);
  document.body.insertAdjacentHTML(
    'afterbegin',
    `<div class="fatal">加载失败：${esc(String(err))}。请确认 leonida-wasm 已构建。</div>`,
  );
});

if ('serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('./sw.js').catch(() => undefined);
  });
}
