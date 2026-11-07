// Monitor de rendimiento tipo Administrador de tareas.

import { $, invoke, esc, fmtSize, fmtRate, fmtPct, fmtNum, toast, confirmDialog, helpBtn } from "../util.js";
import { LineChart } from "../chart.js";

const RES = {
  cpu: { label: "CPU", color: "#d19a3c" },
  ram: { label: "Memoria", color: "#b98acb" },
  gpu: { label: "GPU", color: "#86a35e" },
  disk: { label: "Disco", color: "#5fa8a0" },
  net: { label: "Red", color: "#c8553d" },
};

let selected = "cpu";
let timer = null;
let last = null;
let sortKey = "mem";
let filter = "";
const minis = {};
let big = null;

export function init(el) {
  el.innerHTML = `
    <header class="view-head">
      <div>
        <h1>Monitor</h1>
        <p class="muted" id="mon-sub">Rendimiento en tiempo real</p>
      </div>
    </header>

    <div class="monitor">
      <div class="res-list" id="res-list">
        ${Object.entries(RES)
          .map(([k, r]) => `
          <button class="res-tile ${k === selected ? "active" : ""}" data-res="${k}">
            <div class="res-info">
              <span class="res-name">${r.label}</span>
              <span class="res-val" id="val-${k}">–</span>
            </div>
            <canvas class="mini" id="mini-${k}"></canvas>
          </button>`)
          .join("")}
      </div>

      <div class="card res-detail">
        <div class="res-detail-head">
          <h2 id="det-title">CPU</h2>
          <span class="muted" id="det-sub"></span>
        </div>
        <div class="big-chart-wrap"><canvas id="big-chart"></canvas><span class="axis-max" id="axis-max"></span></div>
        <div class="stats" id="det-stats"></div>
        <div class="cores hidden" id="cores"></div>
      </div>
    </div>

    <div class="proc-head has-help">
      <h2>Procesos <span class="muted" id="proc-count"></span></h2>
      ${helpBtn()}
      <input id="proc-search" class="search" type="text" placeholder="Buscar proceso…" />
      <div class="help hidden">
        <p><b>Qué ves aquí:</b> los programas que se están ejecutando y cuántos recursos usa cada uno.
        CPU y GPU en % del total; Memoria es la RAM que usa ahora mismo.</p>
        <p><b>Finalizar tarea:</b> cierra el programa a la fuerza. Se pierde lo que no hayas guardado. No cierres
        procesos de Windows que no conozcas (svchost, csrss, dwm…).</p>
      </div>
    </div>
    <div class="table-wrap">
      <table class="proc-table">
        <thead>
          <tr>
            <th data-sort="name">Nombre</th>
            <th data-sort="cpu" class="num">CPU</th>
            <th data-sort="mem" class="num">Memoria</th>
            <th data-sort="gpu" class="num">GPU</th>
            <th data-sort="pid" class="num">PID</th>
            <th></th>
          </tr>
        </thead>
        <tbody id="proc-body"></tbody>
      </table>
    </div>`;

  for (const [k, r] of Object.entries(RES)) {
    const series = k === "disk" || k === "net" ? [{ color: r.color }, { color: "#efe8d6", fill: false }] : [{ color: r.color }];
    minis[k] = new LineChart($(`mini-${k}`), { max: k === "disk" || k === "net" ? null : 100, series, grid: false });
  }
  setBig();

  $("res-list").addEventListener("click", (e) => {
    const t = e.target.closest(".res-tile");
    if (!t) return;
    selected = t.dataset.res;
    document.querySelectorAll(".res-tile").forEach((x) => x.classList.toggle("active", x === t));
    setBig();
    if (last) render(last);
  });

  el.querySelectorAll("th[data-sort]").forEach((th) =>
    th.addEventListener("click", () => {
      sortKey = th.dataset.sort;
      if (last) renderProcs(last);
    })
  );
  $("proc-search").addEventListener("input", (e) => {
    filter = e.target.value.toLowerCase();
    if (last) renderProcs(last);
  });
  $("proc-body").addEventListener("click", onKill);
}

function setBig() {
  const r = RES[selected];
  const dual = selected === "disk" || selected === "net";
  big = new LineChart($("big-chart"), {
    max: dual ? null : 100,
    series: dual ? [{ color: r.color }, { color: "#efe8d6", fill: false }] : [{ color: r.color }],
  });
  // Rellenamos con el historial de la mini-gráfica para no empezar en blanco
  big.data = minis[selected].data.map((d) => [...d]);
  big.draw();
  $("det-title").textContent = r.label;
  $("cores").classList.toggle("hidden", selected !== "cpu");
}

export function show() {
  tick();
  timer = setInterval(tick, 1000);
}

export function hide() {
  clearInterval(timer);
}

async function tick() {
  try {
    const s = await invoke("perf_snapshot");
    last = s;
    const ramPct = s.ram_total ? (s.ram_used / s.ram_total) * 100 : 0;
    minis.cpu.push(s.cpu);
    minis.ram.push(ramPct);
    minis.gpu.push(s.gpu);
    // disco y red muestran dos líneas: lectura/escritura y bajada/subida
    minis.disk.push(s.disk_read, s.disk_write);
    minis.net.push(s.net_rx, s.net_tx);

    const map = { cpu: [s.cpu], ram: [ramPct], gpu: [s.gpu], disk: [s.disk_read, s.disk_write], net: [s.net_rx, s.net_tx] };
    big.push(...map[selected]);
    render(s);
  } catch (e) {
    console.error(e);
  }
}

function render(s) {
  const ramPct = s.ram_total ? (s.ram_used / s.ram_total) * 100 : 0;
  $("mon-sub").textContent = s.cpu_name;
  $("val-cpu").textContent = `${fmtPct(s.cpu)}${s.cpu_mhz ? ` · ${(s.cpu_mhz / 1000).toFixed(2)} GHz` : ""}`;
  $("val-ram").textContent = `${fmtSize(s.ram_used)} / ${fmtSize(s.ram_total)} (${fmtPct(ramPct)})`;
  $("val-gpu").textContent = `${fmtPct(s.gpu)}${s.gpu_mem ? ` · ${fmtSize(s.gpu_mem)}` : ""}`;
  $("val-disk").textContent = `${fmtPct(s.disk_active)} · ${fmtRate(s.disk_read + s.disk_write)}`;
  $("val-net").textContent = `↓ ${fmtRate(s.net_rx)} ↑ ${fmtRate(s.net_tx)}`;

  const stat = (label, value) => `<div class="stat"><span class="muted">${label}</span><strong>${value}</strong></div>`;
  let stats = "";
  let sub = "";
  if (selected === "cpu") {
    sub = s.cpu_name;
    stats = stat("Uso", fmtPct(s.cpu)) + stat("Velocidad", s.cpu_mhz ? `${(s.cpu_mhz / 1000).toFixed(2)} GHz` : "–") +
      stat("Procesos", fmtNum(s.proc_count)) + stat("Hilos lógicos", s.cores.length) + stat("Encendida", fmtUptime(s.uptime));
    $("cores").innerHTML = s.cores
      .map((c, i) => `<div class="core" title="Núcleo ${i}: ${c.toFixed(0)}%"><i style="height:${Math.min(100, c)}%"></i><span>${i}</span></div>`)
      .join("");
  } else if (selected === "ram") {
    sub = `${fmtSize(s.ram_total)} instalada`;
    stats = stat("En uso", fmtSize(s.ram_used)) + stat("Disponible", fmtSize(s.ram_total - s.ram_used)) + stat("Uso", fmtPct(ramPct)) +
      stat("Archivo de paginación", `${fmtSize(s.swap_used)} / ${fmtSize(s.swap_total)}`);
  } else if (selected === "gpu") {
    sub = "Uso del motor más ocupado (como el Administrador de tareas)";
    stats = stat("Uso", fmtPct(s.gpu)) + stat("Memoria de video dedicada", fmtSize(s.gpu_mem)) + stat("Memoria compartida", fmtSize(s.gpu_shared));
  } else if (selected === "disk") {
    sub = "Todos los discos · naranja: lectura, blanco: escritura";
    stats = stat("Tiempo activo", fmtPct(s.disk_active)) + stat("Lectura", fmtRate(s.disk_read)) + stat("Escritura", fmtRate(s.disk_write));
  } else {
    sub = "Todas las conexiones · rojo: bajada, blanco: subida";
    stats = stat("Recibiendo", fmtRate(s.net_rx)) + stat("Enviando", fmtRate(s.net_tx));
  }
  $("det-sub").textContent = sub;
  $("det-stats").innerHTML = stats;
  const max = big.currentMax ?? 100;
  $("axis-max").textContent = selected === "disk" || selected === "net" ? fmtRate(max) : "100%";
  renderProcs(s);
}

function fmtUptime(sec) {
  const d = Math.floor(sec / 86400);
  const h = Math.floor((sec % 86400) / 3600);
  const m = Math.floor((sec % 3600) / 60);
  return d ? `${d} d ${h} h` : `${h} h ${m} min`;
}

function renderProcs(s) {
  let list = s.processes;
  if (filter) list = list.filter((p) => p.name.toLowerCase().includes(filter) || String(p.pid) === filter);
  list = [...list].sort((a, b) => (sortKey === "name" ? a.name.localeCompare(b.name) : (b[sortKey] ?? 0) - (a[sortKey] ?? 0)));
  $("proc-count").textContent = `(${fmtNum(s.proc_count)})`;
  const heat = (v, max) => `style="--heat:${Math.min(1, v / max).toFixed(2)}"`;
  $("proc-body").innerHTML = list
    .slice(0, 150)
    .map((p) => `
      <tr>
        <td class="pname" title="${esc(p.name)}">${esc(p.name)}</td>
        <td class="num heat" ${heat(p.cpu, 50)}>${p.cpu.toFixed(1)}%</td>
        <td class="num heat" ${heat(p.mem, 2e9)}>${fmtSize(p.mem)}</td>
        <td class="num heat" ${heat(p.gpu, 50)}>${p.gpu ? p.gpu.toFixed(1) + "%" : "–"}</td>
        <td class="num muted">${p.pid}</td>
        <td><button class="btn tiny ghost" data-kill="${p.pid}" data-name="${esc(p.name)}">Finalizar</button></td>
      </tr>`)
    .join("");
  document.querySelectorAll("th[data-sort]").forEach((th) => th.classList.toggle("sorted", th.dataset.sort === sortKey));
}

async function onKill(e) {
  const b = e.target.closest("button[data-kill]");
  if (!b) return;
  const ok = await confirmDialog(
    "¿Finalizar tarea?",
    `Se cerrará "${b.dataset.name}" a la fuerza. Si tiene algo sin guardar, se perderá.`,
    "Finalizar",
    { danger: true }
  );
  if (!ok) return;
  try {
    await invoke("kill_process", { pid: Number(b.dataset.kill), name: b.dataset.name });
    toast(`${b.dataset.name} finalizado`);
  } catch (err) {
    toast(String(err), true);
  }
}
