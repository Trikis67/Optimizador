// Drivers: driver de video, guía de errores de dispositivos y limpieza de drivers obsoletos.

import { $, app, invoke, esc, fmtSize, toast, helpBtn, loading, openTarget, withBusy, confirmDialog, adminNote } from "../util.js";

let obsolete = [];
let leftovers = null;

export function init(el) {
  el.innerHTML = `
    <header class="view-head">
      <div>
        <h1>Drivers</h1>
        <p class="muted">Revisa tus drivers, soluciona errores y recupera el espacio que dejan las versiones viejas.</p>
      </div>
      <div class="head-actions">
        <button class="btn ghost" data-open="devmgmt.msc">Administrador de dispositivos</button>
        <button class="btn" data-open="ms-settings:windowsupdate-optionalupdates">Drivers en Windows Update</button>
      </div>
    </header>
    <div id="drv-admin"></div>

    <div class="section-head has-help">
      <h2>Tarjeta gráfica</h2>${helpBtn()}
      <div class="help hidden">
        <p><b>Por qué importa:</b> el driver de video es el que más influye en el rendimiento de los juegos. Los nuevos
        traen optimizaciones para juegos recientes y corrigen errores.</p>
        <p><b>Cómo actualizar:</b> usa la app oficial del fabricante (NVIDIA App, AMD Adrenalin o Intel DSA). Evita
        programas de «actualizar todos los drivers» de terceros: suelen instalar versiones incorrectas o publicidad.</p>
        <p><b>Instalación limpia:</b> si tienes problemas después de actualizar, usa DDU (en Herramientas) para borrar el
        driver anterior y vuelve a instalar.</p>
      </div>
    </div>
    <div id="gpu-list" class="grid-2">${loading("Leyendo drivers…")}</div>

    <div class="section-head has-help">
      <h2>Errores de dispositivos</h2>${helpBtn()}
      <div class="help hidden">
        <p>Dispositivos que Windows marca con error (el triángulo amarillo del Administrador de dispositivos). Para cada
        uno te decimos qué significa su código de error y cómo arreglarlo, con un botón para el paso más común.</p>
      </div>
      <button class="btn small ghost push" id="dev-rescan">Buscar cambios de hardware</button>
    </div>
    <div class="list" id="dev-list">${loading()}</div>

    <div class="section-head has-help">
      <h2>Drivers obsoletos</h2>${helpBtn()}
      <div class="help hidden">
        <p><b>Qué es:</b> cada vez que actualizas un driver, Windows guarda una copia completa del paquete en el
        <b>DriverStore</b> (C:\\Windows\\System32\\DriverStore) y nunca borra las versiones anteriores. Con NVIDIA cada
        versión ocupa 1–2 GB, así que con los años se juntan decenas de GB.</p>
        <p><b>Qué borramos:</b> solo paquetes que tienen una versión más nueva instalada y que ningún dispositivo
        conectado está usando. Se borran con la herramienta oficial de Windows (<i>pnputil</i>), que se niega a borrar
        cualquier driver en uso.</p>
        <p><b>Ten en cuenta:</b> después ya no podrás «revertir al driver anterior» desde el Administrador de
        dispositivos. Si algún día lo necesitas, descarga esa versión de la página del fabricante.</p>
      </div>
      <div class="push head-actions">
        <button class="btn small ghost" id="store-scan">Buscar drivers obsoletos</button>
        <button class="btn small hidden" id="store-clean">Borrar seleccionados</button>
      </div>
    </div>
    <div class="card row-card stat-strip" id="store-summary">
      <div><span class="muted small">Tamaño del DriverStore</span><strong id="st-size">–</strong></div>
      <div><span class="muted small">Paquetes instalados</span><strong id="st-count">–</strong></div>
      <div><span class="muted small">Recuperable</span><strong class="accent-text" id="st-free">–</strong></div>
      <div><span class="muted small">Restos de instaladores</span><strong id="st-left">–</strong></div>
    </div>
    <div class="list" id="store-list"><p class="pad muted">Pulsa «Buscar drivers obsoletos». Tarda unos segundos.</p></div>
    <div class="card row-card hidden" id="left-card">
      <div class="grow">
        <strong>Restos de instaladores de NVIDIA y AMD</strong>
        <p class="muted small">C:\\NVIDIA, C:\\AMD y las descargas de la app de NVIDIA. Son copias de instaladores ya
        usados, no el driver instalado.</p>
      </div>
      <button class="btn small" id="left-clean">Limpiar</button>
    </div>

    <div class="note">
      <b>Otros drivers:</b> para chipset, audio, red y Bluetooth, lo más seguro es descargarlos de la página de soporte del
      fabricante de tu laptop o de tu placa madre, o desde «Drivers en Windows Update».
    </div>`;

  $("drv-admin").innerHTML = adminNote("Para borrar drivers obsoletos y reparar dispositivos se necesitan permisos de administrador.");
  el.querySelectorAll("[data-open]").forEach((b) => b.addEventListener("click", () => openTarget(b.dataset.open)));
  $("gpu-list").addEventListener("click", (e) => {
    const b = e.target.closest("button[data-url]");
    if (b) openTarget(b.dataset.url);
  });
  $("dev-list").addEventListener("click", onDeviceAction);
  $("dev-rescan").addEventListener("click", (e) => withBusy(e.currentTarget, rescan, "Buscando…"));
  $("store-scan").addEventListener("click", (e) => withBusy(e.currentTarget, scanStore, "Analizando…"));
  $("store-clean").addEventListener("click", (e) => withBusy(e.currentTarget, cleanStore, "Borrando…"));
  $("store-list").addEventListener("change", updateSelected);
  $("left-clean").addEventListener("click", (e) => withBusy(e.currentTarget, cleanLeftovers, "Limpiando…"));
  load();
  loadLeftovers();
}

const ACTIONS = {
  update: { label: "Buscar driver", target: "ms-settings:windowsupdate-optionalupdates" },
  devmgmt: { label: "Abrir Administrador", target: "devmgmt.msc" },
  scan: { label: "Buscar cambios de hardware" },
  enable: { label: "Habilitar" },
  restart: { label: "Reiniciar más tarde" },
};

async function load() {
  try {
    const d = await invoke("drivers_info");
    $("gpu-list").innerHTML =
      d.gpus
        .map((g) => {
          const old = g.age_days != null && g.age_days > 180;
          const age =
            g.age_days == null ? "" : g.age_days < 60 ? `hace ${g.age_days} días` : `hace ${Math.round(g.age_days / 30)} meses`;
          return `
          <div class="card gpu-card">
            <h3>${esc(g.name)}</h3>
            <div class="kv"><span class="muted">Versión del driver</span><strong>${esc(g.version || "–")}</strong></div>
            <div class="kv"><span class="muted">Fecha</span><strong>${esc(g.date || "–")} ${age ? `<span class="muted">(${age})</span>` : ""}</strong></div>
            ${g.vram ? `<div class="kv"><span class="muted">Memoria de video</span><strong>${fmtSize(g.vram)}</strong></div>` : ""}
            ${old ? `<div class="note warn">Tu driver tiene más de 6 meses. Probablemente hay una versión más nueva.</div>` : ""}
            ${g.url ? `<button class="btn ${old ? "" : "ghost"}" data-url="${esc(g.url)}">Descargar ${esc(g.tool)}</button>` : ""}
          </div>`;
        })
        .join("") || `<p class="muted">No se detectó la tarjeta gráfica.</p>`;

    $("dev-list").innerHTML = d.problems.length
      ? d.problems
          .map((p) => {
            const a = ACTIONS[p.action];
            const btn =
              a && p.action !== "restart"
                ? `<button class="btn tiny ${p.action === "enable" ? "" : "ghost"}" data-act="${p.action}" data-id="${esc(p.id)}"
                    data-name="${esc(p.name)}" ${a.target ? `data-target="${a.target}"` : ""}>${a.label}</button>`
                : "";
            return `
          <div class="row device">
            <div class="row-main">
              <span class="dot ${p.code === 45 ? "dot-muted" : "dot-warn"}"></span>
              <div class="grow min0">
                <div class="name"><strong>${esc(p.name)}</strong> <span class="muted small">${esc(p.class)} · código ${p.code}</span></div>
                <div class="small">${esc(p.explain)}</div>
                <div class="muted small fix">→ ${esc(p.fix)}</div>
              </div>
              ${btn}
            </div>
          </div>`;
          })
          .join("")
      : `<p class="pad ok-text">✓ Todos tus dispositivos funcionan correctamente.</p>`;
  } catch (e) {
    $("gpu-list").innerHTML = `<p class="msg">${esc(String(e))}</p>`;
    $("dev-list").innerHTML = "";
  }
}

async function onDeviceAction(e) {
  const b = e.target.closest("button[data-act]");
  if (!b) return;
  const act = b.dataset.act;
  if (b.dataset.target) return openTarget(b.dataset.target);
  if (act === "scan") return withBusy(b, rescan, "Buscando…");
  if (act === "enable") {
    await withBusy(b, async () => {
      try {
        await invoke("drivers_enable_device", { id: b.dataset.id, name: b.dataset.name });
        toast(`${b.dataset.name} habilitado`);
        load();
      } catch (err) {
        toast(String(err), true);
      }
    }, "Habilitando…");
  }
}

async function rescan() {
  try {
    await invoke("drivers_scan_devices");
    toast("Windows volvió a detectar el hardware");
    await load();
  } catch (e) {
    toast(String(e), true);
  }
}

async function scanStore() {
  if (!app.admin) return toast("Necesitas abrir la app como administrador para revisar el DriverStore.", true);
  $("store-list").innerHTML = loading("Leyendo los drivers guardados por Windows…");
  try {
    const r = await invoke("drivers_store_scan");
    obsolete = r.obsolete;
    $("st-size").textContent = fmtSize(r.store_size);
    $("st-count").textContent = r.total_packages;
    renderStore();
  } catch (e) {
    $("store-list").innerHTML = `<p class="pad msg">${esc(String(e))}</p>`;
  }
}

function renderStore() {
  const total = obsolete.reduce((s, p) => s + p.size, 0);
  $("st-free").textContent = fmtSize(total);
  $("store-clean").classList.toggle("hidden", !obsolete.length);
  if (!obsolete.length) {
    $("store-list").innerHTML = `<p class="pad ok-text">✓ No hay drivers obsoletos. Tu DriverStore está limpio.</p>`;
    return;
  }
  $("store-list").innerHTML =
    `<label class="row check-row select-all"><input type="checkbox" id="store-all" checked /> <strong>Seleccionar todos</strong>
       <span class="muted small push">${obsolete.length} paquetes</span></label>` +
    obsolete
      .map(
        (p, i) => `
      <label class="row check-row">
        <input type="checkbox" data-i="${i}" checked />
        <div class="grow min0">
          <div class="name"><strong>${esc(p.provider)}</strong> · ${esc(p.class)} <span class="muted small">${esc(p.inf)}</span></div>
          <div class="muted small">Versión ${esc(p.version)}${p.date ? ` (${esc(p.date)})` : ""} → instalada: ${esc(p.newest)}</div>
        </div>
        <span class="size">${fmtSize(p.size)}</span>
      </label>`
      )
      .join("");
  updateSelected();
}

function selected() {
  return [...document.querySelectorAll("#store-list input[data-i]:checked")].map((c) => obsolete[Number(c.dataset.i)]);
}

function updateSelected(e) {
  if (e?.target?.id === "store-all") {
    document.querySelectorAll("#store-list input[data-i]").forEach((c) => (c.checked = e.target.checked));
  }
  const sel = selected();
  const btn = $("store-clean");
  if (!btn.disabled) btn.textContent = sel.length ? `Borrar ${sel.length} (${fmtSize(sel.reduce((s, p) => s + p.size, 0))})` : "Borrar seleccionados";
  btn.disabled = !sel.length;
}

async function cleanStore() {
  const sel = selected();
  if (!sel.length) return;
  const size = fmtSize(sel.reduce((s, p) => s + p.size, 0));
  const ok = await confirmDialog(
    "¿Borrar drivers obsoletos?",
    `<p>Se borrarán <b>${sel.length}</b> paquetes viejos (${size}) con la herramienta oficial de Windows.</p>
     <p>Los drivers que estás usando no se tocan. Después no podrás «revertir al driver anterior» desde el
     Administrador de dispositivos.</p>`,
    "Borrar",
    { danger: true, html: true }
  );
  if (!ok) return;
  try {
    const r = await invoke("drivers_store_delete", { items: sel });
    toast(
      `Liberados ${fmtSize(r.freed)} · ${r.deleted.length} borrados` +
        (r.failed.length ? ` · ${r.failed.length} no se pudieron borrar (en uso)` : ""),
      r.failed.length > 0 && !r.deleted.length
    );
  } catch (e) {
    toast(String(e), true);
  }
  await scanStore();
}

async function loadLeftovers() {
  try {
    const items = await invoke("cleaner_scan");
    leftovers = items.find((i) => i.id === "driver_leftovers");
    $("st-left").textContent = leftovers ? fmtSize(leftovers.size) : "–";
    $("left-card").classList.toggle("hidden", !leftovers || leftovers.size === 0);
  } catch (e) {
    console.error(e);
  }
}

async function cleanLeftovers() {
  if (!app.admin) return toast("Necesitas permisos de administrador.", true);
  try {
    const r = await invoke("cleaner_run", { ids: ["driver_leftovers"] });
    toast(`Liberados ${fmtSize(r.freed)}`);
  } catch (e) {
    toast(String(e), true);
  }
  loadLeftovers();
}
