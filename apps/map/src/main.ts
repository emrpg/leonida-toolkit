import L from 'leaflet';
import 'leaflet/dist/leaflet.css';
import './style.css';
import {
  MAX_TILE_Z,
  TILE_SIZE,
  WORLD_EXTENT,
  bearing,
  formatDistance,
  formatWorld,
  idSortKey,
  type IrlStatus,
  type Landmark,
  type Landmark3D,
  type Ray2D,
  type UserMarker,
  type WorldPoint,
} from './types';
import {
  MarkerClient,
  fetchLandmarks3D,
  fetchLandmarksData,
  fetchTileManifest,
} from './api';
import { initLeonidaWasm, type WasmTileResolverHandle } from './wasm-bridge';

// ---------- CRS: Leonida world metres, matching the gta6map tile pyramid ----------
// At Leaflet zoom z (scale 2^z): px = 2^z * (x + 16384) / 32,  py = 2^z * (16384 - y) / 32
const k = 1 / (WORLD_EXTENT.size / TILE_SIZE);
const LeonidaCRS = L.extend({}, L.CRS.Simple, {
  transformation: new L.Transformation(k, WORLD_EXTENT.half * k, -k, WORLD_EXTENT.half * k),
}) as L.CRS;

const toLatLng = (p: WorldPoint) => L.latLng(p.y, p.x);
const toWorld = (ll: L.LatLng): WorldPoint => ({ x: ll.lng, y: ll.lat });

const $ = <T extends HTMLElement = HTMLElement>(sel: string) => document.querySelector(sel) as T;
const esc = (s: string) =>
  s.replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);

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

async function main() {
  const [wasm, landmarksPayload, vc, manifest] = await Promise.all([
    initLeonidaWasm(),
    fetchLandmarksData(),
    fetchLandmarks3D(),
    fetchTileManifest(),
  ]);

  const landmarkStore =
    landmarksPayload.kind === 'normalized'
      ? wasm.createLandmarksFromNormalized(landmarksPayload.landmarks)
      : wasm.createLandmarksFromTuples(landmarksPayload.rows);

  const landmarks = [...landmarkStore.all()].sort((a, b) => idSortKey(a.id).localeCompare(idSortKey(b.id)));
  const landmarks3d: Landmark3D[] = vc ? wasm.parseGtamaplibVc(vc) : [];
  const resolver = wasm.createTileResolver(manifest);
  const stats = landmarkStore.stats();

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
    .addAttribution('地标与底图: gta6map / gtamaplib 社区 (Public Domain) · 非 Rockstar 官方地图');

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
  const layer3d = L.layerGroup();
  const mineLayer = L.layerGroup().addTo(map);
  const toolLayer = L.layerGroup().addTo(map);
  L.control
    .layers(
      basemaps,
      { 地标: landmarkLayer, '3D 三角测量地标 (gtamaplib-vc)': layer3d, 我的标记: mineLayer },
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
  });
  map.on('mousemove', (e: L.LeafletMouseEvent) => {
    $('#coords').textContent = `X ${e.latlng.lng.toFixed(0)}  Y ${e.latlng.lat.toFixed(0)}  (m) · zoom ${map.getZoom()}`;
  });

  // ---------- Landmarks ----------
  const markerById = new Map<string, L.CircleMarker>();
  const popupHtml = (l: Landmark) => {
    const near = l.ig ? landmarkStore.nearest(l.ig, 4).slice(1) : [];
    const gmaps = l.irl ? `https://www.google.com/maps?q=${l.irl.lat},${l.irl.lon}` : null;
    return `<div class="pop">
      <div class="pop-id" style="border-color:#${l.color}">${esc(l.id)} · ${esc(STATUS_LABEL[l.irlStatus])}</div>
      <div class="pop-name">${esc(l.name)}</div>
      <div><b>游戏地址</b> ${esc(l.igAddress)}</div>
      ${l.ig ? `<div><b>游戏坐标</b> ${formatWorld(l.ig)}</div>` : ''}
      <div><b>现实原型</b> ${esc(l.irlAddress)}</div>
      ${gmaps ? `<div><a href="${gmaps}" target="_blank" rel="noopener">在 Google 地图中查看现实位置 ↗</a></div>` : ''}
      <div><b>标签</b> ${l.tags.map(esc).join(', ') || '—'}</div>
      ${near.length ? `<div class="muted">附近: ${near.map((n) => `${esc(n.landmark.id)} (${formatDistance(n.distance)})`).join(', ')}</div>` : ''}
      <div class="muted">更新: ${new Date(l.editedAt * 1000).toLocaleDateString()}</div>
    </div>`;
  };
  for (const l of landmarks) {
    if (!l.ig) continue;
    const m = L.circleMarker(toLatLng(l.ig), {
      radius: 6,
      color: '#111',
      weight: 1.5,
      fillColor: `#${l.color}`,
      fillOpacity: 0.95,
      dashArray: l.irlStatus === 'confirmed' ? undefined : '2 2',
    })
      .bindPopup(() => popupHtml(l), { maxWidth: 340 })
      .bindTooltip(`${l.id} ${l.name}`, { direction: 'top', offset: [0, -6] });
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
  function refresh() {
    current = [...landmarkStore.filter({ text: state.text, tags: [...state.tags], statuses: [...state.statuses] })].sort(
      (a, b) => idSortKey(a.id).localeCompare(idSortKey(b.id)),
    );
    landmarkLayer.clearLayers();
    for (const l of current) markerById.get(l.id)?.addTo(landmarkLayer);
    $('#result-count').textContent = `显示 ${current.length} / ${stats.total} 个地标（已定位 ${current.filter((l) => l.ig).length}）`;
    const list = $('#landmark-list');
    list.innerHTML = '';
    for (const l of current.slice(0, 400)) {
      const li = document.createElement('li');
      li.style.borderLeftColor = `#${l.color}`;
      li.innerHTML = `<div class="li-top"><span class="li-id">${esc(l.id)}</span> ${esc(l.name)}</div><div class="muted">${esc(l.district ?? '')} · ${esc(l.irlAddress)}</div>`;
      if (!l.ig) li.classList.add('unpositioned');
      li.onclick = () => focusLandmark(l);
      list.appendChild(li);
    }
  }
  function focusLandmark(l: Landmark) {
    if (!l.ig) return;
    map.flyTo(toLatLng(l.ig), Math.max(map.getZoom(), 5), { duration: 0.6 });
    markerById.get(l.id)?.openPopup();
  }
  refresh();

  // ---------- Tabs ----------
  document.querySelectorAll<HTMLButtonElement>('#tabs button').forEach((btn) => {
    btn.onclick = () => {
      document.querySelectorAll('#tabs button, .tab').forEach((el) => el.classList.remove('active'));
      btn.classList.add('active');
      $(`#tab-${btn.dataset.tab}`).classList.add('active');
    };
  });

  // ---------- Tools: click-mode state machine ----------
  type Mode = 'none' | 'measure' | 'area' | 'ray-origin' | 'ray-target';
  let mode: Mode = 'none';
  let measurePts: WorldPoint[] = [];
  const measureLayer = L.layerGroup().addTo(toolLayer);
  const rayLayer = L.layerGroup().addTo(toolLayer);
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

  map.on('click', (e: L.LeafletMouseEvent) => {
    const p = toWorld(e.latlng);
    if (mode === 'measure' || mode === 'area') {
      measurePts.push(p);
      drawMeasure();
    } else if (mode === 'ray-origin') {
      rayOrigin = p;
      setMode('ray-target');
      $('#ray-out').textContent = '点击视线方向上的一点…';
    } else if (mode === 'ray-target' && rayOrigin) {
      rays.push({ origin: rayOrigin, bearing: bearing(rayOrigin, p) });
      rayOrigin = null;
      setMode('none');
      drawRays();
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

  // ---------- User markers (REST API + localStorage fallback) ----------
  const markerClient = new MarkerClient();
  let mine: UserMarker[] = await markerClient.list();

  function drawMine() {
    mineLayer.clearLayers();
    const list = $('#mine-list');
    list.innerHTML = '';
    for (const m of mine) {
      L.marker(toLatLng(m), { title: m.name })
        .bindPopup(`<b>${esc(m.name)}</b><br/>${formatWorld(m)}${m.note ? `<br/>${esc(m.note)}` : ''}`)
        .addTo(mineLayer);
      const li = document.createElement('li');
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
    const name = prompt('标记名称', `标记 ${mine.length + 1}`);
    if (!name) return;
    const p = toWorld(e.latlng);
    const created = await markerClient.create({
      id: crypto.randomUUID(),
      name,
      x: Math.round(p.x),
      y: Math.round(p.y),
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

  // ---------- About ----------
  const modeLabel = landmarksPayload.fromServer ? 'Rust 服务端 (/api) + WASM' : '离线静态模式 (localStorage) + WASM';
  $('#about').innerHTML = `
    <p><b>Leonida Toolkit · 互动地图 v0.2</b>（${esc(modeLabel)}）</p>
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
