// Programas que se abren al iniciar Windows.

import { $, app, invoke, esc, toast, helpBtn, switchHtml, adminNote, loading, openTarget } from "../util.js";

let entries = [];

export function init(el) {
  el.innerHTML = `
    <header class="view-head">
      <div>
        <h1>Inicio de Windows</h1>
        <p class="muted">Elige qué programas se abren solos al encender la PC.</p>
      </div>
      <div class="head-actions">
        <button class="btn ghost" id="st-taskmgr">Abrir Administrador de tareas</button>
      </div>
    </header>

    <div class="card has-help">
      <div class="card-title"><h3 id="st-summary">Programas de inicio</h3>${helpBtn()}</div>
      <div class="help hidden">
        <p><b>Qué hace:</b> desactivar un programa aquí evita que se abra al iniciar Windows. No lo desinstala: puedes
        abrirlo normalmente cuando quieras y volver a activarlo aquí.</p>
        <p><b>Ventaja:</b> Windows arranca más rápido y queda más RAM libre.</p>
        <p><b>Qué conviene desactivar:</b> launchers de juegos (Steam, Epic, EA), Spotify, Discord, Teams, OneDrive si no lo
        usas, actualizadores.<br><b>Qué dejar activo:</b> antivirus, drivers de audio/touchpad, software de tu mouse o
        teclado si usas sus funciones.</p>
        <p>Es el mismo interruptor que el del Administrador de tareas, así que verás los cambios en ambos lados.</p>
      </div>
    </div>
    <div id="st-admin"></div>
    <div class="list" id="st-list">${loading()}</div>`;

  $("st-admin").innerHTML = adminNote("Los programas «para todos los usuarios» necesitan permisos de administrador.");
  $("st-taskmgr").addEventListener("click", () => openTarget("taskmgr.exe"));
  $("st-list").addEventListener("change", async (e) => {
    const input = e.target.closest("input[data-entry]");
    if (!input) return;
    const entry = entries.find((x) => x.id === input.dataset.entry);
    if (entry?.admin && !app.admin) {
      input.checked = !input.checked;
      toast("Necesitas permisos de administrador para este programa.", true);
      return;
    }
    try {
      await invoke("startup_set", { id: input.dataset.entry, enabled: input.checked });
      entry.enabled = input.checked;
      toast(`${entry.name}: ${input.checked ? "se abrirá" : "ya no se abrirá"} al iniciar Windows`);
      summary();
    } catch (err) {
      input.checked = !input.checked;
      toast(String(err), true);
    }
  });
}

export function show() {
  load();
}

async function load() {
  try {
    entries = await invoke("startup_list");
    $("st-list").innerHTML =
      entries
        .map((e) => `
        <div class="row startup">
          <div class="row-main">
            <div class="grow min0">
              <div class="name"><strong>${esc(e.name)}</strong></div>
              <div class="muted small ellipsis" title="${esc(e.command)}">${esc(e.command)}</div>
            </div>
            <span class="badge">${esc(e.location)}</span>
            ${e.admin ? `<span class="badge admin">Admin</span>` : ""}
            ${switchHtml(e.enabled, { attrs: `data-entry="${esc(e.id)}"` })}
          </div>
        </div>`)
        .join("") || `<p class="muted pad">No hay programas de inicio.</p>`;
    summary();
  } catch (e) {
    $("st-list").innerHTML = `<p class="msg pad">${esc(String(e))}</p>`;
  }
}

function summary() {
  const on = entries.filter((e) => e.enabled).length;
  $("st-summary").textContent = `${on} de ${entries.length} programas se abren al iniciar`;
}
