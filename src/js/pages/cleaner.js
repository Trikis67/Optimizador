// Limpieza de archivos temporales y cachés.

import { $, app, invoke, esc, fmtSize, fmtNum, toast, helpBtn, withBusy, adminNote, loading, openTarget } from "../util.js";

let items = [];

export function init(el) {
  el.innerHTML = `
    <header class="view-head">
      <div>
        <h1>Limpieza</h1>
        <p class="muted">Borra archivos temporales y cachés que Windows y los programas dejan acumulados.</p>
      </div>
      <div class="head-actions">
        <button class="btn ghost" id="cl-scan">Volver a analizar</button>
        <button class="btn" id="cl-run">Limpiar seleccionados</button>
      </div>
    </header>
    <div id="cl-admin"></div>
    <div class="card row-card">
      <div class="grow">
        <span class="muted">Seleccionado para limpiar</span>
        <div class="ram-big" id="cl-total">–</div>
      </div>
      <div class="link-group">
        <button class="btn small ghost" data-open="cleanmgr.exe">Liberador de espacio de Windows</button>
        <button class="btn small ghost" data-open="ms-settings:storagesense">Sensor de almacenamiento</button>
      </div>
    </div>
    <div class="list" id="cl-list">${loading("Analizando…")}</div>`;

  $("cl-admin").innerHTML = adminNote("Las categorías marcadas «Admin» solo se limpian con permisos de administrador.");
  $("cl-scan").addEventListener("click", (e) => withBusy(e.currentTarget, scan, "Analizando…"));
  $("cl-run").addEventListener("click", (e) => withBusy(e.currentTarget, run, "Limpiando…"));
  el.querySelectorAll("[data-open]").forEach((b) => b.addEventListener("click", () => openTarget(b.dataset.open)));
  $("cl-list").addEventListener("change", total);
  scan();
}

async function scan() {
  try {
    items = await invoke("cleaner_scan");
    $("cl-list").innerHTML = items
      .map((c) => {
        const blocked = c.admin && !app.admin;
        return `
        <div class="row has-help ${blocked ? "is-na" : ""}">
          <div class="row-main">
            <label class="check-row grow">
              <input type="checkbox" data-id="${c.id}" ${c.default_on && !blocked && c.size > 0 ? "checked" : ""} ${blocked ? "disabled" : ""} />
              <span><strong>${esc(c.title)}</strong></span>
              ${c.admin ? `<span class="badge admin">Admin</span>` : ""}
            </label>
            ${helpBtn()}
            <span class="muted sub">${fmtNum(c.files)} archivos</span>
            <span class="size">${fmtSize(c.size)}</span>
          </div>
          <div class="help hidden"><p><b>Qué es:</b> ${esc(c.what)}</p><p><b>Ten en cuenta:</b> ${esc(c.note)}</p></div>
        </div>`;
      })
      .join("");
    total();
  } catch (e) {
    $("cl-list").innerHTML = `<p class="msg pad">${esc(String(e))}</p>`;
  }
}

function selected() {
  return [...$("cl-list").querySelectorAll("input[data-id]:checked")].map((x) => x.dataset.id);
}

function total() {
  const ids = selected();
  const sum = items.filter((i) => ids.includes(i.id)).reduce((a, i) => a + i.size, 0);
  $("cl-total").textContent = fmtSize(sum);
}

async function run() {
  const ids = selected();
  if (!ids.length) return toast("Selecciona al menos una categoría", true);
  try {
    const r = await invoke("cleaner_run", { ids });
    toast(`Liberados ${fmtSize(r.freed)}${r.skipped ? ` · ${fmtNum(r.skipped)} archivos en uso se saltaron` : ""}`);
  } catch (e) {
    toast(String(e), true);
  }
  await scan();
}
