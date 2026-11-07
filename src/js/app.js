// Punto de entrada: navegación, estado de administrador y enlaces del autor.

import { $, app, invoke, toast, openTarget } from "./util.js";
import { AUTHOR, LINKS } from "./config.js";
import * as monitor from "./pages/monitor.js";
import * as ram from "./pages/ram.js";
import * as tweaks from "./pages/tweaks.js";
import * as browsers from "./pages/browsers.js";
import * as startup from "./pages/startup.js";
import * as power from "./pages/power.js";
import * as disk from "./pages/disk.js";
import * as cleaner from "./pages/cleaner.js";
import * as drivers from "./pages/drivers.js";
import * as tools from "./pages/tools.js";
import * as health from "./pages/health.js";
import * as history from "./pages/history.js";

const pages = { monitor, ram, tweaks, browsers, startup, power, disk, cleaner, drivers, tools, health, history };
const started = new Set();
let current = null;

function show(name) {
  if (!pages[name] || name === current) return;
  if (current) pages[current].hide?.();
  current = name;
  document.querySelectorAll(".nav-btn").forEach((b) => b.classList.toggle("active", b.dataset.view === name));
  document.querySelectorAll(".view").forEach((v) => v.classList.toggle("active", v.id === `view-${name}`));
  const el = $(`view-${name}`);
  if (!started.has(name)) {
    started.add(name);
    pages[name].init(el);
  }
  pages[name].show?.();
}

document.querySelectorAll(".nav-btn").forEach((b) => b.addEventListener("click", () => show(b.dataset.view)));

// Botones "?" de ayuda: abren el panel dentro de su tarjeta.
document.addEventListener("click", (e) => {
  const help = e.target.closest(".help-btn");
  if (help) {
    const box = help.closest(".has-help")?.querySelector(".help");
    if (box) {
      box.classList.toggle("hidden");
      help.classList.toggle("open", !box.classList.contains("hidden"));
    }
    return;
  }
  if (e.target.closest("[data-action='relaunch-admin']")) relaunchAdmin();
});

async function relaunchAdmin() {
  try {
    await invoke("relaunch_admin");
  } catch (e) {
    toast(String(e), true);
  }
}

function renderAdminBox() {
  $("admin-box").innerHTML = app.admin
    ? `<span class="admin-ok">● Administrador</span>`
    : `<span class="admin-no">● Sin permisos de admin</span>
       <button class="btn small ghost full" data-action="relaunch-admin">Reiniciar como admin</button>`;
}

// Enlaces del autor
$("author-name").textContent = AUTHOR;
document.querySelectorAll(".foot-link").forEach((b) =>
  b.addEventListener("click", () => {
    const url = LINKS[b.dataset.link];
    if (url) openTarget(url);
    else toast("Agrega tu enlace de GitHub en src/js/config.js", true);
  })
);

async function boot() {
  try {
    const info = await invoke("app_info");
    app.admin = info.admin;
    app.dev = info.dev;
    $("app-version").textContent = `v${info.version}${info.dev ? " · desarrollo" : ""}`;
  } catch (e) {
    console.error(e);
  }
  renderAdminBox();
  ram.startAutoClean();
  show("monitor");
}

boot();
