let schema = [];
let selFamily = null, selOp = null, selMeta = null;
let mode = 'single';            // 'single' | 'workflow'
let wfSteps = [];               // [{family,op,options,inputs:[{kind,..}]}]
let currentRead = null;
let lastResultId = null;

// File pool: {id, name, virtual, stepIndex, selected}. Virtual entries are
// placeholder outputs of earlier workflow steps (cannot be previewed/deleted).
let pool = [];
let dragFromIdx = null;          // drag-to-reorder source index

const $ = id => document.getElementById(id);

async function init() {
  schema = await fetch('/api/schema').then(r => r.json());
  renderFamilies();
  $('wf-toggle').onclick = toggleWorkflowMode;
  $('btn-add-step').onclick = openPicker;
  $('picker-close').onclick = () => $('op-picker').classList.remove('show');
  $('btn-single').onclick = execSingle;
  $('btn-exec-wf').onclick = execWorkflow;
  $('btn-add-to-wf').onclick = addStepFromForm;
  $('btn-upload').onclick = () => $('file-input').click();
  $('file-input').onchange = onUpload;
  $('file-dropdown-btn').onclick = toggleDropdown;
  document.addEventListener('click', e => {
    if (!$('file-select').contains(e.target)) $('file-list').classList.remove('show');
  });
}

function renderFamilies() {
  const el = $('families');
  el.innerHTML = '';
  for (const f of schema) {
    const d = document.createElement('div');
    d.className = 'item' + (selFamily === f.name && mode === 'single' ? ' active' : '');
    d.textContent = f.name;
    d.onclick = () => { if (mode === 'workflow') return; selectFamily(f.name); };
    el.appendChild(d);
  }
}

function selectFamily(name) {
  selFamily = name; selOp = null;
  renderFamilies();
  renderOps();
}

function renderOps() {
  const el = $('ops');
  el.innerHTML = '';
  const fam = schema.find(f => f.name === selFamily);
  if (!fam) return;
  for (const op of fam.ops) {
    const d = document.createElement('div');
    d.className = 'item' + (selOp === op.name ? ' active' : '');
    d.innerHTML = `<code>${op.name}</code>`;
    if (op.multi_input) d.title = 'Requires multiple inputs';
    d.onclick = () => showOpForm(fam.name, op, false);
    el.appendChild(d);
  }
}

function showOpForm(family, op, wfMode) {
  selFamily = family; selOp = op.name; selMeta = op;
  if (mode === 'single') renderOps();
  $('op-panel').style.display = '';
  $('op-title').textContent = `${family} › ${op.name}`;
  currentRead = renderForm(op.schema || {}, $('form-fields'));
  $('btn-single').style.display = wfMode ? 'none' : '';
  $('btn-add-to-wf').style.display = wfMode ? '' : 'none';
  clearError();
}

function toggleWorkflowMode() {
  mode = mode === 'workflow' ? 'single' : 'workflow';
  $('c2-single').style.display = mode === 'workflow' ? 'none' : '';
  $('c2-workflow').style.display = mode === 'workflow' ? '' : 'none';
  $('wf-toggle').classList.toggle('active', mode === 'workflow');
  $('op-panel').style.display = 'none';
  selOp = null;
  // Drop virtual entries when leaving/entering workflow mode.
  pool = pool.filter(f => !f.virtual);
  wfSteps = [];
  renderFamilies();
  renderChain();
  renderFileList();
}

window.init = init;

// ---- File pool / dropdown ----
function toggleDropdown() { $('file-list').classList.toggle('show'); }

function renderFileList() {
  const el = $('file-list');
  el.innerHTML = '';
  if (!pool.length) { el.innerHTML = '<div class="empty">No files. Upload to begin.</div>'; return; }
  pool.forEach((f, idx) => {
    const row = document.createElement('div');
    row.className = 'file-item';
    row.draggable = true;
    row.dataset.idx = idx;

    const grip = document.createElement('span');
    grip.className = 'grip';
    grip.textContent = '⠿';
    grip.title = 'Drag to reorder';

    const cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.checked = f.selected;
    cb.onchange = () => { f.selected = cb.checked; onSelectionChange(); };
    const name = document.createElement('span');
    name.className = 'fname' + (f.virtual ? ' virtual' : '');
    name.textContent = f.name;
    name.title = f.id;
    const del = document.createElement('button');
    del.className = 'fdel';
    del.textContent = '✕';
    del.disabled = f.virtual;
    del.onclick = () => deleteFile(f);

    row.addEventListener('dragstart', e => {
      dragFromIdx = idx;
      row.classList.add('dragging');
      e.dataTransfer.effectAllowed = 'move';
    });
    row.addEventListener('dragend', () => row.classList.remove('dragging'));
    row.addEventListener('dragover', e => { e.preventDefault(); row.classList.add('drag-over'); });
    row.addEventListener('dragleave', () => row.classList.remove('drag-over'));
    row.addEventListener('drop', e => {
      e.preventDefault();
      row.classList.remove('drag-over');
      reorderPool(dragFromIdx, idx);
    });

    row.append(grip, cb, name, del);
    el.appendChild(row);
  });
}

function reorderPool(from, to) {
  if (from == null || from === to) return;
  const [moved] = pool.splice(from, 1);
  pool.splice(to, 0, moved);
  renderFileList();
}

function onSelectionChange() {
  // Preview the most recently relevant selected real file.
  const sel = pool.filter(f => f.selected && !f.virtual);
  if (sel.length) previewFile(sel[sel.length - 1].id);
}

async function deleteFile(f) {
  try {
    const r = await fetch('/api/file/' + f.id, { method: 'DELETE' });
    if (!r.ok && r.status !== 404) return showError(await r.text());
  } catch (e) { return showError(e.message); }
  pool = pool.filter(x => x !== f);
  renderFileList();
}

function selectedFileIds() { return pool.filter(f => f.selected && !f.virtual).map(f => f.id); }
function selectedInputs() {
  return pool.filter(f => f.selected).map(f =>
    f.virtual ? { kind: 'step', index: f.stepIndex } : { kind: 'file', id: f.id });
}

async function previewFile(id) {
  const url = '/api/file/' + id;
  const head = await fetch(url, { method: 'HEAD' });
  const ct = head.headers.get('content-type') || '';
  const area = $('preview-area');
  if (ct.startsWith('application/pdf')) area.innerHTML = `<embed src="${url}" type="application/pdf">`;
  else if (ct.startsWith('image/')) area.innerHTML = `<img src="${url}" alt="file">`;
  else { const t = await fetch(url).then(r => r.text()); area.innerHTML = '<pre></pre>'; area.querySelector('pre').textContent = t; }
}

async function onUpload() {
  if (!this.files.length) return;
  const fd = new FormData();
  for (const f of this.files) fd.append('file', f);
  try {
    const r = await fetch('/api/upload', { method: 'POST', body: fd });
    if (!r.ok) return showError(await r.text());
    const data = await r.json();
    for (const f of data.files) pool.push({ id: f.id, name: f.filename, virtual: false, selected: true });
    this.value = '';
    renderFileList();
    onSelectionChange();
  } catch (e) { showError(e.message); }
}

// ---- Workflow ----
function openPicker() {
  const body = $('picker-body');
  body.innerHTML = '';
  for (const f of schema) {
    const fl = document.createElement('div');
    fl.className = 'fam';
    fl.textContent = f.name;
    body.appendChild(fl);
    const grid = document.createElement('div');
    grid.className = 'opgrid';
    for (const op of f.ops) {
      const d = document.createElement('div');
      d.textContent = op.name;
      if (op.multi_input) d.title = 'Requires multiple inputs';
      d.onclick = () => { $('op-picker').classList.remove('show'); showOpForm(f.name, op, true); };
      grid.appendChild(d);
    }
    body.appendChild(grid);
  }
  $('op-picker').classList.add('show');
}

function addStepFromForm() {
  if (!selOp) return;
  const inputs = selectedInputs();
  if (!inputs.length) return showError('Select input file(s) for this step first');
  const stepIndex = wfSteps.length;
  wfSteps.push({ family: selFamily, op: selOp, options: currentRead ? currentRead() : {}, inputs });

  // Consume current selection; add a virtual output for this step and select it.
  pool.forEach(f => f.selected = false);
  pool.push({ id: 'step:' + stepIndex, name: `↳ output of step ${stepIndex + 1}`, virtual: true, stepIndex, selected: true });

  $('op-panel').style.display = 'none';
  selOp = null;
  renderChain();
  renderFileList();
  clearError();
}

function renderChain() {
  const el = $('chain');
  el.innerHTML = '<div class="node io">📄 input</div>';
  wfSteps.forEach((s, i) => {
    el.insertAdjacentHTML('beforeend', '<div class="arrow">↓</div>');
    const n = document.createElement('div');
    n.className = 'node';
    n.innerHTML = `<span>${s.family} › ${s.op}</span>`;
    const x = document.createElement('button');
    x.className = 'btn-sm';
    x.textContent = '✕';
    x.onclick = () => removeStep(i);
    n.appendChild(x);
    el.appendChild(n);
  });
  el.insertAdjacentHTML('beforeend', '<div class="arrow">↓</div><div class="node io">📦 output</div>');
}

function removeStep(i) {
  // Remove the step and any steps after it (their inputs may depend on it),
  // plus their virtual outputs. Keeps references consistent.
  wfSteps = wfSteps.slice(0, i);
  pool = pool.filter(f => !(f.virtual && f.stepIndex >= i));
  renderChain();
  renderFileList();
}

// ---- Execute ----
async function execSingle() {
  clearError();
  const ids = selectedFileIds();
  if (!ids.length) return showError('Select input file(s)');
  if (!selOp) return showError('Select an operation');
  const btn = $('btn-single');
  btn.disabled = true; btn.textContent = 'Running…';
  try {
    const r = await fetch('/api/execute/single', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ artifact_ids: ids, family: selFamily, op: selOp,
        options_json: JSON.stringify(currentRead ? currentRead() : {}) }),
    });
    if (!r.ok) return showError(await r.text());
    showResult((await r.json()).result_id);
  } catch (e) { showError(e.message); }
  finally { btn.disabled = false; btn.textContent = 'Execute'; }
}

async function execWorkflow() {
  clearError();
  if (!wfSteps.length) return showError('Add at least one step');
  const btn = $('btn-exec-wf');
  btn.disabled = true; btn.textContent = 'Running…';
  try {
    const tasks = wfSteps.map(s => ({ family: s.family, op: s.op,
      options_json: JSON.stringify(s.options || {}), inputs: s.inputs }));
    const r = await fetch('/api/execute/workflow', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ tasks }),
    });
    if (!r.ok) return showError(await r.text());
    showResult((await r.json()).result_id);
  } catch (e) { showError(e.message); }
  finally { btn.disabled = false; btn.textContent = 'Execute Workflow'; }
}

async function showResult(id) {
  lastResultId = id;
  const dl = $('dl-btn');
  dl.href = '/api/file/' + id;
  dl.style.visibility = 'visible';
  const head = await fetch('/api/file/' + id, { method: 'HEAD' });
  const ct = head.headers.get('content-type') || '';
  dl.download = ct.startsWith('application/pdf') ? 'result.pdf'
    : ct.startsWith('image/') ? 'result.' + (ct.split('/')[1] || 'png') : 'result.txt';
  await previewFile(id);
}

function showError(msg) { $('error-msg').textContent = msg; }
function clearError() { $('error-msg').textContent = ''; }

init();
