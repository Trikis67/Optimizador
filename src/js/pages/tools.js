// Herramientas extra instalables con winget.

import { $, invoke, esc, toast, withBusy, loading, helpBtn, confirmDialog } from "../util.js";

let tools = [];

export function init(el) {
  el.innerHTML = `
    <header class="view-head">
      <div>
        <h1>Herramientas</h1>
        <p class="muted">Programas gratuitos y confiables que complementan al Optimizador.</p>
      </div>
      <div class="head-actions has-help">
        ${helpBtn()}
        <button class="btn ghost" id="tl-refresh">Actualizar estado</button>
        <div class="help hidden pop">
          <p>Se instalan con <b>winget</b>, el instalador oficial de Microsoft que ya viene en Windows 10 y 11. Descarga
          siempre desde la fuente oficial de cada programa.</p>
          <p>Algunos instaladores piden permiso de administrador (aparecerá la ventana de Windows). Instalar puede tardar
          un poco: espera a que el botón termine.</p>
        </div>
      </div>
    </header>
    <div id="tl-grid" class="tool-grid">${loading("Consultando programas instalados…")}</div>`;

  $("tl-refresh").addEventListener("click", (e) => withBusy(e.currentTarget, load, "Consultando…"));
  $("tl-grid").addEventListener("click", onClick);
  load();
}

async function load() {
  try {
    tools = await invoke("tools_list");
    const cats = [...new Set(tools.map((t) => t.cat))];
    $("tl-grid").innerHTML = cats
      .map((c) => `
        <h2 class="tool-cat">${esc(c)}</h2>
        ${tools
          .filter((t) => t.cat === c)
          .map((t) => `
          <div class="card tool ${t.installed ? "is-on" : ""}">
            <div class="tool-head">
              <h3>${esc(t.name)}</h3>
              ${t.installed ? `<span class="badge ok">Instalado</span>` : ""}
            </div>
            <p class="muted">${esc(t.what)}</p>
            <div class="tool-actions">
              ${t.installed
                ? `<button class="btn small ghost" data-uninstall="${esc(t.id)}">Desinstalar</button>`
                : `<button class="btn small" data-install="${esc(t.id)}">Instalar</button>`}
            </div>
          </div>`)
          .join("")}`)
      .join("");
  } catch (e) {
    $("tl-grid").innerHTML = `<p class="msg">${esc(String(e))}</p>`;
  }
}

async function onClick(e) {
  const b = e.target.closest("button[data-install], button[data-uninstall]");
  if (!b) return;
  const install = !!b.dataset.install;
  const id = b.dataset.install || b.dataset.uninstall;
  const tool = tools.find((t) => t.id === id);
  if (!install) {
    const ok = await confirmDialog("Desinstalar", `¿Desinstalar ${tool.name}?`, "Desinstalar", { danger: true });
    if (!ok) return;
  }
  await withBusy(b, async () => {
    try {
      await invoke(install ? "tool_install" : "tool_uninstall", { id });
      toast(`${tool.name} ${install ? "instalado" : "desinstalado"} ✓`);
    } catch (err) {
      toast(String(err), true);
    }
  }, install ? "Instalando…" : "Desinstalando…");
  load();
}
