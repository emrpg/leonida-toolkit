import './style.css';
import {
  computeNamespaceStats,
  deleteBookmark,
  fetchBookmarks,
  loadAllNatives,
  saveBookmark,
} from './api';
import type { NativeBookmark, NativeEntry, StubLang } from './types';
import { generateShvStub } from './wasm';

const $ = <T extends HTMLElement = HTMLElement>(sel: string) => document.querySelector(sel) as T;
const esc = (s: unknown) =>
  String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);

async function main() {
  const [entries, initialBookmarks] = await Promise.all([loadAllNatives(), fetchBookmarks()]);
  let bookmarks: NativeBookmark[] = initialBookmarks;
  let activeNs = 'ALL';
  let searchText = '';
  let selected: NativeEntry | null = entries[0] ?? null;
  let activeLang: StubLang = 'cpp';

  const stats = computeNamespaceStats(entries);
  $('#ns-all-count').textContent = String(entries.length);

  function renderNamespaces() {
    $('#ns-bm-count').textContent = String(bookmarks.length);
    const box = $('#ns-list');
    box.innerHTML = '';
    for (const st of stats) {
      const btn = document.createElement('button');
      btn.className = `ns-item ${activeNs === st.namespace ? 'active' : ''}`;
      btn.innerHTML = `<span>${esc(st.namespace)}</span><span class="badge">${st.count}</span>`;
      btn.onclick = () => {
        activeNs = st.namespace;
        updateNsActive();
        renderList();
      };
      box.appendChild(btn);
    }
  }

  function updateNsActive() {
    document.querySelectorAll<HTMLElement>('.ns-item').forEach((el) => {
      el.classList.toggle('active', el.dataset.ns === activeNs);
    });
    renderNamespaces();
  }

  document.querySelectorAll<HTMLElement>('#ns-sidebar > .ns-item').forEach((el) => {
    el.onclick = () => {
      activeNs = el.dataset.ns!;
      updateNsActive();
      renderList();
    };
  });

  function renderList() {
    const t0 = performance.now();
    const q = searchText.trim().toLowerCase();
    const bmSet = new Set(bookmarks.map((b) => b.hash));

    const filtered = entries.filter((e) => {
      if (activeNs === 'BOOKMARKS' && !bmSet.has(e.hash)) return false;
      if (activeNs !== 'ALL' && activeNs !== 'BOOKMARKS' && e.namespace !== activeNs) return false;
      if (!q) return true;
      return (
        e.name.toLowerCase().includes(q) ||
        e.hash.toLowerCase().includes(q) ||
        (e.jhash && e.jhash.toLowerCase().includes(q)) ||
        e.description.toLowerCase().includes(q)
      );
    });

    const elapsed = (performance.now() - t0).toFixed(2);
    $('#search-meta').textContent = `${filtered.length} 条 (${elapsed} ms)`;

    const ul = $('#native-list');
    ul.innerHTML = '';
    for (const e of filtered.slice(0, 200)) {
      const li = document.createElement('li');
      if (selected?.hash === e.hash) li.classList.add('active');
      li.innerHTML = `<div class="n-name">${bmSet.has(e.hash) ? '★ ' : ''}${esc(e.name)}</div><div class="n-sub mono">${esc(e.namespace)} · ${esc(e.hash)}</div>`;
      li.onclick = () => {
        selected = e;
        renderList();
        renderDetail();
      };
      ul.appendChild(li);
    }
  }

  function renderDetail() {
    if (!selected) {
      $('#detail-empty').classList.remove('hidden');
      $('#detail-body').classList.add('hidden');
      return;
    }
    $('#detail-empty').classList.add('hidden');
    $('#detail-body').classList.remove('hidden');

    $('#d-ns').textContent = `${selected.namespace} -> ${selected.return_type}`;
    $('#d-name').textContent = selected.name;
    $('#d-hashes').textContent = `Hash: ${selected.hash}${selected.jhash ? ` · JHash: ${selected.jhash}` : ''}`;
    $('#d-desc').textContent = selected.description || '（无描述）';

    const existingBm = bookmarks.find((b) => b.hash === selected!.hash);
    $<HTMLInputElement>('#bookmark-note').value = existingBm?.note ?? '';
    $('#toggle-bookmark').textContent = existingBm ? '★ 已收藏' : '☆ 收藏';

    const tbl = $('#d-params');
    tbl.innerHTML = selected.params.length
      ? `<thead><tr><th>#</th><th>参数名</th><th>类型</th></tr></thead><tbody>${selected.params
          .map((p, i) => `<tr><td>${i + 1}</td><td class="mono">${esc(p.name)}</td><td class="mono accent">${esc(p.param_type)}</td></tr>`)
          .join('')}</tbody>`
      : `<tbody><tr><td class="muted">无参数 (void)</td></tr></tbody>`;

    $('#stub-code').textContent = generateShvStub(selected, activeLang);
  }

  $('#native-search').addEventListener('input', (e) => {
    searchText = (e.target as HTMLInputElement).value;
    renderList();
  });

  document.querySelectorAll<HTMLButtonElement>('.lang-tabs button').forEach((b) => {
    b.onclick = () => {
      activeLang = b.dataset.lang as StubLang;
      document.querySelectorAll('.lang-tabs button').forEach((x) => x.classList.remove('on'));
      b.classList.add('on');
      if (selected) $('#stub-code').textContent = generateShvStub(selected, activeLang);
    };
  });

  $('#copy-stub').onclick = async () => {
    const code = $('#stub-code').textContent ?? '';
    await navigator.clipboard?.writeText(code).catch(() => undefined);
    const btn = $('#copy-stub');
    btn.textContent = '已复制 ✓';
    setTimeout(() => {
      btn.textContent = '一键复制';
    }, 1200);
  };

  $('#toggle-bookmark').onclick = async () => {
    if (!selected) return;
    const exists = bookmarks.some((b) => b.hash === selected!.hash);
    if (exists) {
      bookmarks = await deleteBookmark(selected.hash);
    } else {
      const note = $<HTMLInputElement>('#bookmark-note').value.trim();
      bookmarks = await saveBookmark({
        hash: selected.hash,
        namespace: selected.namespace,
        name: selected.name,
        note: note || undefined,
      });
    }
    renderNamespaces();
    renderList();
    renderDetail();
  };

  // ScriptHookV ABI Modal
  $('#btn-shv-abi').onclick = () => $('#shv-modal').classList.remove('hidden');
  $('#shv-close').onclick = () => $('#shv-modal').classList.add('hidden');

  renderNamespaces();
  renderList();
  renderDetail();
}

main().catch(console.error);
