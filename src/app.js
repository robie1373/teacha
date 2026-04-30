'use strict';

// ── Tauri invoke bridge ──────────────────────────────────────────────────────
// Wait for the Tauri webview to inject __TAURI__ before doing anything.

function waitForTauri(fn) {
  if (window.__TAURI__) { fn(window.__TAURI__.core.invoke); return; }
  let attempts = 0;
  const t = setInterval(() => {
    attempts++;
    if (window.__TAURI__) { clearInterval(t); fn(window.__TAURI__.core.invoke); }
    else if (attempts > 100) { clearInterval(t); fatalError('Tauri API unavailable.'); }
  }, 50);
}

function fatalError(msg) {
  document.body.innerHTML =
    `<div style="padding:40px;color:#ed8796;font-family:monospace">${msg}</div>`;
}

// ── App state ────────────────────────────────────────────────────────────────
let invoke;
let allCards = [];
let nowTs    = 0;

const filters = { search: '', state: 'due', tag: '' };
const sort    = { col: 'due', asc: true };

// ── Bootstrap ────────────────────────────────────────────────────────────────
waitForTauri(async (inv) => {
  invoke = inv;
  bindStaticEvents();
  await refresh();
});

// ── Data loading ─────────────────────────────────────────────────────────────
async function refresh() {
  nowTs = Math.floor(Date.now() / 1000);
  try {
    allCards = await invoke('get_all_cards');
  } catch (e) {
    toast('Error loading cards: ' + e);
    return;
  }
  updateStats();
  populateTagFilter();
  renderTable();
}

// ── Stats bar ────────────────────────────────────────────────────────────────
function updateStats() {
  const due      = allCards.filter(c => c.due_at <= nowTs).length;
  const newCards = allCards.filter(c => c.state === 'New').length;
  const learning = allCards.filter(c => c.state === 'Learning').length;
  const review   = allCards.filter(c => c.state === 'Review').length;
  const relearn  = allCards.filter(c => c.state === 'Relearning').length;

  document.getElementById('stat-due').textContent    = due;
  document.getElementById('stat-new').textContent    = newCards;
  document.getElementById('stat-learn').textContent  = learning;
  document.getElementById('stat-review').textContent = review;
  document.getElementById('stat-relearn').textContent = relearn;
  document.getElementById('stat-total').textContent  = allCards.length;
}

// ── Tag filter population ────────────────────────────────────────────────────
function populateTagFilter() {
  const sel = document.getElementById('filter-tag');
  const current = sel.value;

  const tags = new Set();
  allCards.forEach(c => {
    if (c.tags) c.tags.split(',').forEach(t => { const s = t.trim(); if (s) tags.add(s); });
  });

  sel.innerHTML = '<option value="">All tags</option>';
  [...tags].sort().forEach(t => {
    const opt = document.createElement('option');
    opt.value = t; opt.textContent = t;
    if (t === current) opt.selected = true;
    sel.appendChild(opt);
  });
}

// ── Table rendering ──────────────────────────────────────────────────────────
function applyFilters(cards) {
  let result = cards;

  // State / due filter (from stats bar)
  if (filters.state === 'due') {
    result = result.filter(c => c.due_at <= nowTs);
  } else if (filters.state) {
    result = result.filter(c => c.state === filters.state);
  }

  // Tag filter
  if (filters.tag) {
    result = result.filter(c =>
      c.tags && c.tags.split(',').map(t => t.trim()).includes(filters.tag)
    );
  }

  // Search
  if (filters.search) {
    const q = filters.search.toLowerCase();
    result = result.filter(c =>
      c.title.toLowerCase().includes(q) ||
      (c.prompt && c.prompt.toLowerCase().includes(q)) ||
      c.body.toLowerCase().includes(q)
    );
  }

  return result;
}

function applySort(cards) {
  return [...cards].sort((a, b) => {
    let av, bv;
    switch (sort.col) {
      case 'id':    av = a.id;    bv = b.id;    break;
      case 'title': av = a.title.toLowerCase(); bv = b.title.toLowerCase(); break;
      case 'state': av = stateOrder(a.state); bv = stateOrder(b.state); break;
      case 'due':   av = a.due_at; bv = b.due_at; break;
      default:      return 0;
    }
    if (av < bv) return sort.asc ? -1 : 1;
    if (av > bv) return sort.asc ?  1 : -1;
    return 0;
  });
}

function stateOrder(s) {
  return { New: 0, Learning: 1, Relearning: 2, Review: 3 }[s] ?? 4;
}

function renderTable() {
  const filtered = applyFilters(allCards);
  const sorted   = applySort(filtered);
  const tbody    = document.getElementById('card-tbody');
  const empty    = document.getElementById('empty-state');

  // Update sort indicators
  document.querySelectorAll('thead th.sortable').forEach(th => {
    const col = th.dataset.col;
    th.classList.toggle('sorted', col === sort.col);
    const arrow = th.querySelector('.sort-arrow');
    if (col === sort.col) arrow.textContent = sort.asc ? '↑' : '↓';
    else arrow.textContent = '↕';
  });

  if (sorted.length === 0) {
    tbody.innerHTML = '';
    empty.style.display = '';
    return;
  }
  empty.style.display = 'none';

  tbody.innerHTML = sorted.map(card => {
    const tags = card.tags
      ? card.tags.split(',').map(t => t.trim()).filter(Boolean)
          .map(t => `<span class="tag-chip">${esc(t)}</span>`).join('')
      : '—';

    const stateLabel = card.state ?? 'New';
    const stateBadge = `<span class="state-badge state-${stateLabel}">${stateLabel}</span>`;

    const due    = formatDue(card.due_at, nowTs);
    const dueClass = card.due_at <= nowTs ? 'due-now'
                   : card.due_at - nowTs < 86400 ? 'due-soon'
                   : 'due-later';

    return `<tr>
      <td class="col-id">${card.id}</td>
      <td class="col-title"><span class="cell-title" title="${esc(card.title)}">${esc(card.title)}</span></td>
      <td class="col-prompt"><span class="cell-prompt" title="${esc(card.prompt || '')}">${card.prompt ? esc(card.prompt) : '<span style="color:var(--overlay0)">—</span>'}</span></td>
      <td class="col-tags">${tags}</td>
      <td class="col-state">${stateBadge}</td>
      <td class="col-due"><span class="${dueClass}">${due}</span></td>
      <td class="col-actions">
        <div class="row-actions">
          <button class="btn-icon" title="Edit" onclick="openEdit(${card.id})">✎</button>
          <button class="btn-icon" title="Delete" style="color:var(--red)" onclick="confirmDelete(${card.id})">✕</button>
        </div>
      </td>
    </tr>`;
  }).join('');
}

// ── Formatting helpers ───────────────────────────────────────────────────────
function formatDue(dueAt, now) {
  const diff = dueAt - now;
  if (diff <= 0) return 'now';
  const s = diff;
  if (s < 60)   return s + 's';
  if (s < 3600) return Math.floor(s / 60) + 'm';
  if (s < 86400) return Math.floor(s / 3600) + 'h';
  if (s < 604800) return Math.floor(s / 86400) + 'd';
  return Math.floor(s / 604800) + 'w';
}

function esc(str) {
  if (!str) return '';
  return str.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}

// ── Static event bindings ────────────────────────────────────────────────────
function bindStaticEvents() {
  // Stats bar filter
  document.querySelectorAll('.stat').forEach(el => {
    el.addEventListener('click', () => {
      filters.state = el.dataset.stateFilter;
      document.querySelectorAll('.stat').forEach(s => s.classList.remove('active'));
      el.classList.add('active');
      renderTable();
    });
  });

  // Search
  document.getElementById('search').addEventListener('input', e => {
    filters.search = e.target.value;
    renderTable();
  });

  // Tag filter
  document.getElementById('filter-tag').addEventListener('change', e => {
    filters.tag = e.target.value;
    renderTable();
  });

  // Sort columns
  document.querySelectorAll('thead th.sortable').forEach(th => {
    th.addEventListener('click', () => {
      const col = th.dataset.col;
      if (sort.col === col) sort.asc = !sort.asc;
      else { sort.col = col; sort.asc = true; }
      renderTable();
    });
  });

  // Clear filter button (in empty state)
  document.getElementById('btn-clear-filter').addEventListener('click', () => {
    filters.search = ''; filters.state = ''; filters.tag = '';
    document.getElementById('search').value = '';
    document.getElementById('filter-tag').value = '';
    document.querySelectorAll('.stat').forEach(s => s.classList.remove('active'));
    document.querySelector('.stat-all').classList.add('active');
    renderTable();
  });

  // Add card button
  document.getElementById('btn-add').addEventListener('click', openAdd);

  // Modal close / cancel
  document.getElementById('modal-close').addEventListener('click',  closeCardModal);
  document.getElementById('modal-cancel').addEventListener('click', closeCardModal);
  document.getElementById('modal-save').addEventListener('click',   saveCard);

  // Confirm modal
  document.getElementById('confirm-close').addEventListener('click',  closeConfirm);
  document.getElementById('confirm-cancel').addEventListener('click', closeConfirm);

  // Close modal on overlay click
  document.getElementById('card-modal').addEventListener('click', e => {
    if (e.target === e.currentTarget) closeCardModal();
  });
  document.getElementById('confirm-modal').addEventListener('click', e => {
    if (e.target === e.currentTarget) closeConfirm();
  });

  // Import / Export
  document.getElementById('btn-import').addEventListener('click', () => {
    document.getElementById('import-input').click();
  });
  document.getElementById('import-input').addEventListener('change', handleImport);
  document.getElementById('btn-export').addEventListener('click', handleExport);

  // Keyboard: Escape closes modals
  document.addEventListener('keydown', e => {
    if (e.key === 'Escape') { closeCardModal(); closeConfirm(); }
  });
}

// ── Add / Edit modal ─────────────────────────────────────────────────────────
function openAdd() {
  document.getElementById('modal-title').textContent = 'Add Card';
  document.getElementById('edit-id').value    = '';
  document.getElementById('field-title').value  = '';
  document.getElementById('field-prompt').value = '';
  document.getElementById('field-body').value   = '';
  document.getElementById('field-tags').value   = '';
  document.getElementById('card-modal').classList.add('open');
  document.getElementById('field-title').focus();
}

function openEdit(id) {
  const card = allCards.find(c => c.id === id);
  if (!card) return;
  document.getElementById('modal-title').textContent = 'Edit Card';
  document.getElementById('edit-id').value    = id;
  document.getElementById('field-title').value  = card.title;
  document.getElementById('field-prompt').value = card.prompt || '';
  document.getElementById('field-body').value   = card.body;
  document.getElementById('field-tags').value   = card.tags || '';
  document.getElementById('card-modal').classList.add('open');
  document.getElementById('field-title').focus();
}

function closeCardModal() {
  document.getElementById('card-modal').classList.remove('open');
}

async function saveCard() {
  const id    = document.getElementById('edit-id').value;
  const title = document.getElementById('field-title').value.trim();
  const prompt = document.getElementById('field-prompt').value.trim() || null;
  const body  = document.getElementById('field-body').value.trim();
  const tags  = document.getElementById('field-tags').value.trim();

  if (!title) { document.getElementById('field-title').focus(); return; }
  if (!body)  { document.getElementById('field-body').focus(); return; }

  try {
    if (id) {
      await invoke('update_card', { id: parseInt(id), title, prompt, body, tags });
      toast('Card updated.');
    } else {
      await invoke('add_card', { title, prompt, body, tags });
      toast('Card added.');
    }
    closeCardModal();
    await refresh();
  } catch (e) {
    toast('Error: ' + e);
  }
}

// ── Delete confirmation ──────────────────────────────────────────────────────
let pendingDeleteId = null;

function confirmDelete(id) {
  const card = allCards.find(c => c.id === id);
  if (!card) return;
  pendingDeleteId = id;
  document.getElementById('confirm-body').innerHTML =
    `<p>Delete <strong>${esc(card.title)}</strong>?</p>
     <p style="color:var(--overlay0);margin-top:8px">This cannot be undone.</p>`;
  document.getElementById('confirm-ok').onclick = doDelete;
  document.getElementById('confirm-modal').classList.add('open');
}

function closeConfirm() {
  document.getElementById('confirm-modal').classList.remove('open');
  pendingDeleteId = null;
}

async function doDelete() {
  if (!pendingDeleteId) return;
  try {
    await invoke('delete_card', { id: pendingDeleteId });
    toast('Card deleted.');
    closeConfirm();
    await refresh();
  } catch (e) {
    toast('Error: ' + e);
  }
}

// ── Import / Export ──────────────────────────────────────────────────────────
async function handleImport(e) {
  const file = e.target.files[0];
  if (!file) return;
  e.target.value = '';

  let records;
  try {
    const text = await file.text();
    records = JSON.parse(text);
  } catch (_) {
    toast('Invalid JSON file.');
    return;
  }

  if (!Array.isArray(records)) { toast('Expected a JSON array.'); return; }

  let added = 0, errors = 0;
  for (const r of records) {
    if (!r.title || !r.body) { errors++; continue; }
    try {
      await invoke('add_card', {
        title:  r.title,
        prompt: r.prompt || null,
        body:   r.body,
        tags:   r.tags || '',
      });
      added++;
    } catch (_) { errors++; }
  }

  toast(`Imported ${added} card${added !== 1 ? 's' : ''}${errors ? ` (${errors} skipped)` : ''}.`);
  await refresh();
}

function handleExport() {
  const records = allCards.map(c => {
    const r = { title: c.title, body: c.body, tags: c.tags || '' };
    if (c.prompt) r.prompt = c.prompt;
    return r;
  });

  const json = JSON.stringify(records, null, 2);
  const blob = new Blob([json], { type: 'application/json' });
  const url  = URL.createObjectURL(blob);
  const a    = document.createElement('a');
  a.href     = url;
  a.download = `teacha-cards-${new Date().toISOString().slice(0,10)}.json`;
  a.click();
  URL.revokeObjectURL(url);
  toast(`Exported ${records.length} cards.`);
}

// ── Toast ─────────────────────────────────────────────────────────────────────
let toastTimer;
function toast(msg) {
  const el = document.getElementById('toast');
  el.textContent = msg;
  el.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove('show'), 2500);
}
