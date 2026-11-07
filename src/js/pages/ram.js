// RAM: composición de la memoria, limpieza y limpieza automática.

import { $, invoke, esc, fmtSize, fmtPct, toast, helpBtn, withBusy, adminNote } from "../util.js";

const OPTIONS = [
  {
    id: "working_sets",
    title: "Memoria de trabajo de los programas",
    def: true,
    admin: false,
    what: "Pide a cada programa que devuelva la RAM que no está usando activamente. Es lo que hace el botón «Limpiar» de Mem Reduct.",
    note: "Es seguro. Los programas recuperan lo que necesiten al volver a usarlos (pueden tardar un instante). Sin admin solo afecta a tus programas.",
  },
  {
    id: "standby_low",
    title: "Caché de baja prioridad",
    def: true,
    admin: true,
    what: "Libera la parte menos importante de la caché (lista standby de baja prioridad).",
    note: "Equilibrio entre liberar RAM y no perder la caché útil.",
  },
  {
    id: "standby",
    title: "Toda la caché (lista standby)",
    def: false,
    admin: true,
    what: "Windows guarda en RAM archivos que usaste hace poco para abrirlos más rápido. Esto la vacía por completo.",
    note: "Útil si un juego tiene tirones porque la RAM está llena de caché. Los programas pueden abrir más lento hasta que la caché se reconstruya.",
  },
  {
    id: "modified",
    title: "Lista modificada",
    def: false,
    admin: true,
    what: "Escribe al disco datos pendientes que están ocupando RAM.",
    note: "Seguro; solo genera algo de escritura en disco.",
  },
];

const STORE_KEY = "optimizador.ram";
let settings = loadSettings();
let timer = null;
let lastAuto = 0;

function loadSettings() {
  const def = { options: OPTIONS.filter((o) => o.def).map((o) => o.id), auto: false, threshold: 85 };
  try {
    return { ...def, ...JSON.parse(localStorage.getItem(STORE_KEY) || "{}") };
  } catch {
    return def;
  }
}
function saveSettings() {
  try {
    localStorage.setItem(STORE_KEY, JSON.stringify(settings));
  } catch {
    /* sin almacenamiento disponible */
  }
}

export function init(el) {
  el.innerHTML = `
    <header class="view-head">
      <div>
        <h1>RAM</h1>
        <p class="muted">Cómo se está usando tu memoria y opciones para liberarla.</p>
      </div>
    </header>
    <div id="ram-admin"></div>

    <div class="card">
      <div class="ram-top">
        <div>
          <span class="muted">En uso</span>
          <div class="ram-big" id="ram-big">–</div>
          <span class="muted" id="ram-total"></span>
        </div>
        <button class="btn big" id="ram-clean-btn">Limpiar RAM ahora</button>
      </div>
      <div class="stack-bar" id="ram-bar"></div>
      <div class="legend" id="ram-legend"></div>
    </div>

    <div class="grid-2">
      <div class="card">
        <div class="card-title"><h3>Qué limpiar</h3></div>
        <div id="ram-options"></div>
      </div>
      <div class="card has-help">
        <div class="card-title"><h3>Limpieza automática</h3>${helpBtn()}</div>
        <div class="help hidden">
          <p><b>Qué hace:</b> cuando la RAM en uso pasa del porcentaje que elijas, limpia con las opciones marcadas
          (como máximo una vez cada 2 minutos).</p>
          <p><b>Ten en cuenta:</b> solo funciona mientras esta app está abierta. Para algo que corra siempre en la
          bandeja, instala Mem Reduct desde Herramientas.</p>
        </div>
        <label class="check-row">
          <input type="checkbox" id="auto-on" ${settings.auto ? "checked" : ""} />
          <span>Limpiar sola cuando la RAM pase de</span>
          <select id="auto-th">
            ${[75, 80, 85, 90, 95].map((v) => `<option value="${v}" ${v === settings.threshold ? "selected" : ""}>${v}%</option>`).join("")}
          </select>
        </label>
        <p class="muted small" id="auto-last"></p>
        <div class="note">
          <b>¿Sirve limpiar la RAM?</b> Windows usa la RAM libre como caché a propósito, así que ver la RAM «llena» no
          siempre es malo. Limpiar ayuda cuando tienes poca RAM (8 GB o menos), cuando un programa no la suelta o antes de
          abrir un juego pesado. No aumenta los FPS por sí solo.
        </div>
      </div>
    </div>

    <h2>Programas que más memoria usan</h2>
    <div class="list" id="ram-procs"></div>`;

  $("ram-admin").innerHTML = adminNote("Liberar la caché del sistema necesita permisos de administrador.");
  renderOptions();
  $("ram-clean-btn").addEventListener("click", (e) => withBusy(e.currentTarget, () => clean(false), "Limpiando…"));
  $("auto-on").addEventListener("change", (e) => {
    settings.auto = e.target.checked;
    saveSettings();
  });
  $("auto-th").addEventListener("change", (e) => {
    settings.threshold = Number(e.target.value);
    saveSettings();
  });
  $("ram-procs").addEventListener("click", async (e) => {
    const b = e.target.closest("button[data-trim]");
    if (!b) return;
    const ok = await invoke("ram_trim_process", { pid: Number(b.dataset.trim) });
    toast(ok ? `Memoria de ${b.dataset.name} liberada` : "Windows no permitió liberar ese proceso", !ok);
    refresh();
  });
}

function renderOptions() {
  $("ram-options").innerHTML = OPTIONS.map(
    (o) => `
    <div class="opt has-help">
      <label class="check-row">
        <input type="checkbox" data-opt="${o.id}" ${settings.options.includes(o.id) ? "checked" : ""} />
        <span>${esc(o.title)}</span>
        ${o.admin ? `<span class="badge admin">Admin</span>` : ""}
      </label>
      ${helpBtn()}
      <div class="help hidden"><p><b>Qué hace:</b> ${esc(o.what)}</p><p><b>Ten en cuenta:</b> ${esc(o.note)}</p></div>
    </div>`
  ).join("");
  $("ram-options").querySelectorAll("input[data-opt]").forEach((c) =>
    c.addEventListener("change", () => {
      settings.options = [...$("ram-options").querySelectorAll("input[data-opt]:checked")].map((x) => x.dataset.opt);
      saveSettings();
    })
  );
}

export function show() {
  refresh();
  timer = setInterval(refresh, 2000);
}
export function hide() {
  clearInterval(timer);
}

async function refresh() {
  try {
    const r = await invoke("ram_status");
    const { total, available } = r.status;
    const standby = Math.min(r.detail.standby, available);
    const modified = r.detail.modified;
    const free = Math.max(0, available - standby);
    const inUse = Math.max(0, total - available - modified);
    const used = total - available;
    $("ram-big").textContent = `${fmtSize(used)} · ${fmtPct((used / total) * 100)}`;
    $("ram-total").textContent = `de ${fmtSize(total)} · comprometida ${fmtSize(r.detail.committed)} de ${fmtSize(r.detail.commit_limit)}`;

    const parts = [
      { k: "En uso", v: inUse, c: "#b98acb", d: "Memoria que los programas están usando." },
      { k: "Modificada", v: modified, c: "#c8553d", d: "Datos que deben escribirse al disco antes de reutilizarse." },
      { k: "En caché", v: standby, c: "#d19a3c", d: "Archivos recientes guardados para abrir más rápido. Se libera sola si hace falta." },
      { k: "Libre", v: free, c: "#38332a", d: "Totalmente vacía." },
    ];
    $("ram-bar").innerHTML = parts
      .map((p) => `<i style="width:${((p.v / total) * 100).toFixed(2)}%;background:${p.c}" title="${p.k}: ${fmtSize(p.v)}"></i>`)
      .join("");
    $("ram-legend").innerHTML = parts
      .map((p) => `<div class="leg" title="${esc(p.d)}"><span class="swatch" style="background:${p.c}"></span>${p.k}<strong>${fmtSize(p.v)}</strong></div>`)
      .join("");

    const max = r.processes[0]?.mem || 1;
    $("ram-procs").innerHTML = r.processes
      .map((p) => `
        <div class="row">
          <div class="row-main">
            <span class="name">${esc(p.name)}</span>
            <span class="muted sub">PID ${p.pid}</span>
            <span class="size">${fmtSize(p.mem)}</span>
            <button class="btn tiny ghost" data-trim="${p.pid}" data-name="${esc(p.name)}">Liberar</button>
          </div>
          <div class="pct"><i style="width:${((p.mem / max) * 100).toFixed(1)}%;background:#b98acb"></i></div>
        </div>`)
      .join("");
  } catch (e) {
    console.error(e);
  }
}

async function clean(auto) {
  const options = settings.options.length ? settings.options : ["working_sets"];
  try {
    const r = await invoke("ram_clean", { options, auto: !!auto });
    const freed = Math.max(0, r.freed);
    const msg = `${auto ? "Limpieza automática: " : ""}liberados ${fmtSize(freed)}`;
    toast(r.warnings.length && !auto ? `${msg}. ${r.warnings.join(". ")}` : msg, false);
    if (auto && $("auto-last")) $("auto-last").textContent = `Última limpieza automática: ${new Date().toLocaleTimeString()} (${fmtSize(freed)})`;
    if (!auto) refresh();
  } catch (e) {
    toast(String(e), true);
  }
}

/** Revisa la RAM cada 15 s (en cualquier página) y limpia si se pasa del límite. */
export function startAutoClean() {
  setInterval(async () => {
    if (!settings.auto || Date.now() - lastAuto < 120_000) return;
    try {
      const r = await invoke("ram_status");
      const pct = ((r.status.total - r.status.available) / r.status.total) * 100;
      if (pct >= settings.threshold) {
        lastAuto = Date.now();
        await clean(true);
      }
    } catch {
      /* ignorar */
    }
  }, 15_000);
}

