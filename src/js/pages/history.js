// Historial: todo lo que la app cambió en tu PC, con fecha y resultado.

import { $, invoke, esc, toast, confirmDialog, loading } from "../util.js";

let entries = [];
let area = "";

export function init(el) {
  el.innerHTML = `
    <header class="view-head">
      <div>
        <h1>Historial de cambios</h1>
        <p class="muted">Cada cosa que el Optimizador hace en tu PC queda anotada aquí. Nada pasa a escondidas.</p>
      </div>
      <div class="head-actions">
        <button class="btn ghost" id="hi-folder">Abrir carpeta</button>
        <button class="btn ghost" id="hi-clear">Borrar historial</button>
      </div>
    </header>
    <div class="note">Los ajustes de <b>Optimización</b> guardan además un respaldo con los valores originales, así que
    puedes deshacerlos desde su interruptor en cualquier momento.</div>
    <div class="chips" id="hi-areas"></div>
    <div class="list" id="hi-list">${loading()}</div>`;

  $("hi-folder").addEventListener("click", () => invoke("history_open_folder").catch((e) => toast(String(e), true)));
  $("hi-clear").addEventListener("click", clearAll);
  $("hi-areas").addEventListener("click", (e) => {
    const b = e.target.closest("[data-area]");
    if (!b) return;
    area = b.dataset.area;
    render();
  });
}

export function show() {
  load();
}

async function load() {
  try {
    entries = await invoke("history_list");
    render();
  } catch (e) {
    $("hi-list").innerHTML = `<p class="pad msg">${esc(String(e))}</p>`;
  }
}

function when(ts) {
  const d = new Date(ts * 1000);
  const today = new Date();
  const sameDay = d.toDateString() === today.toDateString();
  const time = d.toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" });
  return sameDay ? `Hoy ${time}` : `${d.toLocaleDateString("es", { day: "numeric", month: "short", year: "numeric" })} ${time}`;
}

function render() {
  const areas = [...new Set(entries.map((e) => e.area))];
  $("hi-areas").innerHTML = ["", ...areas]
    .map((a) => `<button class="chip ${a === area ? "active" : ""}" data-area="${esc(a)}">${a ? esc(a) : "Todo"}</button>`)
    .join("");
  const list = area ? entries.filter((e) => e.area === area) : entries;
  $("hi-list").innerHTML = list.length
    ? list
        .map(
          (e) => `
      <div class="row">
        <div class="row-main">
          <span class="dot ${e.ok ? "dot-ok" : "dot-bad"}"></span>
          <div class="grow min0">
            <div class="name"><strong>${esc(e.action)}</strong> <span class="muted small">· ${esc(e.area)}</span></div>
            ${e.detail ? `<div class="muted small ellipsis" title="${esc(e.detail)}">${esc(e.detail)}</div>` : ""}
          </div>
          <span class="muted small nowrap">${when(e.ts)}</span>
        </div>
      </div>`
        )
        .join("")
    : `<p class="pad muted">Todavía no hay cambios registrados.</p>`;
}

async function clearAll() {
  if (!(await confirmDialog("¿Borrar el historial?", "Solo se borra la lista; los cambios hechos en tu PC no se deshacen.", "Borrar", { danger: true }))) return;
  try {
    await invoke("history_clear");
    area = "";
    await load();
  } catch (e) {
    toast(String(e), true);
  }
}
