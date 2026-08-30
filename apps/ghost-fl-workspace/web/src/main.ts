const $ = (selector) => document.querySelector(selector);

const elements = {
  messages: $('#messages'),
  prompt: $('#prompt'),
  send: $('#send'),
  turnStatus: $('#turn-status'),
  activeThreadName: $('#active-thread-name'),
  threadButton: $('#thread-button'),
  threadCount: $('#thread-count'),
  projectTitle: $('#project-title'),
  projectMeta: $('#project-meta'),
  projectEdit: $('#project-edit'),
  assetList: $('#asset-list'),
  addAsset: $('#add-asset'),
  composerAddAsset: $('#composer-add-asset'),
  composerAssets: $('#composer-assets'),
  modelSelect: $('#model-select'),
  effortSelect: $('#effort-select'),
  openPlan: $('#open-plan'),
  openSkills: $('#open-skills'),
  openThreads: $('#open-threads'),
  inspectorFl: $('#inspector-fl'),
  inspectorAnalysis: $('#inspector-analysis'),
  inspectorPlan: $('#inspector-plan'),
  threadDialog: $('#thread-dialog'),
  threadFilter: $('#thread-filter'),
  threadList: $('#thread-list'),
  newThread: $('#new-thread'),
  forkThread: $('#fork-thread'),
  renameThread: $('#rename-thread'),
  assetDialog: $('#asset-dialog'),
  assetForm: $('#asset-form'),
  assetPath: $('#asset-path'),
  assetLabel: $('#asset-label'),
  assetRole: $('#asset-role'),
  projectDialog: $('#project-dialog'),
  projectForm: $('#project-form'),
  projectTitleInput: $('#project-title-input'),
  projectTempo: $('#project-tempo'),
  projectTimeSignature: $('#project-time-signature'),
  projectDescription: $('#project-description'),
  skillsDialog: $('#skills-dialog'),
  skillList: $('#skill-list'),
  skillContent: $('#skill-content'),
  closeSkills: $('#close-skills')
};

const state = {
  busy: false,
  info: null,
  threads: { selectedThreadId: null, threads: [] },
  project: null,
  snapshot: null,
  skills: [],
  selectedAssetId: null,
  selectedThreadRowId: null,
  analysisById: new Map(),
  activeInspector: 'fl'
};

async function request(path, init = undefined) {
  const response = await fetch(path, init);
  const text = await response.text();
  let payload = {};
  try { payload = text ? JSON.parse(text) : {}; } catch { payload = { error: text || `HTTP ${response.status}` }; }
  if (!response.ok) throw new Error(payload.error || `HTTP ${response.status}`);
  return payload;
}

function jsonRequest(path, method, body) {
  return request(path, {
    method,
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body ?? {})
  });
}

const post = (path, body = {}) => jsonRequest(path, 'POST', body);
const put = (path, body = {}) => jsonRequest(path, 'PUT', body);

function setBusy(value, label = '') {
  state.busy = value;
  elements.send.disabled = value;
  elements.prompt.disabled = value;
  elements.modelSelect.disabled = value;
  elements.effortSelect.disabled = value;
  elements.turnStatus.textContent = value ? (label || 'Ghost is working…') : '';
  elements.turnStatus.classList.toggle('working', value);
  elements.send.textContent = value ? 'Working…' : 'Send';
}

function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}

function inlineMarkdown(text) {
  let value = text;
  value = value.replace(/`([^`]+)`/g, '<code>$1</code>');
  value = value.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
  value = value.replace(/__([^_]+)__/g, '<strong>$1</strong>');
  value = value.replace(/(?<!\*)\*([^*]+)\*(?!\*)/g, '<em>$1</em>');
  value = value.replace(/\[([^\]]+)\]\((https?:\/\/[^)]+)\)/g, '<a href="$2" target="_blank" rel="noreferrer">$1</a>');
  return value;
}

function renderMarkdown(source) {
  const codeBlocks = [];
  let text = String(source ?? '').replace(/```([^\n]*)\n([\s\S]*?)```/g, (_, language, code) => {
    const index = codeBlocks.length;
    codeBlocks.push(`<pre><code data-language="${escapeHtml(language.trim())}">${escapeHtml(code.replace(/\n$/, ''))}</code></pre>`);
    return `\n@@CODEBLOCK_${index}@@\n`;
  });
  text = escapeHtml(text).replaceAll('\r\n', '\n');
  const lines = text.split('\n');
  const output = [];
  let paragraph = [];
  let listType = null;
  let listItems = [];

  const flushParagraph = () => {
    if (!paragraph.length) return;
    output.push(`<p>${inlineMarkdown(paragraph.join(' '))}</p>`);
    paragraph = [];
  };
  const flushList = () => {
    if (!listType || !listItems.length) return;
    output.push(`<${listType}>${listItems.map((item) => `<li>${inlineMarkdown(item)}</li>`).join('')}</${listType}>`);
    listType = null;
    listItems = [];
  };

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const trimmed = line.trim();
    if (!trimmed) { flushParagraph(); flushList(); continue; }
    const codeMatch = trimmed.match(/^@@CODEBLOCK_(\d+)@@$/);
    if (codeMatch) { flushParagraph(); flushList(); output.push(codeBlocks[Number(codeMatch[1])]); continue; }

    if (trimmed.includes('|') && index + 1 < lines.length && /^\s*\|?\s*:?-{3,}/.test(lines[index + 1])) {
      flushParagraph(); flushList();
      const headers = trimmed.replace(/^\||\|$/g, '').split('|').map((cell) => inlineMarkdown(cell.trim()));
      index += 1;
      const rows = [];
      while (index + 1 < lines.length && lines[index + 1].includes('|') && lines[index + 1].trim()) {
        index += 1;
        rows.push(lines[index].trim().replace(/^\||\|$/g, '').split('|').map((cell) => inlineMarkdown(cell.trim())));
      }
      output.push(`<table><thead><tr>${headers.map((cell) => `<th>${cell}</th>`).join('')}</tr></thead><tbody>${rows.map((row) => `<tr>${headers.map((_, column) => `<td>${row[column] ?? ''}</td>`).join('')}</tr>`).join('')}</tbody></table>`);
      continue;
    }

    const heading = trimmed.match(/^(#{1,4})\s+(.+)$/);
    if (heading) { flushParagraph(); flushList(); const level = heading[1].length; output.push(`<h${level}>${inlineMarkdown(heading[2])}</h${level}>`); continue; }
    const quote = trimmed.match(/^&gt;\s?(.+)$/);
    if (quote) { flushParagraph(); flushList(); output.push(`<blockquote>${inlineMarkdown(quote[1])}</blockquote>`); continue; }
    const unordered = trimmed.match(/^[-*+]\s+(.+)$/);
    if (unordered) {
      flushParagraph();
      if (listType && listType !== 'ul') flushList();
      listType = 'ul'; listItems.push(unordered[1]); continue;
    }
    const ordered = trimmed.match(/^\d+[.)]\s+(.+)$/);
    if (ordered) {
      flushParagraph();
      if (listType && listType !== 'ol') flushList();
      listType = 'ol'; listItems.push(ordered[1]); continue;
    }
    flushList();
    paragraph.push(trimmed);
  }
  flushParagraph(); flushList();
  return output.join('');
}

function traceText(trace) {
  return trace.map((event) => {
    if (event.kind === 'tool_started') return `→ ${event.tool} ${JSON.stringify(event.arguments ?? {})}`;
    return `← ${event.tool} success=${event.success ?? false} duration=${event.durationMs ?? 0}ms`;
  }).join('\n');
}

function addMessage(role, text, trace = [], scroll = true, isError = false) {
  const article = document.createElement('article');
  article.className = `message ${role}`;
  const roleNode = document.createElement('div');
  roleNode.className = 'message-role';
  roleNode.textContent = role === 'user' ? 'You' : 'Ghost';
  const body = document.createElement('div');
  body.className = `message-body markdown-body${isError ? ' message-error' : ''}`;
  body.innerHTML = renderMarkdown(text);
  article.append(roleNode, body);
  if (trace?.length) {
    const details = document.createElement('details');
    details.className = 'tool-trace';
    const summary = document.createElement('summary');
    summary.textContent = `${trace.filter((event) => event.kind === 'tool_started').length} tool calls`;
    const pre = document.createElement('pre');
    pre.textContent = traceText(trace);
    details.append(summary, pre);
    article.append(details);
  }
  elements.messages.append(article);
  if (scroll) article.scrollIntoView({ behavior: 'smooth', block: 'end' });
}

function renderHistory(history) {
  elements.messages.innerHTML = '';
  for (const message of history.messages || []) addMessage(message.role, message.text, message.trace || [], false);
  if (elements.messages.lastElementChild) elements.messages.lastElementChild.scrollIntoView({ block: 'end' });
}

function shortId(id) {
  if (!id) return '';
  return id.length > 20 ? `${id.slice(0, 8)}…${id.slice(-6)}` : id;
}

function selectedThread() {
  return state.threads.threads.find((thread) => thread.id === state.threads.selectedThreadId) || null;
}

function renderThreadSummary() {
  const selected = selectedThread();
  elements.threadCount.textContent = String(state.threads.threads.length);
  elements.activeThreadName.textContent = selected?.name || (selected ? 'Untitled thread' : 'New workspace');
  if (!state.project) {
    elements.projectMeta.textContent = selected ? shortId(selected.id) : 'No thread selected';
  }
}

function renderThreadList() {
  const query = elements.threadFilter.value.trim().toLowerCase();
  elements.threadList.innerHTML = '';
  const visible = state.threads.threads.filter((thread) => `${thread.name || ''} ${thread.id}`.toLowerCase().includes(query));
  if (!visible.length) {
    const empty = document.createElement('div');
    empty.className = 'asset-empty';
    empty.textContent = state.threads.threads.length ? 'No matching threads.' : 'No workspace threads yet.';
    elements.threadList.append(empty);
  }
  for (const thread of visible) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = `thread-row${thread.id === state.threads.selectedThreadId ? ' selected' : ''}`;
    const copy = document.createElement('span');
    copy.className = 'thread-row-copy';
    copy.innerHTML = `<strong>${escapeHtml(thread.name || 'Untitled thread')}</strong><span>${escapeHtml(thread.id)}</span>`;
    const status = document.createElement('span');
    status.className = 'thread-row-state';
    status.textContent = thread.hasTurns ? 'active' : 'empty';
    button.append(copy, status);
    button.addEventListener('click', () => chooseThread(thread.id));
    elements.threadList.append(button);
  }
  const canOperate = Boolean(state.threads.selectedThreadId) && !state.busy;
  elements.forkThread.disabled = !canOperate;
  elements.renameThread.disabled = !canOperate;
}

async function refreshThreads() {
  state.threads = await request('/api/threads');
  state.selectedThreadRowId = state.threads.selectedThreadId;
  renderThreadSummary();
  renderThreadList();
}

async function loadHistory() {
  if (!state.threads.selectedThreadId) { renderHistory({ messages: [] }); return; }
  try { renderHistory(await request('/api/history')); }
  catch (error) { renderHistory({ messages: [] }); addMessage('assistant', `Thread history unavailable: ${error.message}`, [], true, true); }
}

async function chooseThread(threadId) {
  if (state.busy || !threadId || threadId === state.threads.selectedThreadId) {
    elements.threadDialog.close();
    return;
  }
  setBusy(true, 'Switching thread…');
  try {
    state.threads = await post('/api/threads/select', { threadId });
    state.selectedAssetId = null;
    await Promise.all([loadHistory(), loadProject()]);
    renderThreadSummary();
    renderThreadList();
    elements.threadDialog.close();
  } catch (error) {
    addMessage('assistant', `Thread selection failed: ${error.message}`, [], true, true);
  } finally {
    setBusy(false);
    refreshInfo();
  }
}

async function newThread() {
  if (state.busy) return;
  setBusy(true, 'Creating thread…');
  try {
    state.threads = await post('/api/threads/new');
    state.selectedAssetId = null;
    renderHistory({ messages: [] });
    await loadProject();
    renderThreadSummary();
    renderThreadList();
    elements.threadDialog.close();
  } catch (error) {
    addMessage('assistant', `Thread creation failed: ${error.message}`, [], true, true);
  } finally { setBusy(false); refreshInfo(); }
}

async function forkThread() {
  const threadId = state.threads.selectedThreadId;
  if (state.busy || !threadId) return;
  setBusy(true, 'Forking thread…');
  try {
    state.threads = await post('/api/threads/fork', { threadId });
    state.selectedAssetId = null;
    await Promise.all([loadHistory(), loadProject()]);
    renderThreadSummary(); renderThreadList(); elements.threadDialog.close();
  } catch (error) {
    addMessage('assistant', `Thread fork failed: ${error.message}`, [], true, true);
  } finally { setBusy(false); refreshInfo(); }
}

async function renameThread() {
  const selected = selectedThread();
  if (!selected || state.busy) return;
  const name = window.prompt('Thread name', selected.name || '');
  if (name === null || !name.trim()) return;
  try {
    state.threads = await post('/api/threads/rename', { threadId: selected.id, name: name.trim() });
    renderThreadSummary(); renderThreadList();
  } catch (error) { addMessage('assistant', `Thread rename failed: ${error.message}`, [], true, true); }
}

async function refreshInfo() {
  try {
    const info = await request('/api/info');
    state.info = info;
    syncModelControl(info.model);
    if (!state.busy && info.reasoningEffort) elements.effortSelect.value = info.reasoningEffort;
    renderFlInspector();
  } catch (error) {
    state.info = { error: error.message, scripting: { connected: false } };
    renderFlInspector();
  }
}

function syncModelControl(model) {
  if (!model) return;
  const existing = [...elements.modelSelect.options].find((option) => option.value === model);
  if (!existing) {
    const option = document.createElement('option');
    option.value = model;
    option.textContent = model;
    elements.modelSelect.insertBefore(option, elements.modelSelect.firstChild);
  }
  if (!elements.modelSelect.querySelector('option[value="__custom__"]')) {
    const custom = document.createElement('option');
    custom.value = '__custom__';
    custom.textContent = 'Custom model…';
    elements.modelSelect.append(custom);
  }
  if (!state.busy) elements.modelSelect.value = model;
}

function openCustomModel() {
  if (elements.modelSelect.value !== '__custom__') return;
  const current = state.info?.model || '';
  const model = window.prompt('Model identifier', current);
  if (!model?.trim()) { elements.modelSelect.value = current; return; }
  syncModelControl(model.trim());
  elements.modelSelect.value = model.trim();
}

async function loadProject() {
  try {
    const response = await request('/api/project');
    state.project = response.project || null;
    if (state.project && !state.selectedAssetId && state.project.assets?.length) {
      state.selectedAssetId = state.project.assets[0].id;
    }
    if (state.selectedAssetId && !state.project?.assets?.some((asset) => asset.id === state.selectedAssetId)) state.selectedAssetId = null;
    renderProject();
  } catch (error) {
    state.project = null;
    renderProject();
  }
}

function renderProject() {
  const project = state.project;
  elements.projectTitle.textContent = project?.title || 'Untitled production';
  elements.projectMeta.textContent = project ? [project.tempoBpm ? `${formatNumber(project.tempoBpm, 1)} BPM` : null, project.timeSignature || null].filter(Boolean).join(' · ') || shortId(project.threadId) : (state.threads.selectedThreadId ? 'Project unavailable' : 'Select or create a thread');
  renderAssets();
  renderPlanInspector();
  renderAnalysisInspector();
}

function renderAssets() {
  elements.assetList.innerHTML = '';
  elements.composerAssets.innerHTML = '';
  const assets = state.project?.assets || [];
  if (!assets.length) {
    const empty = document.createElement('div');
    empty.className = 'asset-empty';
    empty.textContent = state.project ? 'Add the reference mix and separated stems.' : 'Create or select a thread first.';
    elements.assetList.append(empty);
    return;
  }
  for (const asset of assets) {
    const card = document.createElement('button');
    card.type = 'button';
    card.className = `asset-card${asset.id === state.selectedAssetId ? ' selected' : ''}`;
    card.innerHTML = `<span class="asset-icon">${escapeHtml(roleGlyph(asset.role))}</span><span class="asset-copy"><strong>${escapeHtml(asset.label)}</strong><span>${escapeHtml(asset.role.replaceAll('_', ' '))}</span></span><span class="analysis-dot${asset.analysisId ? ' ready' : ''}"></span>`;
    card.title = asset.path;
    card.addEventListener('click', () => selectAsset(asset.id));
    elements.assetList.append(card);

    const chip = document.createElement('button');
    chip.type = 'button';
    chip.className = 'composer-asset-chip';
    chip.textContent = asset.label;
    chip.title = asset.path;
    chip.addEventListener('click', () => selectAsset(asset.id));
    elements.composerAssets.append(chip);
  }
}

function roleGlyph(role) {
  const map = { reference_mix: 'REF', drums: 'DR', kick: 'K', snare: 'S', hihat: 'HH', percussion: 'PC', bass: 'B', music: 'M', vocals: 'V', fx: 'FX' };
  return map[role] || 'A';
}

async function selectAsset(assetId) {
  state.selectedAssetId = assetId;
  renderAssets();
  activateInspector('analysis');
  await renderAnalysisInspector();
}

function openAssetDialog() {
  if (!state.project) {
    elements.threadDialog.showModal();
    return;
  }
  elements.assetPath.value = '';
  elements.assetLabel.value = '';
  elements.assetRole.value = 'reference_mix';
  elements.assetDialog.showModal();
  setTimeout(() => elements.assetPath.focus(), 0);
}

async function saveAsset(event) {
  event.preventDefault();
  const path = elements.assetPath.value.trim();
  if (!path) return;
  try {
    await post('/api/project/assets', {
      path,
      label: elements.assetLabel.value.trim() || undefined,
      role: elements.assetRole.value
    });
    elements.assetDialog.close();
    await loadProject();
    const match = state.project?.assets?.find((asset) => asset.path.toLowerCase() === path.toLowerCase()) || state.project?.assets?.at(-1);
    if (match) { state.selectedAssetId = match.id; renderAssets(); activateInspector('analysis'); }
  } catch (error) { window.alert(`Could not add audio path: ${error.message}`); }
}

function openProjectDialog() {
  if (!state.project) { elements.threadDialog.showModal(); return; }
  elements.projectTitleInput.value = state.project.title || '';
  elements.projectTempo.value = state.project.tempoBpm ?? '';
  elements.projectTimeSignature.value = state.project.timeSignature || '4/4';
  elements.projectDescription.value = state.project.description || '';
  elements.projectDialog.showModal();
}

async function saveProject(event) {
  event.preventDefault();
  const tempoText = String(elements.projectTempo.value).trim();
  try {
    state.project = await put('/api/project', {
      title: elements.projectTitleInput.value.trim(),
      description: elements.projectDescription.value.trim(),
      tempoBpm: tempoText ? Number(tempoText) : null,
      timeSignature: elements.projectTimeSignature.value.trim() || '4/4'
    });
    elements.projectDialog.close();
    renderProject();
  } catch (error) { window.alert(`Could not save project context: ${error.message}`); }
}

function selectedAsset() {
  return state.project?.assets?.find((asset) => asset.id === state.selectedAssetId) || null;
}

async function analyzeSelectedAsset(force = false) {
  const asset = selectedAsset();
  if (!asset || state.busy) return;
  setBusy(true, `Analyzing ${asset.label}…`);
  try {
    const result = await post('/api/analysis/run', {
      path: asset.path,
      label: asset.label,
      role: asset.role,
      tempoBpm: state.project?.tempoBpm ?? undefined,
      force
    });
    state.analysisById.set(result.analysisId, result);
    await loadProject();
    state.selectedAssetId = asset.id;
    renderAnalysisInspector();
  } catch (error) {
    window.alert(`Audio analysis failed: ${error.message}`);
  } finally { setBusy(false); }
}

async function analysisSummary(analysisId) {
  if (!analysisId) return null;
  if (state.analysisById.has(analysisId)) return state.analysisById.get(analysisId);
  try {
    const summary = await post('/api/analysis/read', { analysisId, view: 'summary' });
    state.analysisById.set(analysisId, summary);
    return summary;
  } catch { return null; }
}

async function renderAnalysisInspector() {
  const asset = selectedAsset();
  if (!asset) {
    elements.inspectorAnalysis.innerHTML = '<div class="inspector-empty">Select a reference or stem to inspect its analysis.</div>';
    return;
  }
  const summary = asset.analysisId ? await analysisSummary(asset.analysisId) : null;
  if (!summary) {
    elements.inspectorAnalysis.innerHTML = `<section class="inspector-section"><div class="inspector-section-title">${escapeHtml(asset.role.replaceAll('_', ' '))}</div><h3>${escapeHtml(asset.label)}</h3><div class="inspector-empty">${escapeHtml(asset.path)}</div><button id="analyze-asset" class="primary-button" type="button">Analyze file</button></section>`;
    $('#analyze-asset')?.addEventListener('click', () => analyzeSelectedAsset(false));
    return;
  }
  const values = summary.summary || {};
  const tempo = values.tempoCandidates?.[0];
  elements.inspectorAnalysis.innerHTML = `
    <section class="inspector-section">
      <div class="inspector-section-title">${escapeHtml(asset.role.replaceAll('_', ' '))}</div>
      <h3>${escapeHtml(asset.label)}</h3>
      <div class="subtle">${escapeHtml(asset.path)}</div>
    </section>
    <section class="inspector-section">
      <div class="metric-grid">
        ${metric('LUFS', values.integratedLufs, 1)}
        ${metric('RMS', values.rmsDbfs, 1, ' dBFS')}
        ${metric('Crest', values.crestFactorDb, 1, ' dB')}
        ${metric('Centroid', values.centroidHz, 0, ' Hz')}
        ${metric('Transients', values.transientDensityHz, 2, ' /s')}
        ${metric('Correlation', values.stereoCorrelation, 2)}
      </div>
    </section>
    <section class="inspector-section">
      <div class="inspector-section-title">Musical projection</div>
      <div class="kv-list">
        <div class="kv"><span>Tempo</span><span>${tempo ? `${formatNumber(tempo.bpm, 1)} BPM · ${formatNumber(tempo.confidence * 100, 0)}%` : state.project?.tempoBpm ? `${formatNumber(state.project.tempoBpm, 1)} BPM hint` : '—'}</span></div>
        <div class="kv"><span>Sections</span><span>${values.sectionCandidateCount ?? 0} candidates</span></div>
        <div class="kv"><span>Pitch events</span><span>${values.pitchEventCount ?? 0}</span></div>
      </div>
    </section>
    <section class="inspector-section"><button id="reanalyze-asset" type="button">Re-analyze</button></section>`;
  $('#reanalyze-asset')?.addEventListener('click', () => analyzeSelectedAsset(true));
}

function metric(label, value, digits = 1, suffix = '') {
  return `<div class="metric"><span>${escapeHtml(label)}</span><strong>${value === null || value === undefined ? '—' : `${formatNumber(value, digits)}${suffix}`}</strong></div>`;
}

function formatNumber(value, digits = 1) {
  const number = Number(value);
  return Number.isFinite(number) ? number.toFixed(digits) : '—';
}

async function refreshSnapshot() {
  try {
    state.snapshot = await request('/api/snapshot');
    renderFlInspector();
  } catch (error) {
    state.snapshot = { connected: false, values: {}, errors: { connection: error.message } };
    renderFlInspector();
  }
}

function renderFlInspector() {
  const snapshot = state.snapshot || { connected: false, values: {}, errors: {} };
  const values = snapshot.values || {};
  const info = state.info || {};
  const scripting = info.scripting || {};
  const playing = Number(values.isPlaying) !== 0;
  elements.inspectorFl.innerHTML = `
    <section class="inspector-section">
      <div class="inspector-section-title">Live session</div>
      <div class="kv-list">
        ${kv('Project', values.projectTitle || '—')}
        ${kv('Transport', playing ? 'Playing' : 'Stopped')}
        ${kv('Position', (values.songPositionHint || values.songPosition) ?? '—')}
        ${kv('Pattern', (values.currentPatternName || values.currentPattern) ?? '—')}
        ${kv('Channel', values.selectedChannel ?? '—')}
        ${kv('Mixer', values.selectedMixerTrack ?? '—')}
        ${kv('Safe to edit', Number(values.safeToEdit) ? 'Yes' : 'No')}
      </div>
    </section>
    <section class="inspector-section">
      <div class="inspector-section-title">Connections</div>
      <div class="status-line"><span class="status-dot ${info.error ? 'warn' : 'good'}"></span><span>${info.error ? `Gopher unavailable · ${escapeHtml(info.error)}` : `Gopher · ${info.gopherToolCount ?? 0} tools`}</span></div>
      <div class="status-line"><span class="status-dot ${scripting.connected ? 'good' : 'warn'}"></span><span>${scripting.connected ? `Scripting API ${scripting.hello?.scriptingApiVersion ?? values.scriptingApiVersion ?? '?'}` : 'Scripting waiting'}</span></div>
    </section>
    ${Object.keys(snapshot.errors || {}).length ? `<section class="inspector-section"><div class="inspector-section-title">Observation gaps</div><div class="inspector-empty">${Object.entries(snapshot.errors).map(([key, value]) => `${escapeHtml(key)}: ${escapeHtml(value)}`).join('<br>')}</div></section>` : ''}`;
}

function kv(label, value) {
  return `<div class="kv"><span>${escapeHtml(label)}</span><span>${escapeHtml(value)}</span></div>`;
}

function renderPlanInspector() {
  const plan = state.project?.productionPlan;
  if (!plan) {
    elements.inspectorPlan.innerHTML = '<div class="inspector-empty">The Production Plan will appear here once a thread has project context.</div>';
    return;
  }
  const groups = [
    ['Sections', plan.sections],
    ['Channels', plan.channels],
    ['Playlist', plan.playlistTracks],
    ['Mixer', plan.mixerInserts],
    ['Timbres', plan.timbres],
    ['Next steps', plan.nextSteps]
  ];
  const populated = groups.filter(([, items]) => Array.isArray(items) && items.length);
  elements.inspectorPlan.innerHTML = `
    <section class="inspector-section"><div class="inspector-section-title">Production plan</div><h3>${escapeHtml(plan.title || state.project?.title || 'Untitled')}</h3><div class="subtle">Semantic intent · live FL remains authoritative.</div></section>
    ${populated.length ? populated.map(([name, items]) => `<div class="plan-group"><h4>${escapeHtml(name)}</h4>${items.slice(0, 16).map((item) => `<div class="plan-item">${escapeHtml(planItemText(item))}</div>`).join('')}</div>`).join('') : '<div class="inspector-empty">No structured plan yet. Ask Ghost to analyze the references and create one before scaffolding FL.</div>'}`;
}

function planItemText(item) {
  if (typeof item === 'string') return item;
  if (!item || typeof item !== 'object') return String(item ?? '');
  const primary = item.name || item.title || item.role || item.description || item.intent;
  const range = item.startBar && item.endBar ? ` · bars ${item.startBar}–${item.endBar}` : '';
  const detail = item.description && item.description !== primary ? ` — ${item.description}` : '';
  return `${primary || JSON.stringify(item)}${range}${detail}`;
}

function activateInspector(name) {
  state.activeInspector = name;
  document.querySelectorAll('.inspector-tab').forEach((tab) => tab.classList.toggle('active', tab.dataset.tab === name));
  document.querySelectorAll('.inspector-panel').forEach((panel) => panel.classList.remove('active'));
  $(`#inspector-${name}`)?.classList.add('active');
  if (name === 'analysis') renderAnalysisInspector();
  if (name === 'plan') renderPlanInspector();
  if (name === 'fl') renderFlInspector();
}

async function loadSkills() {
  try {
    state.skills = await request('/api/skills');
    renderSkills();
  } catch (error) {
    elements.skillList.innerHTML = '';
    elements.skillContent.innerHTML = `<div class="message-error">Skills unavailable: ${escapeHtml(error.message)}</div>`;
  }
}

function renderSkills() {
  elements.skillList.innerHTML = '';
  elements.skillContent.innerHTML = '<div class="inspector-empty">Select a skill to inspect its operational workflow.</div>';
  for (const skill of state.skills) {
    const button = document.createElement('button');
    button.type = 'button'; button.className = 'skill-chip'; button.textContent = skill.name; button.title = skill.description;
    button.addEventListener('click', () => readSkill(skill.name, button));
    elements.skillList.append(button);
  }
}

async function readSkill(name, button) {
  document.querySelectorAll('.skill-chip').forEach((chip) => chip.classList.remove('active'));
  button?.classList.add('active');
  try {
    const skill = await post('/api/skills/read', { name });
    elements.skillContent.innerHTML = `<div class="subtle">${escapeHtml(skill.path)}</div>${renderMarkdown(stripFrontmatter(skill.content))}`;
  } catch (error) { elements.skillContent.innerHTML = `<div class="message-error">${escapeHtml(error.message)}</div>`; }
}

function stripFrontmatter(content) {
  return String(content || '').replace(/^---\n[\s\S]*?\n---\n/, '');
}

async function sendPrompt() {
  if (state.busy) return;
  const message = elements.prompt.value.trim();
  if (!message) return;
  const model = elements.modelSelect.value === '__custom__' ? state.info?.model : elements.modelSelect.value;
  const effort = elements.effortSelect.value;
  elements.prompt.value = '';
  autoResizePrompt();
  addMessage('user', message);
  setBusy(true, `Thinking · ${effort}`);
  try {
    const result = await post('/api/chat', { message, model, effort });
    addMessage('assistant', result.text, result.trace || []);
    state.snapshot = result.snapshot;
    if (result.model) syncModelControl(result.model);
    elements.effortSelect.value = result.effort || effort;
    await Promise.all([refreshThreads(), loadProject()]);
    renderFlInspector();
  } catch (error) {
    addMessage('assistant', `Request failed: ${error.message}`, [], true, true);
  } finally {
    setBusy(false);
    elements.prompt.focus();
    refreshInfo();
    refreshSnapshot();
  }
}

function autoResizePrompt() {
  elements.prompt.style.height = 'auto';
  elements.prompt.style.height = `${Math.min(elements.prompt.scrollHeight, 220)}px`;
}

async function bootstrap() {
  await refreshThreads();
  await Promise.all([loadHistory(), loadProject(), refreshInfo(), refreshSnapshot(), loadSkills()]);
  renderThreadSummary();
  activateInspector('fl');
}

elements.send.addEventListener('click', sendPrompt);
elements.prompt.addEventListener('input', autoResizePrompt);
elements.prompt.addEventListener('keydown', (event) => {
  if (event.key === 'Enter' && !event.shiftKey) { event.preventDefault(); sendPrompt(); }
});
elements.modelSelect.addEventListener('change', openCustomModel);
elements.threadButton.addEventListener('click', () => { renderThreadList(); elements.threadDialog.showModal(); });
elements.openThreads.addEventListener('click', () => { renderThreadList(); elements.threadDialog.showModal(); });
elements.threadFilter.addEventListener('input', renderThreadList);
elements.newThread.addEventListener('click', newThread);
elements.forkThread.addEventListener('click', forkThread);
elements.renameThread.addEventListener('click', renameThread);
elements.addAsset.addEventListener('click', openAssetDialog);
elements.composerAddAsset.addEventListener('click', openAssetDialog);
elements.assetForm.addEventListener('submit', saveAsset);
elements.projectEdit.addEventListener('click', openProjectDialog);
elements.projectForm.addEventListener('submit', saveProject);
elements.openPlan.addEventListener('click', () => activateInspector('plan'));
elements.openSkills.addEventListener('click', () => { elements.skillsDialog.showModal(); loadSkills(); });
elements.closeSkills.addEventListener('click', () => elements.skillsDialog.close());
document.querySelectorAll('.inspector-tab').forEach((tab) => tab.addEventListener('click', () => activateInspector(tab.dataset.tab)));

bootstrap();
setInterval(() => { if (!state.busy) { refreshInfo(); refreshSnapshot(); } }, 2500);
