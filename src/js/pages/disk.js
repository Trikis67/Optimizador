// Analizador de disco (tipo WizTree).

import { $, invoke, listen, fmtSize, fmtNum, esc, toast, confirmDialog } from "../util.js";
import { squarify } from "../treemap.js";

const CATEGORIES = {
  video: { color: "#e5484d", ext: ["mp4", "mkv", "avi", "mov", "wmv", "webm", "flv", "m4v"] },
  imagen: { color: "#f5a524", ext: ["jpg", "jpeg", "png", "gif", "bmp", "webp", "heic", "psd", "raw", "tiff"] },
  audio: { color: "#d6409f", ext: ["mp3", "wav", "flac", "aac", "ogg", "m4a", "wma"] },
  comprimido: { color: "#8e4ec6", ext: ["zip", "rar", "7z", "tar", "gz", "iso", "cab", "img"] },
  programa: { color: "#30a46c", ext: ["exe", "msi", "dll", "sys", "appx", "msix"] },
  documento: { color: "#12a594", ext: ["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "csv"] },
  sistema: { color: "#6e6e78", ext: ["tmp", "log", "dmp", "bak", "cache", "etl", "vhdx", "vmdk"] },
};
const EXT_COLOR = {};
for (const [cat, { color, ext }] of Object.entries(CATEGORIES)) for (const e of ext) EXT_COLOR[e] = { color, cat };

function fileInfo(name) {
  const dot = name.lastIndexOf(".");
  const ext = dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
  return EXT_COLOR[ext] || { color: "#5b6b8c", cat: "otro" };
}
function folderColor(i) {
  const hues = [212, 196, 228, 186, 244, 170];
  return `hsl(${hues[i % hues.length]} 55% ${42 - (i % 3) * 5}%)`;
}

let segments = [];
let currentListing = null;
let rootPath = "";
let topFiles = [];

const ACTIONS = `
  <button class="icon-btn" data-act="open" title="Mostrar en el Explorador">📂</button>
  <button class="icon-btn" data-act="trash" title="Enviar a la papelera">🗑</button>`;

export function init(el) {
  el.innerHTML = `
    <header class="view-head">
      <div>
        <h1>Analizador de disco</h1>
        <p class="muted">Elige una unidad o escribe una carpeta para ver qué ocupa espacio.</p>
      </div>
    </header>

    <div id="disk-start">
      <div id="drive-list" class="drive-grid"></div>
      <div class="custom-path">
        <input id="path-input" type="text" placeholder="O escribe una carpeta, ej. C:\\Users\\Gabriel\\Downloads" />
        <button id="path-btn" class="btn">Analizar carpeta</button>
      </div>
    </div>

    <div id="disk-progress" class="card row-card hidden">
      <div class="spinner"></div>
      <div class="grow">
        <strong>Analizando <span id="prog-path"></span></strong>
        <p class="muted" id="prog-text">Preparando…</p>
      </div>
      <button id="cancel-btn" class="btn ghost">Cancelar</button>
    </div>

    <div id="disk-result" class="hidden">
      <div class="result-bar">
        <div class="crumbs" id="crumbs"></div>
        <div class="result-actions">
          <span class="muted" id="scan-info"></span>
          <button id="new-scan-btn" class="btn ghost">Nuevo análisis</button>
        </div>
      </div>
      <div class="tabs" id="disk-tabs">
        <button class="tab active" data-tab="folders">Carpetas</button>
        <button class="tab" data-tab="top">Archivos más grandes</button>
        <button class="tab" data-tab="types">Tipos de archivo</button>
      </div>
      <div id="tab-folders" class="tab-body active split">
        <div class="list" id="folder-list"></div>
        <div class="treemap" id="treemap"></div>
      </div>
      <div id="tab-top" class="tab-body"><div class="list" id="top-list"></div></div>
      <div id="tab-types" class="tab-body"><div class="list" id="type-list"></div></div>
    </div>`;

  $("cancel-btn").addEventListener("click", () => invoke("cancel_scan"));
  $("path-btn").addEventListener("click", () => startScan($("path-input").value.trim()));
  $("path-input").addEventListener("keydown", (e) => e.key === "Enter" && startScan($("path-input").value.trim()));
  $("new-scan-btn").addEventListener("click", () => {
    currentListing = null;
    loadDrives();
    showPanel("disk-start");
  });
  $("disk-tabs").querySelectorAll(".tab").forEach((t) => t.addEventListener("click", () => selectTab(t.dataset.tab)));
  $("folder-list").addEventListener("click", onFolderClick);
  $("top-list").addEventListener("click", onTopClick);
  setupTreemap();
  loadDrives();
}

export function show() {
  if (currentListing) renderTreemap();
}

async function loadDrives() {
  try {
    const drives = await invoke("list_drives");
    $("drive-list").innerHTML = drives
      .map((d) => {
        const used = d.total - d.free;
        const pct = d.total ? (used / d.total) * 100 : 0;
        return `
        <div class="card drive">
          <div class="drive-top">
            <strong>${esc(d.mount)}</strong>
            <span class="muted">${esc(d.name || "Disco local")} · ${esc(d.fs)}</span>
            ${d.kind ? `<span class="badge">${esc(d.kind)}</span>` : ""}
          </div>
          <div class="bar${pct > 90 ? " warn" : ""}"><i style="width:${pct.toFixed(1)}%"></i></div>
          <div class="drive-bottom">
            <span>${fmtSize(d.free)} libres de ${fmtSize(d.total)}</span>
            <button class="btn small" data-path="${esc(d.mount)}">Analizar</button>
          </div>
        </div>`;
      })
      .join("");
    $("drive-list").querySelectorAll("button[data-path]").forEach((b) => b.addEventListener("click", () => startScan(b.dataset.path)));
  } catch (e) {
    toast(String(e), true);
  }
}

function showPanel(name) {
  for (const p of ["disk-start", "disk-progress", "disk-result"]) $(p).classList.toggle("hidden", p !== name);
}

async function startScan(path) {
  if (!path) return;
  $("prog-path").textContent = path;
  $("prog-text").textContent = "Preparando…";
  showPanel("disk-progress");
  const unlisten = await listen("scan-progress", (e) => {
    const p = e.payload;
    $("prog-text").textContent = `${fmtNum(p.files)} archivos · ${fmtNum(p.dirs)} carpetas · ${fmtSize(p.bytes)}`;
  });
  try {
    const s = await invoke("start_scan", { path });
    rootPath = s.path;
    const extra = s.errors ? ` · ${fmtNum(s.errors)} carpetas sin acceso` : "";
    $("scan-info").textContent =
      `${fmtSize(s.size)} en ${fmtNum(s.files)} archivos · ${s.seconds.toFixed(1)} s${extra}` +
      (s.cancelled ? " · (cancelado, resultado parcial)" : "");
    showPanel("disk-result");
    selectTab("folders");
    await openFolder([]);
  } catch (e) {
    toast(String(e), true);
    showPanel("disk-start");
  } finally {
    unlisten();
  }
}

function selectTab(name) {
  $("disk-tabs").querySelectorAll(".tab").forEach((t) => t.classList.toggle("active", t.dataset.tab === name));
  for (const b of ["folders", "top", "types"]) $(`tab-${b}`).classList.toggle("active", b === name);
  if (name === "top") loadTopFiles();
  if (name === "types") loadTypes();
  if (name === "folders") renderTreemap();
}

async function openFolder(segs) {
  try {
    currentListing = await invoke("get_children", { segments: segs });
    segments = segs;
    renderCrumbs();
    renderFolderList();
    renderTreemap();
  } catch (e) {
    toast(String(e), true);
  }
}

function renderCrumbs() {
  const parts = [{ label: rootPath, segs: [] }];
  segments.forEach((s, i) => parts.push({ label: s, segs: segments.slice(0, i + 1) }));
  $("crumbs").innerHTML = parts.map((p, i) => `<button class="crumb" data-i="${i}">${esc(p.label)}</button>`).join('<span class="sep">›</span>');
  $("crumbs").querySelectorAll(".crumb").forEach((b) => b.addEventListener("click", () => openFolder(parts[+b.dataset.i].segs)));
}

function itemRow(name, size, pct, sub, color, actions) {
  return `
    <div class="row-main">
      <span class="swatch" style="background:${color}"></span>
      <span class="name" title="${esc(name)}">${esc(name)}</span>
      ${sub ? `<span class="muted sub">${sub}</span>` : ""}
      <span class="size">${fmtSize(size)}</span>
      <span class="actions">${actions}</span>
    </div>
    <div class="pct"><i style="width:${Math.max(pct, 0.3).toFixed(2)}%;background:${color}"></i></div>`;
}

function renderFolderList() {
  const L = currentListing;
  if (!L.items.length) {
    $("folder-list").innerHTML = `<p class="muted pad">Carpeta vacía</p>`;
    return;
  }
  let html = segments.length ? `<div class="row up" data-up="1">⬆ Subir un nivel</div>` : "";
  L.items.forEach((it, i) => {
    const pct = L.size ? (it.size / L.size) * 100 : 0;
    const color = it.is_dir ? folderColor(i) : fileInfo(it.name).color;
    const sub = it.is_dir ? `${fmtNum(it.files)} archivos` : "";
    html += `<div class="row ${it.is_dir ? "dir" : ""}" data-i="${i}">${itemRow((it.is_dir ? "📁 " : "") + it.name, it.size, pct, sub, color, ACTIONS)}</div>`;
  });
  if (L.total_items > L.items.length) html += `<p class="muted pad">Mostrando ${L.items.length} de ${fmtNum(L.total_items)} elementos</p>`;
  $("folder-list").innerHTML = html;
}

async function onFolderClick(e) {
  const row = e.target.closest(".row");
  if (!row) return;
  if (row.dataset.up) return openFolder(segments.slice(0, -1));
  const it = currentListing.items[+row.dataset.i];
  const segs = [...segments, it.name];
  const act = e.target.dataset.act;
  if (act === "open") return invoke("open_in_explorer", { segments: segs }).catch((err) => toast(String(err), true));
  if (act === "trash") return trashItem(segs, it.name, it.size);
  if (it.is_dir) openFolder(segs);
}

async function trashItem(segs, name, size) {
  const ok = await confirmDialog(
    "¿Enviar a la papelera?",
    `"${name}" (${fmtSize(size)}) se moverá a la papelera de reciclaje. Podrás recuperarlo desde allí.`,
    "Enviar a la papelera",
    { danger: true }
  );
  if (!ok) return;
  try {
    const freed = await invoke("move_to_trash", { segments: segs });
    toast(`Movido a la papelera (${fmtSize(freed)}). Vacía la papelera para liberar el espacio.`);
    await openFolder(segments);
    if ($("tab-top").classList.contains("active")) loadTopFiles();
  } catch (e) {
    toast(String(e), true);
  }
}

function renderTreemap() {
  const box = $("treemap");
  if (!currentListing || !box || !box.offsetWidth) return;
  const items = currentListing.items.slice(0, 400).map((it, i) => ({ ...it, i }));
  const rects = squarify(items, { x: 0, y: 0, w: box.clientWidth, h: box.clientHeight });
  box.innerHTML = rects
    .map(({ item, x, y, w, h }) => {
      const color = item.is_dir ? folderColor(item.i) : fileInfo(item.name).color;
      const label = w > 70 && h > 30 ? `<span class="tm-label">${esc(item.name)}<small>${fmtSize(item.size)}</small></span>` : "";
      return `<div class="tm ${item.is_dir ? "dir" : ""}" data-i="${item.i}"
        style="left:${x}px;top:${y}px;width:${w}px;height:${h}px;background:${color}">${label}</div>`;
    })
    .join("");
}

function setupTreemap() {
  const box = $("treemap");
  box.addEventListener("click", (e) => {
    const tile = e.target.closest(".tm");
    if (!tile) return;
    const it = currentListing.items[+tile.dataset.i];
    if (it.is_dir) openFolder([...segments, it.name]);
  });
  box.addEventListener("mousemove", (e) => {
    const tile = e.target.closest(".tm");
    const tip = $("tooltip");
    if (!tile) return tip.classList.add("hidden");
    const it = currentListing.items[+tile.dataset.i];
    const pct = currentListing.size ? ((it.size / currentListing.size) * 100).toFixed(1) : 0;
    tip.innerHTML = `<strong>${esc(it.name)}</strong><br>${fmtSize(it.size)} · ${pct}%` +
      (it.is_dir ? `<br>${fmtNum(it.files)} archivos · clic para entrar` : `<br>${fileInfo(it.name).cat}`);
    tip.classList.remove("hidden");
    tip.style.left = `${Math.min(e.clientX + 14, window.innerWidth - tip.offsetWidth - 8)}px`;
    tip.style.top = `${Math.min(e.clientY + 14, window.innerHeight - tip.offsetHeight - 8)}px`;
  });
  box.addEventListener("mouseleave", () => $("tooltip").classList.add("hidden"));
  new ResizeObserver(() => renderTreemap()).observe(box);
}

async function loadTopFiles() {
  try {
    topFiles = await invoke("top_files", { limit: 200 });
    const max = topFiles[0]?.size || 1;
    $("top-list").innerHTML =
      topFiles.map((f, i) => `<div class="row" data-i="${i}">${itemRow(f.path, f.size, (f.size / max) * 100, "", fileInfo(f.path).color, ACTIONS)}</div>`).join("") ||
      `<p class="muted pad">No hay archivos</p>`;
  } catch (e) {
    toast(String(e), true);
  }
}

function onTopClick(e) {
  const row = e.target.closest(".row");
  const act = e.target.dataset.act;
  if (!row || !act) return;
  const f = topFiles[+row.dataset.i];
  if (act === "open") invoke("open_in_explorer", { segments: f.segments }).catch((err) => toast(String(err), true));
  if (act === "trash") trashItem(f.segments, f.path, f.size);
}

async function loadTypes() {
  try {
    const types = await invoke("ext_stats");
    const max = types[0]?.size || 1;
    $("type-list").innerHTML = types
      .map((t) => {
        const info = fileInfo("x." + t.ext);
        return `<div class="row">${itemRow("." + t.ext, t.size, (t.size / max) * 100, `${fmtNum(t.count)} archivos · ${info.cat}`, info.color, "")}</div>`;
      })
      .join("");
  } catch (e) {
    toast(String(e), true);
  }
}
