// The colony explorer (src/lab/colony_explorer.rs writes it with data.js beside it).
// Views: Days (the reader: the camp's days, its people, its works), Activity (who did what
// when, and why), Needs (needs and stocks over time), Map (the camp replayed), Bonds (what
// settlers think of each other), Works (the works rising, the things made), Checks, Compare.
// No outside libraries: SVG and canvas by hand, so the folder opens from disk.
"use strict";

const C = window.COLONY, B = window.COLONY_B;
const M = C.meta, DAY = M.day_ticks;
const $ = (s, el = document) => el.querySelector(s);
const esc = s => String(s ?? "").replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const css = name => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
const clip = (s, n) => s.length > n ? s.slice(0, n - 1) + "…" : s;
window.addEventListener("error", e => { document.body.dataset.error = (document.body.dataset.error || "") + e.message + " @" + e.lineno + "; "; });

// ---------------------------------------------------------------------------------------------
// Time

const pad2 = n => String(n).padStart(2, "0");
const dayOf = t => Math.floor(t / DAY) + 1;
const clock = t => `${pad2(Math.floor((t % DAY) / 60))}:${pad2(t % 60)}`;
const stamp = t => `Day ${dayOf(t)}, ${clock(t)}`;
const dur = t => t >= DAY ? `${Math.floor(t / DAY)}d ${Math.floor((t % DAY) / 60)}h` : t >= 60 ? `${Math.floor(t / 60)}h ${pad2(t % 60)}m` : `${t}m`;
const T0 = M.start, T1 = M.now;
const FIRST_DAY = dayOf(T0), LAST_DAY = dayOf(Math.max(T0, T1 - 1));
// Index of the last tick <= t in a sorted array (or 0).
function at(ticks, t) {
  let lo = 0, hi = ticks.length - 1;
  if (hi < 0 || t < ticks[0]) return 0;
  while (lo < hi) { const m = (lo + hi + 1) >> 1; if (ticks[m] <= t) lo = m; else hi = m - 1; }
  return lo;
}

// ---------------------------------------------------------------------------------------------
// Data

function decode(C) {
  const spans = C.spans.map(a => ({ who: a[0], t0: a[1], t1: a[2], v: a[3], tx: a[4], ty: a[5], why: C.whys[a[6]] }));
  const byWho = C.settlers.map(() => []);
  for (const s of spans) byWho[s.who].push(s);
  const series = {};
  C.series.fields.forEach((f, k) => series[f] = C.series.rows.map(r => r[k]));
  const log = C.log.map(a => ({ t: a[0], text: a[1] }));
  const moments = C.moments.map(a => ({ t: a[0], title: a[1], text: a[2], because: a[3], x: a[4], y: a[5], major: a[6] }));
  // Each settler's time with the camp: [from, to) intervals while alive.
  const life = C.settlers.map(() => []);
  const open = C.settlers.map(() => null);
  for (const [t, i, alive] of C.status) {
    if (alive && open[i] == null) open[i] = t;
    if (!alive && open[i] != null) { life[i].push([open[i], t]); open[i] = null; }
  }
  open.forEach((t, i) => { if (t != null) life[i].push([t, C.meta.now + 1]); });
  const creatureKind = new Map(C.creature_names.map(c => [c[0], { kind: c[1], name: c[2] }]));
  return { C, spans, byWho, series, log, moments, life, creatureKind };
}
const D = decode(C);
const DB = B ? decode(B) : null;
const S = C.settlers;
const VERBS = C.verbs;
const aliveAt = (d, i, t) => d.life[i].some(([a, b]) => t >= a && t < b);
function spanAt(d, i, t) {
  const L = d.byWho[i];
  let lo = 0, hi = L.length - 1, k = -1;
  while (lo <= hi) { const m = (lo + hi) >> 1; if (L[m].t0 <= t) { k = m; lo = m + 1; } else hi = m - 1; }
  return k >= 0 && t < Math.max(L[k].t1, L[k].t0 + 1) ? L[k] : null;
}
function posAt(d, k, i) { const r = d.C.pos.rows[k]; if (!r || i * 4 + 3 >= r.length) return null; return { x: r[i * 4], y: r[i * 4 + 1], z: r[i * 4 + 2], f: r[i * 4 + 3] }; }
function needsAt(d, k, i) { const r = d.C.needs.rows[k]; if (!r || i * 6 + 5 >= r.length) return null; return { hunger: r[i * 6], fatigue: r[i * 6 + 1], exposure: r[i * 6 + 2], stress: r[i * 6 + 3], focus: r[i * 6 + 4], stuck: r[i * 6 + 5] }; }

const vcol = v => css("--v-" + v);
const PAL = [];
function personColor(i) {
  const dark = document.documentElement.dataset.theme === "dark" || (!document.documentElement.dataset.theme && matchMedia("(prefers-color-scheme: dark)").matches);
  return `hsl(${(i * 137.508 + 20) % 360} ${dark ? 55 : 50}% ${dark ? 66 : 40}%)`;
}
const isHostile = k => /Beast|Raider|Wolf|CaveHunter|Besieger|Scout|Prowler/.test(k);

// Settlers' names in text become links (longest first, every time).
const NAMES = S.map((s, i) => [s.name, i]).filter(n => n[0].length > 2).sort((a, b) => b[0].length - a[0].length);
function linkNames(text) {
  let out = esc(text);
  const slots = [];
  for (const [n, i] of NAMES) {
    const en = esc(n);
    if (!out.includes(en)) continue;
    slots.push(`<a href="#/s/${i}">${en}</a>`);
    out = out.split(en).join(`\u0001${slots.length - 1}\u0002`);
  }
  return out.replace(/\u0001(\d+)\u0002/g, (_, k) => slots[+k]);
}
const who = i => `<a href="#/s/${i}">${esc(S[i]?.name ?? "?")}</a>`;
const dayLink = (t, text) => `<a href="#/day/${dayOf(t)}">${esc(text ?? stamp(t))}</a>`;
const histEvent = id => M.history && id != null ? `${M.history}#/event/${id}` : null;
const histPage = code => M.history && code ? `${M.history}#/page/${code}` : null;

// ---------------------------------------------------------------------------------------------
// Shared pieces: drawer, tooltip, canvas, SVG

const view = $("#view");
let cleanup = null;
function openDrawer(html) {
  const d = $("#drawer");
  $("#drawer-body").innerHTML = html;
  d.hidden = false;
  d.scrollTop = 0;
  drawSparks($("#drawer-body"));
}
$("#drawer-close").onclick = () => { $("#drawer").hidden = true; };
const tip = $("#tip");
function showTip(html, x, y) {
  tip.innerHTML = html; tip.hidden = false;
  const w = tip.offsetWidth, h = tip.offsetHeight;
  tip.style.left = Math.min(x + 14, innerWidth - w - 8) + "px";
  tip.style.top = (y + h + 18 > innerHeight ? y - h - 10 : y + 14) + "px";
}
function closeTip() { tip.hidden = true; }
function fitCanvas(cv) {
  const r = cv.getBoundingClientRect(), dpr = devicePixelRatio || 1;
  cv.width = Math.max(1, Math.round(r.width * dpr)); cv.height = Math.max(1, Math.round(r.height * dpr));
  const g = cv.getContext("2d");
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  return { g, w: r.width, h: r.height };
}
const svgEl = (tag, attrs = {}, text) => {
  const el = document.createElementNS("http://www.w3.org/2000/svg", tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
  if (text != null) el.textContent = text;
  return el;
};
function niceStep(pxPerUnit, minPx) { for (const s of [1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000]) if (s * pxPerUnit >= minPx) return s; return 2000; }

// A small line chart in a canvas (class "spark" with data-kind and data-i).
function drawSparks(root) {
  for (const cv of root.querySelectorAll("canvas.spark")) {
    const kind = cv.dataset.kind;
    if (kind === "needs") {
      const i = +cv.dataset.i;
      const xs = C.needs.ticks;
      const keys = [["hunger", 0, "--c-war"], ["fatigue", 1, "--c-trade"], ["exposure", 2, "--c-culture"], ["stress", 3, "--c-beast"]];
      lineChart(cv, xs, keys.map(([n, k, col]) => ({ name: n, color: css(col), ys: C.needs.rows.map(r => r.length > i * 6 + k ? r[i * 6 + k] : null) })), { min: -50, max: 150, unit: "%", marks: deathMarks() });
    } else if (kind === "pair") {
      const a = +cv.dataset.a, b = +cv.dataset.b;
      const xs = C.opinions.map(o => o[0]);
      const ys = C.opinions.map(o => { const f = o[1]; for (let k = 0; k < f.length; k += 3) if (f[k] === Math.min(a, b) && f[k + 1] === Math.max(a, b)) return f[k + 2]; return 0; });
      lineChart(cv, xs, [{ name: "opinion", color: css("--rubric"), ys }], { zero: true });
    } else if (kind === "work") {
      const w = C.works[+cv.dataset.k];
      const xs = [w.first, ...w.steps.map(s => s[0]), w.done ?? w.gone ?? T1];
      const ys = [0, ...w.steps.map(s => s[1]), w.used];
      lineChart(cv, xs, [{ name: "loads laid", color: css("--c-town"), ys, step: true }], { min: 0, max: Math.max(1, w.needed) });
    }
  }
}
function deathMarks() { return D.moments.filter(m => /death|dies|died|fell|the last/i.test(m.title)).map(m => m.t); }

// A line chart: xs ticks, series [{name, color, ys, step, width}], opts {min, max, unit, marks, zero, onClick}.
function lineChart(cv, xs, series, opts = {}) {
  const draw = () => {
    const { g, w, h } = fitCanvas(cv);
    const L = 34, R = 8, Tp = 8, Bt = 18;
    const x0 = opts.t0 ?? (xs[0] ?? 0), x1 = opts.t1 ?? (xs[xs.length - 1] ?? 1);
    let vmin = Infinity, vmax = -Infinity;
    for (const s of series) for (const v of s.ys) if (v != null) { if (v < vmin) vmin = v; if (v > vmax) vmax = v; }
    if (vmin === Infinity) { vmin = 0; vmax = 1; }
    const lo = opts.min != null ? Math.min(opts.min, vmin) : Math.min(0, vmin);
    const hi = opts.max != null ? Math.max(opts.max, vmax) : Math.max(lo + 1, vmax);
    const X = t => L + (t - x0) / Math.max(1, x1 - x0) * (w - L - R);
    const Y = v => Tp + (1 - (v - lo) / Math.max(1e-9, hi - lo)) * (h - Tp - Bt);
    g.clearRect(0, 0, w, h);
    g.font = `10px ${css("--label")}`; g.fillStyle = css("--ink-faint"); g.strokeStyle = css("--rule"); g.lineWidth = 1;
    // Days along the bottom.
    const pxDay = (w - L - R) / Math.max(1, (x1 - x0) / DAY);
    const st = niceStep(pxDay, 46);
    for (let d = Math.ceil(x0 / DAY / st) * st; d * DAY <= x1; d += st) { const x = X(d * DAY); g.globalAlpha = 0.5; g.beginPath(); g.moveTo(x, Tp); g.lineTo(x, h - Bt); g.stroke(); g.globalAlpha = 1; g.fillText(`day ${d + 1}`, x + 2, h - 5); }
    // Values on the left.
    for (const v of [lo, (lo + hi) / 2, hi]) g.fillText(Math.round(v * 10) / 10 + (opts.unit || ""), 2, Y(v) + 3);
    if (opts.zero && lo < 0 && hi > 0) { g.strokeStyle = css("--ink-faint"); g.beginPath(); g.moveTo(L, Y(0)); g.lineTo(w - R, Y(0)); g.stroke(); }
    for (const t of opts.marks || []) { const x = X(t); g.strokeStyle = alphaOf(css("--rubric"), 0.45); g.beginPath(); g.moveTo(x, Tp); g.lineTo(x, h - Bt); g.stroke(); }
    for (const s of series) {
      g.strokeStyle = s.color; g.lineWidth = s.width || 1.4; g.globalAlpha = s.faint ? 0.25 : 1;
      g.beginPath();
      let started = false, py = 0;
      s.ys.forEach((v, k) => {
        if (v == null) { started = false; return; }
        const x = X(xs[k]), y = Y(v);
        if (!started) { g.moveTo(x, y); started = true; } else if (s.step) { g.lineTo(x, py); g.lineTo(x, y); } else g.lineTo(x, y);
        py = y;
      });
      g.stroke(); g.globalAlpha = 1;
    }
    cv._chart = { X, Y, x0, x1, L, R, w };
  };
  draw();
  cv.onmousemove = ev => {
    const c = cv._chart; if (!c) return;
    const r = cv.getBoundingClientRect();
    const t = c.x0 + (ev.clientX - r.left - c.L) / (c.w - c.L - c.R) * (c.x1 - c.x0);
    const k = at(xs, t);
    const rows = series.filter(s => !s.faint && s.ys[k] != null).slice(0, 14).map(s => `<span class="sw" style="display:inline-block;width:.6rem;height:.6rem;border-radius:50%;background:${s.color}"></span> ${esc(s.name)}: <b>${Math.round(s.ys[k] * 100) / 100}${opts.unit || ""}</b>`);
    showTip(`<b>${stamp(xs[k] ?? 0)}</b><br>${rows.join("<br>")}`, ev.clientX, ev.clientY);
  };
  cv.onmouseleave = closeTip;
  if (opts.onClick) cv.onclick = ev => { const c = cv._chart; const r = cv.getBoundingClientRect(); opts.onClick(c.x0 + (ev.clientX - r.left - c.L) / (c.w - c.L - c.R) * (c.x1 - c.x0)); };
  return draw;
}
const alphaOf = (col, a) => {
  const m = col.match(/^#?([0-9a-f]{6})$/i);
  if (m) { const n = parseInt(m[1], 16); return `rgba(${n >> 16},${(n >> 8) & 255},${n & 255},${a})`; }
  return col;
};

// ---------------------------------------------------------------------------------------------
// Router

function route() {
  if (cleanup) { cleanup(); cleanup = null; }
  closeTip();
  const [path, query] = location.hash.replace(/^#\/?/, "").split("?");
  const parts = path.split("/").filter(Boolean);
  const q = Object.fromEntries(new URLSearchParams(query || ""));
  const tab = { activity: "activity", needs: "needs", map: "map", bonds: "bonds", works: "works", checks: "checks", compare: "compare" }[parts[0]] || "read";
  for (const a of document.querySelectorAll("#tabs a")) a.classList.toggle("on", a.dataset.tab === tab);
  if (tab === "read") $("#drawer").hidden = true;
  switch (parts[0]) {
    case "activity": return activityView(q);
    case "needs": return needsView(q);
    case "map": return mapView(q);
    case "bonds": return bondsView(q);
    case "works": return worksView(q);
    case "checks": return checksView(q);
    case "compare": return compareView(q);
    default: return readView(parts, q);
  }
}
window.addEventListener("hashchange", route);
$("#theme").onclick = () => {
  const root = document.documentElement;
  const dark = root.dataset.theme ? root.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
  root.dataset.theme = dark ? "light" : "dark";
  try { localStorage.setItem("annals-theme", root.dataset.theme); } catch (_) {}
  route();
};
try { const t = localStorage.getItem("annals-theme"); if (t) document.documentElement.dataset.theme = t; } catch (_) {}
{ const t = new URLSearchParams(location.search).get("theme"); if (t === "light" || t === "dark") document.documentElement.dataset.theme = t; }
$("#home").textContent = M.name;
$("#outlinks").innerHTML = [M.annals ? `<a href="${esc(M.annals)}" title="The game's own annals of the camp">Annals</a>` : "", M.history ? `<a href="${esc(M.history)}" title="The world's history explorer">History</a>` : ""].join("");
if (B) $("#tab-compare").hidden = false;

// Search: settlers, works, moments and the log.
{
  const q = $("#q"), hits = $("#hits");
  let sel = -1;
  q.addEventListener("input", () => {
    const s = q.value.trim().toLowerCase();
    sel = -1;
    if (s.length < 2) { hits.innerHTML = ""; return; }
    const out = [];
    S.forEach((x, i) => { if (x.name.toLowerCase().includes(s)) out.push(`<li><a href="#/s/${i}">${esc(x.name)}<span>settler</span></a></li>`); });
    C.works.forEach((w, k) => { if (w.word.toLowerCase().includes(s)) out.push(`<li><a href="#/work/${k}">${esc(w.word)}<span>day ${w.day}</span></a></li>`); });
    D.moments.filter(m => m.title.toLowerCase().includes(s)).slice(0, 8).forEach(m => out.push(`<li><a href="#/day/${dayOf(m.t)}">${esc(m.title)}<span>${stamp(m.t)}</span></a></li>`));
    D.log.filter(l => l.text.toLowerCase().includes(s)).slice(0, 12).forEach(l => out.push(`<li><a href="#/day/${dayOf(l.t)}">${esc(clip(l.text, 70))}<span>${stamp(l.t)}</span></a></li>`));
    hits.innerHTML = out.slice(0, 30).join("");
  });
  q.addEventListener("keydown", ev => {
    const items = [...hits.querySelectorAll("a")];
    if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
      ev.preventDefault();
      sel = (sel + (ev.key === "ArrowDown" ? 1 : -1) + items.length) % Math.max(1, items.length);
      items.forEach((a, i) => a.classList.toggle("sel", i === sel));
    } else if (ev.key === "Enter" && items.length) { location.hash = items[Math.max(0, sel)].getAttribute("href"); hits.innerHTML = ""; q.blur(); }
    else if (ev.key === "Escape") { hits.innerHTML = ""; q.blur(); }
  });
  hits.addEventListener("click", () => { hits.innerHTML = ""; });
  document.addEventListener("click", ev => { if (!ev.target.closest(".find")) hits.innerHTML = ""; });
}

// ---------------------------------------------------------------------------------------------
// The reader

let indexCat = "days";
function readView(parts, q) {
  view.innerHTML = `<div class="split"><nav class="index" id="index"></nav><article class="reader"><div class="page" id="page"></div></article></div>`;
  drawIndex();
  const kind = parts[0] || "home";
  const page = $("#page");
  page.innerHTML = kind === "day" ? dayPage(+parts[1]) : kind === "s" ? settlerPage(+parts[1]) : kind === "work" ? workPage(+parts[1]) : homePage();
  drawSparks(page);
  for (const cv of page.querySelectorAll("canvas.strip")) dayStrip(cv, +cv.dataset.day);
  $(".reader").scrollTop = 0;
}

function drawIndex() {
  const el = $("#index");
  const cats = [["days", "Days"], ["people", "People"], ["works", "Works"], ["made", "Made"]];
  let html = `<div class="cats">${cats.map(([k, n]) => `<button data-c="${k}" class="${k === indexCat ? "on" : ""}">${n}</button>`).join("")}</div><ol>`;
  if (indexCat === "days") {
    for (let d = FIRST_DAY; d <= LAST_DAY; d++) {
      const lines = D.log.filter(l => dayOf(l.t) === d).length;
      const big = D.moments.filter(m => m.major && dayOf(m.t) === d);
      html += `<li><a href="#/day/${d}">${big.length ? `<b>Day ${d}</b>` : `Day ${d}`}</a><span>${big.length ? esc(clip(big[0].title, 22)) : lines + " lines"}</span></li>`;
    }
  } else if (indexCat === "people") {
    S.forEach((s, i) => { html += `<li><a href="#/s/${i}">${esc(s.name)}</a><span>${esc(clip(s.fate, 24))}</span></li>`; });
  } else if (indexCat === "works") {
    C.works.forEach((w, k) => { html += `<li><a href="#/work/${k}">${esc(w.word)}</a><span>${w.done != null ? "day " + dayOf(w.done) : w.gone != null ? "set aside" : w.used + "/" + w.needed}</span></li>`; });
  } else {
    C.made.forEach(m => { html += `<li><span style="color:var(--ink)">${esc(clip(m.kind, 26))}</span><span>${esc(S[m.maker]?.name ?? "")}, day ${m.day}</span></li>`; });
  }
  el.innerHTML = html + `</ol>`;
  el.querySelector(".cats").onclick = ev => { const b = ev.target.closest("button"); if (!b) return; indexCat = b.dataset.c; drawIndex(); };
}

function card(n, l, bad) { return `<div class="card${bad ? " bad" : ""}"><div class="n">${n}</div><div class="l">${l}</div></div>`; }

function homePage() {
  const deaths = S.filter(s => /died|killed|slain|starv|lost|carried off/i.test(s.fate) && !s.alive).length;
  const alive = S.filter(s => s.alive).length;
  const done = C.works.filter(w => w.done != null).length;
  let h = `<p class="eyebrow">a camp on world tile ${M.tile[0]},${M.tile[1]}${M.code ? " · " + esc(M.code) : ""}</p><h1>${esc(M.name)}</h1>`;
  h += `<p class="lede">${linkNames(D.log[0]?.text ?? "")}</p>`;
  h += `<div class="cards">${card(LAST_DAY - FIRST_DAY + 1, "days lived")}${card(`${alive} of ${S.length}`, "alive now")}${card(deaths, "dead or lost", deaths > 0)}${card(done, "works standing")}${card(C.made.length, "things made")}${card(D.moments.filter(m => m.major).length, "great moments")}${card(D.spans.length, "decisions recorded")}</div>`;
  h += `<p class="muted">Recorded to ${stamp(T1)}: ${M.ticks.toLocaleString()} minutes in ${M.secs} s, positions every ${M.every} minutes. Colony hash ${esc(M.hash)}${M.myth ? " (myth on)" : ""}.${M.script.length ? ` The patron acted ${M.script.length} times.` : ""}</p>`;
  if (M.standing.length || M.plan) h += `<h2>Now</h2>${M.standing.map(s => `<p>${linkNames(s)}</p>`).join("")}${M.plan ? `<p><i>${linkNames(M.plan)}</i></p>` : ""}<p class="muted">Readiness ${Math.round(M.readiness[0] * 100) / 100}: ${esc(M.readiness[1])}</p>`;
  if (C.arc) {
    const a = C.arc;
    h += `<h2>The troubles</h2><p><b>${esc(a.threat)}</b> (${esc(a.kind)}): ${linkNames(a.why)}${histEvent(a.cause) ? ` <a class="why" href="${histEvent(a.cause)}">in the history</a>` : ""}</p>`;
    h += `<p class="muted">Rumour on day ${a.rumour_day}, refugees on day ${a.refugee_day}, the raid on day ${a.raid_day}; chapter ${a.chapter}.${a.later.length ? " Still to come: " + a.later.map(l => esc(l[0])).join("; ") + "." : ""}</p>`;
    h += `<div class="chain">${a.events.map(e => `<div><a href="#/day/${e[0]}">Day ${e[0]}</a>: <b>${esc(e[1])}</b>. ${linkNames(e[2])} <span class="because">(${esc(e[3])})</span>${histEvent(e[4]) ? ` <a class="why" href="${histEvent(e[4])}">history</a>` : ""}</div>`).join("")}</div>`;
  }
  h += `<h2>Great moments</h2><div class="log">${D.moments.filter(m => m.major).map(momentLine).join("") || `<p class="muted">None yet.</p>`}</div>`;
  if (C.regards.length) h += `<h2>The camp and the world</h2>${C.regards.map(r => `<p><b>${histPage(r.code) ? `<a href="${histPage(r.code)}">${esc(r.people)}</a>` : esc(r.people)}</b> <span class="${r.sum >= 0 ? "good" : "bad"}">${r.sum > 0 ? "+" : ""}${r.sum}</span>: ${r.causes.map(c => `${esc(c[2])} (${c[1] > 0 ? "+" : ""}${c[1]}, day ${c[0]})`).join("; ")}</p>`).join("")}`;
  h += `<h2>The company</h2><table class="list"><tr><th>Name</th><th>Age</th><th>Calling</th><th>Fate</th></tr>${S.map((s, i) => `<tr><td>${who(i)}</td><td>${s.age}</td><td>${esc(s.calling ?? s.guest ?? "")}</td><td>${esc(s.fate)}</td></tr>`).join("")}</table>`;
  return h;
}

function momentLine(m) {
  return `<p class="moment ${m.major ? "" : "minor"}"><span class="t"><a href="#/day/${dayOf(m.t)}">${dayOf(m.t)}·${clock(m.t)}</a></span><b>${esc(m.title)}.</b> ${linkNames(m.text)} <span class="because">Because ${linkNames(m.because)}.</span> <a class="why" href="#/map?t=${m.t}&x=${m.x}&y=${m.y}">map</a></p>`;
}
function logLine(l) { return `<p><span class="t">${clock(l.t)}</span>${linkNames(l.text)}</p>`; }

function frameFor(t) {
  const fr = M.frames;
  if (!fr.length) return null;
  let best = fr[0];
  for (const f of fr) if (f[0] <= t) best = f;
  return best;
}

function dayPage(d) {
  const t0 = (d - 1) * DAY, t1 = d * DAY;
  const lines = D.log.filter(l => l.t >= t0 && l.t < t1);
  const moments = D.moments.filter(m => m.t >= t0 && m.t < t1);
  const fr = frameFor(t0 + 12 * 60);
  let h = `<div class="daynav">${d > FIRST_DAY ? `<a href="#/day/${d - 1}">← Day ${d - 1}</a>` : "<span></span>"}<a href="#/activity?from=${d}&to=${d}">this day's activity</a>${d < LAST_DAY ? `<a href="#/day/${d + 1}">Day ${d + 1} →</a>` : "<span></span>"}</div>`;
  h += `<p class="eyebrow">${esc(M.name)}</p><h1>Day ${d}</h1>`;
  const k = at(C.series.ticks, t0 + 6 * 60);
  if (C.series.rows.length) { const r = C.series.rows[k]; h += `<p class="lede">${r[3]} alive · ${r[0]} meals stored (${r[9]} days) · ${r[1]} logs · ${r[2]} stone · ${Math.round(r[7])} °C</p>`; }
  h += `<canvas class="strip" data-day="${d}"></canvas>`;
  if (moments.length) h += `<h2>Moments</h2><div class="log">${moments.map(momentLine).join("")}</div>`;
  h += `<h2>The log</h2><div class="log">${lines.map(logLine).join("") || `<p class="muted">Nothing was written this day.</p>`}</div>`;
  if (fr) h += `<h2>The camp at noon</h2><a href="#/map?t=${t0 + 12 * 60}"><img class="frame" src="${esc(fr[1])}" alt="the embark"></a><p class="muted">The frame of ${stamp(fr[0])}; click for the map at that hour.</p>`;
  return h;
}

// The day's activity: a row per settler.
function dayStrip(cv, d) {
  const t0 = (d - 1) * DAY, t1 = d * DAY;
  const rows = S.map((_, i) => i).filter(i => D.byWho[i].some(s => s.t1 > t0 && s.t0 < t1));
  cv.style.height = Math.max(60, rows.length * 13 + 20) + "px";
  const { g, w, h } = fitCanvas(cv);
  const L = 92, rh = (h - 16) / Math.max(1, rows.length);
  const X = t => L + (t - t0) / DAY * (w - L);
  g.font = `11px ${css("--label")}`;
  rows.forEach((i, r) => {
    g.fillStyle = css("--ink-soft"); g.fillText(clip(S[i].name, 13), 2, 12 + r * rh + rh / 2);
    for (const s of D.byWho[i]) { if (s.t1 <= t0 || s.t0 >= t1) continue; g.fillStyle = vcol(s.v); g.fillRect(X(Math.max(t0, s.t0)), 4 + r * rh, Math.max(1, X(Math.min(t1, s.t1)) - X(Math.max(t0, s.t0))), rh - 2); }
  });
  g.fillStyle = css("--ink-faint");
  for (let hr = 0; hr <= 24; hr += 6) { const x = X(t0 + hr * 60); g.fillText(pad2(hr) + ":00", Math.min(x + 2, w - 32), h - 2); }
  cv.onmousemove = ev => {
    const r = cv.getBoundingClientRect();
    const t = t0 + (ev.clientX - r.left - L) / (w - L) * DAY, row = Math.floor((ev.clientY - r.top - 4) / rh);
    const i = rows[row];
    const s = i != null ? spanAt(D, i, Math.floor(t)) : null;
    if (s) showTip(`<b>${esc(S[i].name)}</b> · ${VERBS[s.v]} · ${clock(s.t0)}–${clock(s.t1)} (${dur(s.t1 - s.t0)})<br>${esc(s.why)}`, ev.clientX, ev.clientY); else closeTip();
  };
  cv.onmouseleave = closeTip;
  cv.onclick = ev => { const r = cv.getBoundingClientRect(); const t = Math.floor(t0 + (ev.clientX - r.left - L) / (w - L) * DAY); const i = rows[Math.floor((ev.clientY - r.top - 4) / rh)]; if (i != null) openDrawer(momentOf(i, t)); };
}

function skillBars(s) {
  const names = ["forage", "fish", "fell", "haul", "build"];
  return `<div class="skill">${names.map((n, k) => `<span>${n}</span><span class="track"><span class="fill" style="width:${s.skill[k]}%"></span></span><span class="muted">${s.skill[k]}</span>`).join("")}</div>`;
}
function timeShare(i, d = D) {
  const tot = new Array(VERBS.length).fill(0);
  for (const s of d.byWho[i]) tot[s.v] += s.t1 - s.t0;
  const all = tot.reduce((a, b) => a + b, 0) || 1;
  return { tot, all };
}
function shareBar(i) {
  const { tot, all } = timeShare(i);
  return `<div class="share">${tot.map((t, v) => t ? `<span style="width:${t / all * 100}%;background:${vcol(v)}" title="${VERBS[v]} ${Math.round(t / all * 100)}%"></span>` : "").join("")}</div><p class="muted">${tot.map((t, v) => [t, v]).filter(x => x[0]).sort((a, b) => b[0] - a[0]).slice(0, 6).map(([t, v]) => `<span class="vkey" style="background:${vcol(v)}"></span>${VERBS[v]} ${Math.round(t / all * 100)}%`).join(" · ")}</p>`;
}

function settlerPage(i) {
  const s = S[i];
  if (!s) return `<p>No such settler.</p>`;
  let h = `<p class="eyebrow">${esc(s.race)}${s.guest ? " · " + esc(s.guest) : ""}</p><h1>${esc(s.name)}</h1>`;
  h += `<p class="lede">${s.age ? `Aged ${s.age}, ` : ""}${esc(s.calling ?? "")}. ${esc(s.fate[0]?.toUpperCase() + s.fate.slice(1))}.</p>`;
  h += `<div class="actions"><a href="#/activity?who=${i}">activity</a><a href="#/needs?who=${i}">needs</a><a href="#/map?who=${i}">on the map</a><a href="#/bonds?who=${i}">bonds</a>${s.people && histPage(s.people) ? `<a href="${histPage(s.people)}">their people</a>` : ""}</div>`;
  h += `<dl class="facts">`;
  if (s.role) h += `<dt>Role</dt><dd>the camp's ${esc(s.role)}</dd>`;
  if (s.office) h += `<dt>Office</dt><dd>${esc(s.office)}</dd>`;
  if (s.spouse != null) h += `<dt>Wed to</dt><dd>${who(s.spouse)}</dd>`;
  if (s.faith) h += `<dt>Worships</dt><dd>${esc(s.faith)}</dd>`;
  if (s.feeling) h += `<dt>Feels</dt><dd>${esc(s.feeling)}</dd>`;
  h += `<dt>Now</dt><dd>${esc(s.job)}: ${linkNames(s.why)}</dd><dt>Stress</dt><dd>${(s.stress / 100).toFixed(2)}${s.breaks ? `, ${s.breaks} breaks` : ""}${s.jaded ? ", jaded" : ""}</dd>`;
  if (s.stuck) h += `<dt>Stuck</dt><dd class="bad">${s.stuck} minutes wanting to move and unable</dd>`;
  h += `</dl>`;
  if (s.lines.length) h += `<h2>Before the camp</h2>${s.lines.map(([t, e]) => `<p>${esc(t)}${histEvent(e) ? ` <a class="why" href="${histEvent(e)}">the event</a>` : ""}</p>`).join("")}`;
  if (s.about.length) h += `<h2>In the camp</h2>${s.about.map(a => `<p>${linkNames(a)}</p>`).join("")}`;
  h += `<h2>Who they are</h2>${s.who.map(p => `<p>${esc(p)}</p>`).join("")}`;
  h += `<h2>Hands</h2>${skillBars(s)}<p class="muted">${s.loads} loads laid${s.drill ? `, drilled ${s.drill}%` : ""}.</p>`;
  h += `<h2>Their time</h2>${shareBar(i)}<canvas class="spark" data-kind="needs" data-i="${i}"></canvas><p class="muted">Hunger <span style="color:var(--c-war)">■</span>, fatigue <span style="color:var(--c-trade)">■</span>, exposure <span style="color:var(--c-culture)">■</span>, stress <span style="color:var(--c-beast)">■</span>, sampled hourly; red lines are deaths.</p>`;
  if (s.wounds.length || s.deeds.length || s.made.length) h += `<h2>Marks of their life</h2>${s.wounds.map(w => `<p>Wound: ${esc(w)}</p>`).join("")}${s.deeds.map(w => `<p>Deed: ${linkNames(w)}</p>`).join("")}${s.made.length ? `<p>Made: ${s.made.map(esc).join("; ")}</p>` : ""}`;
  if (s.thoughts.length || s.scars.length) h += `<h2>Thoughts</h2><div class="thoughts">${[...s.scars.map(x => [...x, true]), ...s.thoughts].sort((a, b) => a[0] - b[0]).map(([t, text, wgt, scar]) => `<p><span class="w ${wgt < 0 ? "bad" : "good"}">${wgt > 0 ? "+" : ""}${(wgt / 100).toFixed(2)}</span>${dayLink(t)} ${linkNames(text)}${scar ? " <i class='muted'>(a scar)</i>" : ""}</p>`).join("")}</div>`;
  if (s.needs.length) h += `<h2>Needs</h2><div class="skill">${s.needs.map(([n, lvl, f]) => `<span>${esc(n)}</span><span class="track"><span class="fill" style="width:${Math.max(0, Math.min(100, (f + 400) / 8))}%;background:${f < -100 ? "var(--rubric)" : "var(--ink-soft)"}"></span></span><span class="muted">${f}</span>`).join("")}</div><p class="muted">Focus per need now (400 met, below 0 unmet), with its strength 1-10.</p>`;
  const mine = D.log.filter(l => l.text.includes(s.name));
  h += `<h2>In the log</h2><div class="log">${mine.slice(-60).map(l => `<p><span class="t"><a href="#/day/${dayOf(l.t)}">${dayOf(l.t)}·${clock(l.t)}</a></span>${linkNames(l.text)}</p>`).join("") || `<p class="muted">Never named.</p>`}</div>${mine.length > 60 ? `<p class="muted">The last 60 of ${mine.length}.</p>` : ""}`;
  const last = D.byWho[i].slice(-20);
  h += `<h2>Their last choices</h2><div class="log">${last.map(sp => `<p><span class="t">${dayOf(sp.t0)}·${clock(sp.t0)}</span><span class="vkey" style="background:${vcol(sp.v)}"></span>${VERBS[sp.v]} for ${dur(sp.t1 - sp.t0)}: ${linkNames(sp.why)}</p>`).join("")}</div>`;
  return h;
}

function workPage(k) {
  const w = C.works[k];
  if (!w) return `<p>No such work.</p>`;
  let h = `<p class="eyebrow">${w.dig ? "a dig" : "a work"} of ${esc(w.material.toLowerCase())}</p><h1>${esc(w.word[0].toUpperCase() + w.word.slice(1))}</h1>`;
  h += `<p class="lede">${linkNames(w.why)}</p><dl class="facts"><dt>At</dt><dd><a href="#/map?t=${w.done ?? T1}&x=${w.at[0]}&y=${w.at[1]}">${w.at[0]},${w.at[1]}</a></dd><dt>Begun</dt><dd>${dayLink(w.first)}</dd>`;
  h += `<dt>Loads</dt><dd>${w.used} of ${w.needed}</dd><dt>${w.done != null ? "Finished" : w.gone != null ? "Set aside" : "Still"}</dt><dd>${w.done != null ? dayLink(w.done) + ` (${dur(w.done - w.first)})` : w.gone != null ? dayLink(w.gone) : "rising"}</dd></dl>`;
  h += `<canvas class="spark" data-kind="work" data-k="${k}"></canvas>`;
  const word = w.word.replace(/^an? /, "");
  const lines = D.log.filter(l => l.text.toLowerCase().includes(word.toLowerCase()));
  h += `<h2>In the log</h2><div class="log">${lines.map(l => `<p><span class="t"><a href="#/day/${dayOf(l.t)}">${dayOf(l.t)}·${clock(l.t)}</a></span>${linkNames(l.text)}</p>`).join("") || `<p class="muted">Not named in the log.</p>`}</div>`;
  return h;
}

// A settler at a moment: what they were doing and why, their needs and place then.
function momentOf(i, t, d = D) {
  const S2 = d.C.settlers;
  const s = spanAt(d, i, t);
  const L = d.byWho[i];
  const k = s ? L.indexOf(s) : L.findIndex(x => x.t0 > t) - 1;
  const ki = at(d.C.needs.ticks, t), kp = at(d.C.pos.ticks, t);
  const n = needsAt(d, ki, i), p = posAt(d, kp, i);
  let h = `<p class="eyebrow">${stamp(t)}</p><h1><a href="#/s/${i}">${esc(S2[i].name)}</a></h1>`;
  if (s) h += `<p class="lede"><span class="vkey" style="background:${vcol(s.v)}"></span><b>${VERBS[s.v]}</b> from ${clock(s.t0)} for ${dur(s.t1 - s.t0)}${s.tx >= 0 ? ` at ${s.tx},${s.ty}` : ""}: ${linkNames(s.why)}</p>`;
  else h += `<p class="lede muted">${aliveAt(d, i, t) ? "No choice recorded." : "Not with the camp at this hour."}</p>`;
  if (p) h += `<p>At ${p.x},${p.y}, level ${p.z}${p.f & 2 ? " (below the ground)" : ""}${p.f & 8 ? ", ill" : ""}${p.f & 128 ? ", in a fit" : ""}${p.f & 64 ? ", carrying" : ""}. <a href="#/map?t=${t}&who=${i}">map</a> · <a href="#/activity?who=${i}&from=${dayOf(t)}&to=${dayOf(t)}">that day</a></p>`;
  if (n) h += `<div class="skill">${[["hunger", n.hunger], ["fatigue", n.fatigue], ["exposure", n.exposure], ["stress", n.stress], ["focus", n.focus]].map(([nm, v]) => `<span>${nm}</span><span class="track"><span class="fill" style="width:${Math.max(0, Math.min(100, v))}%;background:${v > 85 && nm !== "focus" ? "var(--rubric)" : "var(--ink-soft)"}"></span></span><span class="muted">${v}</span>`).join("")}</div><p class="muted">Needs at ${stamp(d.C.needs.ticks[ki])}; stuck ${n.stuck} minutes so far.</p>`;
  if (k >= 0) {
    h += `<h2>Before</h2><div class="log">${L.slice(Math.max(0, k - 8), k).map(x => `<p><span class="t">${clock(x.t0)}</span><span class="vkey" style="background:${vcol(x.v)}"></span>${VERBS[x.v]} ${dur(x.t1 - x.t0)}: ${linkNames(x.why)}</p>`).join("")}</div>`;
    h += `<h2>After</h2><div class="log">${L.slice(k + 1, k + 5).map(x => `<p><span class="t">${clock(x.t0)}</span><span class="vkey" style="background:${vcol(x.v)}"></span>${VERBS[x.v]} ${dur(x.t1 - x.t0)}: ${linkNames(x.why)}</p>`).join("")}</div>`;
  }
  const near = d.log.filter(l => Math.abs(l.t - t) <= 180 && l.text.includes(S2[i].name));
  if (near.length) h += `<h2>The log then</h2><div class="log">${near.map(logLine).join("")}</div>`;
  return h;
}

// ---------------------------------------------------------------------------------------------
// Activity: a row per settler, a bar per stretch of work, coloured by its kind.

function verbLegend(hidden, onChange) {
  const el = document.createElement("div");
  el.className = "legend";
  el.innerHTML = VERBS.map((v, k) => `<label><input type="checkbox" data-k="${k}" ${hidden.has(k) ? "" : "checked"}><span class="vkey" style="background:${vcol(k)}"></span>${v}</label>`).join("");
  el.addEventListener("change", ev => { const k = +ev.target.dataset.k; if (ev.target.checked) hidden.delete(k); else hidden.add(k); onChange(); });
  return el;
}

function activityView(q) {
  view.innerHTML = `<div class="tool"><div class="toolbar"><span class="title">Who did what</span><span id="ac-legend"></span><label>rows <select id="ac-order"><option value="i">as they came</option><option value="idle">most idle first</option><option value="stuck">most stuck first</option></select></label><button id="ac-fit">whole run</button></div><div class="stage"><canvas id="ac"></canvas><div class="hint">wheel to zoom · drag to pan · hover a bar for its why · click to open the moment</div></div></div>`;
  const hidden = new Set();
  const cv = $("#ac");
  let t0 = q.from ? (+q.from - 1) * DAY : T0, t1 = q.to ? +q.to * DAY : T1;
  if (t1 <= t0) t1 = t0 + DAY;
  const focus = q.who != null ? +q.who : null;
  let order = S.map((_, i) => i);
  const sortRows = how => {
    order = S.map((_, i) => i);
    if (how === "idle") order.sort((a, b) => { const A = timeShare(a), Bb = timeShare(b); return Bb.tot[0] / Bb.all - A.tot[0] / A.all; });
    if (how === "stuck") order.sort((a, b) => S[b].stuck - S[a].stuck);
    if (focus != null) order = [focus, ...order.filter(i => i !== focus)];
  };
  sortRows("i");
  $("#ac-legend").appendChild(verbLegend(hidden, () => draw()));
  $("#ac-order").onchange = ev => { sortRows(ev.target.value); draw(); };
  $("#ac-fit").onclick = () => { t0 = T0; t1 = T1; draw(); };
  const L = 138, TOP = 34, BOT = 92;
  let geo = null;
  function draw() {
    const { g, w, h } = fitCanvas(cv);
    const rows = order.length;
    const rh = Math.max(10, Math.min(34, (h - TOP - BOT - 8) / Math.max(1, rows)));
    const X = t => L + (t - t0) / (t1 - t0) * (w - L - 10);
    geo = { X, rh, w, h };
    g.clearRect(0, 0, w, h);
    // Night bands and days.
    const d0 = Math.floor(t0 / DAY), d1 = Math.ceil(t1 / DAY);
    g.fillStyle = css("--night");
    for (let d = d0 - 1; d <= d1; d++) { const a = X(d * DAY + 21 * 60), b = X((d + 1) * DAY + 6 * 60); if (b > L && a < w) g.fillRect(Math.max(L, a), TOP, Math.min(w, b) - Math.max(L, a), rows * rh); }
    const pxDay = (w - L - 10) / ((t1 - t0) / DAY);
    g.font = `11px ${css("--label")}`; g.fillStyle = css("--ink-soft"); g.strokeStyle = css("--rule"); g.lineWidth = 1;
    if (pxDay > 160) {
      const stepH = pxDay > 900 ? 1 : pxDay > 400 ? 3 : 6;
      for (let t = Math.ceil(t0 / (stepH * 60)) * stepH * 60; t <= t1; t += stepH * 60) { const x = X(t); g.globalAlpha = 0.35; g.beginPath(); g.moveTo(x, TOP); g.lineTo(x, TOP + rows * rh); g.stroke(); g.globalAlpha = 1; g.fillText(t % DAY === 0 ? `day ${dayOf(t)}` : clock(t), x + 2, TOP - 6); }
    } else {
      const st = niceStep(pxDay, 50);
      for (let d = Math.ceil(d0 / st) * st; d <= d1; d += st) { const x = X(d * DAY); if (x < L) continue; g.globalAlpha = 0.4; g.beginPath(); g.moveTo(x, TOP); g.lineTo(x, TOP + rows * rh); g.stroke(); g.globalAlpha = 1; g.fillText(`day ${d + 1}`, x + 2, TOP - 6); }
    }
    // Moments along the top.
    for (const m of D.moments) { if (m.t < t0 || m.t > t1) continue; const x = X(m.t); g.fillStyle = m.major ? css("--rubric") : css("--ink-faint"); g.beginPath(); g.moveTo(x, TOP - 2); g.lineTo(x - 4, TOP - 12 + 6); g.lineTo(x + 4, TOP - 12 + 6); g.closePath(); g.fill(); if (m.major) { g.globalAlpha = 0.25; g.fillRect(x, TOP, 1, rows * rh); g.globalAlpha = 1; } }
    // Rows.
    order.forEach((i, r) => {
      const y = TOP + r * rh;
      g.fillStyle = i === focus ? css("--rubric") : css("--ink");
      g.font = `${rh > 14 ? 12 : 10}px ${css("--body")}`;
      g.fillText(clip(S[i].name, 18), 4, y + rh * 0.7);
      // Time away or dead: hatched.
      const life = D.life[i];
      g.fillStyle = css("--page-deep");
      let prev = T0;
      for (const [a, b] of life) { if (a > prev) g.fillRect(Math.max(L, X(prev)), y + 1, Math.max(0, X(a) - Math.max(L, X(prev))), rh - 2); prev = b; }
      if (prev < T1) g.fillRect(Math.max(L, X(prev)), y + 1, Math.max(0, X(T1) - Math.max(L, X(prev))), rh - 2);
      for (const s of D.byWho[i]) {
        if (s.t1 < t0 || s.t0 > t1) continue;
        const a = Math.max(L, X(s.t0)), b = Math.min(w - 10, X(Math.max(s.t1, s.t0 + 1)));
        if (b < L) continue;
        g.globalAlpha = hidden.has(s.v) ? 0.12 : 1;
        g.fillStyle = vcol(s.v);
        g.fillRect(a, y + 1, Math.max(0.6, b - a), rh - 2);
      }
      g.globalAlpha = 1;
    });
    // How many at each kind of work: a stacked strip along the bottom.
    const yb = Math.min(h - BOT + 8, TOP + rows * rh + 18), hb = BOT - 26;
    const buckets = Math.max(10, Math.floor((w - L - 10) / 3));
    const counts = Array.from({ length: buckets }, () => new Array(VERBS.length).fill(0));
    const bw = (t1 - t0) / buckets;
    for (const s of D.spans) {
      if (s.t1 < t0 || s.t0 > t1) continue;
      const a = Math.max(0, Math.floor((s.t0 - t0) / bw)), b = Math.min(buckets - 1, Math.floor((Math.max(s.t1, s.t0 + 1) - 1 - t0) / bw));
      for (let k = a; k <= b; k++) { const lo = t0 + k * bw, hi = lo + bw; counts[k][s.v] += (Math.min(hi, s.t1) - Math.max(lo, s.t0)) / bw; }
    }
    const most = Math.max(1, ...counts.map(c => c.reduce((x, y) => x + y, 0)));
    g.fillStyle = css("--ink-faint"); g.font = `10px ${css("--label")}`; g.fillText("how many at each", 4, yb + 10); g.fillText(`kind of work (${Math.round(most)})`, 4, yb + 22);
    counts.forEach((c, k) => {
      let y = yb + hb;
      const x = L + k * (w - L - 10) / buckets;
      c.forEach((n, v) => { if (!n) return; const hh = n / most * hb; g.globalAlpha = hidden.has(v) ? 0.12 : 1; g.fillStyle = vcol(v); g.fillRect(x, y - hh, (w - L - 10) / buckets + 0.4, hh); y -= hh; });
    });
    g.globalAlpha = 1;
  }
  const hit = ev => {
    const r = cv.getBoundingClientRect();
    const x = ev.clientX - r.left, y = ev.clientY - r.top;
    const t = Math.floor(t0 + (x - L) / (geo.w - L - 10) * (t1 - t0));
    if (y < TOP && y > TOP - 16) {
      const m = D.moments.filter(m => Math.abs(geo.X(m.t) - x) < 5).sort((a, b) => b.major - a.major)[0];
      return m ? { moment: m } : null;
    }
    const row = Math.floor((y - TOP) / geo.rh);
    if (row < 0 || row >= order.length || x < L) return null;
    const i = order[row];
    const s = spanAt(D, i, t);
    return { i, t, s };
  };
  cv.addEventListener("mousemove", ev => {
    if (drag) return;
    const hh = hit(ev);
    cv.style.cursor = hh && (hh.s || hh.moment) ? "pointer" : "default";
    if (!hh) return closeTip();
    if (hh.moment) return showTip(`<b>${esc(hh.moment.title)}</b> · ${stamp(hh.moment.t)}<br>${esc(clip(hh.moment.text, 220))}`, ev.clientX, ev.clientY);
    if (hh.s) showTip(`<b>${esc(S[hh.i].name)}</b> · ${VERBS[hh.s.v]}<br>${stamp(hh.s.t0)} for ${dur(hh.s.t1 - hh.s.t0)}${hh.s.tx >= 0 ? ` · at ${hh.s.tx},${hh.s.ty}` : ""}<br>${esc(hh.s.why)}`, ev.clientX, ev.clientY);
    else showTip(`<b>${esc(S[hh.i].name)}</b> · ${stamp(hh.t)}<br>${aliveAt(D, hh.i, hh.t) ? "no choice recorded" : "not with the camp"}`, ev.clientX, ev.clientY);
  });
  cv.addEventListener("mouseleave", closeTip);
  let drag = null;
  cv.addEventListener("pointerdown", ev => { drag = { x: ev.clientX, t0, t1, moved: false }; });
  const move = ev => {
    if (!drag) return;
    const dx = ev.clientX - drag.x;
    if (Math.abs(dx) > 3) drag.moved = true;
    const dt = dx / (geo.w - L - 10) * (drag.t1 - drag.t0);
    t0 = drag.t0 - dt; t1 = drag.t1 - dt; draw();
  };
  const up = ev => {
    if (drag && !drag.moved) {
      const hh = hit(ev);
      if (hh && hh.moment) location.hash = `#/day/${dayOf(hh.moment.t)}`;
      else if (hh && hh.i != null) openDrawer(momentOf(hh.i, hh.t));
    }
    drag = null;
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  cv.addEventListener("wheel", ev => {
    ev.preventDefault();
    const r = cv.getBoundingClientRect();
    const x = ev.clientX - r.left;
    const tm = t0 + (x - L) / (geo.w - L - 10) * (t1 - t0);
    const k = Math.exp(ev.deltaY * 0.0015);
    const span = Math.max(60, Math.min((T1 - T0) * 1.5, (t1 - t0) * k));
    t0 = tm - (tm - t0) / (t1 - t0) * span; t1 = t0 + span; draw();
  }, { passive: false });
  const onResize = () => draw();
  window.addEventListener("resize", onResize);
  cleanup = () => { window.removeEventListener("pointermove", move); window.removeEventListener("pointerup", up); window.removeEventListener("resize", onResize); };
  draw();
}

// ---------------------------------------------------------------------------------------------
// Needs and stocks

function needsView(q) {
  const focus = q.who != null ? +q.who : null;
  const shown = new Set(focus != null ? [focus] : S.map((_, i) => i));
  view.innerHTML = `<div class="charts"><h1>Needs and stocks</h1><p class="muted">Hover a chart for the values at that hour; click it for the day. Red lines mark deaths.</p>
    <h2>The camp</h2><div id="nd-camp"></div><h2>The settlers</h2><div class="who-list" id="nd-who"></div><div id="nd-people"></div></div>`;
  const marks = deathMarks();
  const toDay = t => { location.hash = `#/day/${dayOf(t)}`; };
  const xs = C.series.ticks, sr = D.series;
  const campCharts = [
    ["Food: stored, the camp's goal", [["stored", sr.food, "--c-nature"], ["goal", sr.food_goal, "--ink-faint"]]],
    ["Days of food in the store", [["days", sr.days_of_food, "--c-faith"]]],
    ["People", [["alive", sr.alive, "--c-people"], ["company", sr.company, "--ink-faint"]]],
    ["Timber, stone and things left lying", [["logs", sr.logs, "--c-town"], ["stone", sr.stone, "--c-other"], ["loose", sr.loose, "--c-war"]]],
    ["Readiness against a raid", [["readiness", sr.readiness, "--c-war"]]],
    ["Hostile creatures on the map", [["hostile", sr.hostile, "--c-beast"]]],
    ["Temperature (°C) and darkness (x10)", [["temperature", sr.temperature, "--c-culture"], ["darkness x10", sr.darkness.map(v => v * 10), "--c-trade"]]],
    ["Works: under way, standing; things made", [["open", sr.works_open, "--c-faith"], ["done", sr.works_done, "--c-town"], ["made", sr.made, "--c-culture"]]],
  ];
  const redraws = [];
  for (const [title, ss] of campCharts) {
    const div = document.createElement("div"); div.className = "chart";
    div.innerHTML = `<h3>${title}</h3><canvas></canvas>`;
    $("#nd-camp").appendChild(div);
    redraws.push(lineChart(div.querySelector("canvas"), xs, ss.map(([n, ys, col]) => ({ name: n, ys, color: css(col) })), { marks, onClick: toDay }));
  }
  const nx = C.needs.ticks;
  const needCharts = [["Hunger (%)", 0], ["Fatigue (%)", 1], ["Exposure to the cold (%)", 2], ["Stress (x100; breaks at 120)", 3], ["Focus (x100: needs met)", 4], ["Minutes stuck (wanting to move, unable)", 5]];
  const peopleDiv = $("#nd-people");
  function drawPeople() {
    peopleDiv.innerHTML = "";
    for (const [title, k] of needCharts) {
      const div = document.createElement("div"); div.className = "chart";
      div.innerHTML = `<h3>${title}</h3><canvas></canvas>`;
      peopleDiv.appendChild(div);
      const series = S.map((s, i) => ({ name: s.name, color: personColor(i), faint: !shown.has(i), width: shown.size === 1 && shown.has(i) ? 2.2 : 1.2, ys: C.needs.rows.map(r => r.length > i * 6 + k ? r[i * 6 + k] : null) }));
      series.sort((a, b) => a.faint - b.faint).reverse();
      lineChart(div.querySelector("canvas"), nx, series, { marks, onClick: toDay, unit: k < 3 ? "%" : "" });
    }
  }
  $("#nd-who").innerHTML = `<label><input type="checkbox" id="nd-all" ${shown.size === S.length ? "checked" : ""}> all</label>` + S.map((s, i) => `<label><input type="checkbox" data-i="${i}" ${shown.has(i) ? "checked" : ""}><span class="sw" style="background:${personColor(i)}"></span>${esc(s.name)}</label>`).join("");
  $("#nd-who").onchange = ev => {
    if (ev.target.id === "nd-all") { shown.clear(); if (ev.target.checked) S.forEach((_, i) => shown.add(i)); for (const c of document.querySelectorAll("#nd-who input[data-i]")) c.checked = ev.target.checked; }
    else { const i = +ev.target.dataset.i; if (ev.target.checked) shown.add(i); else shown.delete(i); }
    drawPeople();
  };
  drawPeople();
  const onResize = () => { redraws.forEach(r => r()); drawPeople(); };
  window.addEventListener("resize", onResize);
  cleanup = () => window.removeEventListener("resize", onResize);
}

// ---------------------------------------------------------------------------------------------
// The map, replayed

const IMG = new Map();
function image(src, onload) {
  let im = IMG.get(src);
  if (!im) { im = new Image(); im.src = src; IMG.set(src, im); }
  if (!im.complete && onload) im.addEventListener("load", onload, { once: true });
  return im;
}

function mapView(q) {
  const ticks = C.pos.ticks;
  if (!ticks.length) { view.innerHTML = `<p class="checks">No positions were recorded.</p>`; return; }
  const levels = M.levels || [];
  view.innerHTML = `<div class="tool"><div class="toolbar controls">
      <button id="mp-play" title="Play or pause (space)">▶</button>
      <select id="mp-speed" title="Game time a second"><option value="60">1 hour/s</option><option value="360" selected>6 hours/s</option><option value="1440">1 day/s</option><option value="7200">5 days/s</option></select>
      <input id="mp-t" type="range" min="0" max="${ticks.length - 1}" step="1"><span class="stamp" id="mp-stamp"></span>
      <label>level <select id="mp-level"><option value="s">the surface</option>${levels.map(l => `<option value="${l[0]}">${l[2] > 0 ? "+" : ""}${l[2]} (z ${l[0]})${l[3].length ? ": " + esc(l[3].slice(0, 3).join(", ")) : ""}</option>`).join("")}</select></label>
      <label><input type="checkbox" id="mp-trails" checked> trails</label><label><input type="checkbox" id="mp-names" checked> names</label><label><input type="checkbox" id="mp-works"> works</label>
      <label>follow <select id="mp-who"><option value="">no one</option>${S.map((s, i) => `<option value="${i}">${esc(s.name)}</option>`).join("")}</select></label>
    </div><div class="stage"><canvas id="mp"></canvas><div class="hint" id="mp-hint">wheel to zoom · drag to pan · click a settler</div><button id="mp-fit" style="position:absolute;right:1rem;bottom:0.7rem">whole map</button></div></div>`;
  const cv = $("#mp");
  const slider = $("#mp-t");
  let k = at(ticks, q.t != null ? +q.t : T1);
  let sel = q.who != null && q.who !== "" ? +q.who : null;
  if (sel != null) $("#mp-who").value = String(sel);
  slider.value = k;
  let level = "s";
  let scale = 0, ox = 0, oy = 0;
  let playing = false, raf = 0, last = 0, acc = 0;
  const W = M.w, Hh = M.h;
  const fit = () => {
    const r = cv.getBoundingClientRect();
    scale = Math.min(r.width / W, r.height / Hh) * 0.98;
    // Centred on the place asked for, else the settler followed, else the camp, close enough
    // to read it ("whole map" in the hint's corner fits it all).
    let fx = M.camp[0], fy = M.camp[1];
    const p0 = sel != null ? posAt(D, k, sel) : null;
    if (q.x != null) { fx = +q.x; fy = +q.y; } else if (p0) { fx = p0.x; fy = p0.y; }
    scale = Math.max(scale, q.x != null || p0 ? 6 : 4.5);
    ox = r.width / 2 - fx * scale; oy = r.height / 2 - fy * scale;
  };
  const S2 = (x, y) => [ox + (x + 0.5) * scale, oy + (y + 0.5) * scale];
  let shown = [];
  function draw() {
    const { g, w, h } = fitCanvas(cv);
    if (!scale) fit();
    const t = ticks[k];
    $("#mp-stamp").textContent = stamp(t);
    g.fillStyle = css("--page-deep"); g.fillRect(0, 0, w, h);
    const src = level === "s" ? frameFor(t)?.[1] : levels.find(l => String(l[0]) === level)?.[1];
    if (src) { const im = image(src, draw); if (im.complete && im.naturalWidth) { g.imageSmoothingEnabled = scale < 3; g.drawImage(im, ox, oy, W * scale, Hh * scale); } }
    if (level !== "s") { g.fillStyle = css("--ink-faint"); g.font = `12px ${css("--label")}`; g.fillText("the level as it stands at the end of the recording", 10, h - 30); }
    // Works and marks.
    if ($("#mp-works").checked) for (const wk of C.works) {
      if (wk.first > t) continue;
      const [x, y] = S2(wk.at[0], wk.at[1]);
      g.strokeStyle = wk.done != null && wk.done <= t ? css("--c-town") : css("--rubric"); g.lineWidth = 1.5; g.setLineDash(wk.done != null && wk.done <= t ? [] : [3, 3]);
      g.strokeRect(x - 4, y - 4, 8, 8); g.setLineDash([]);
      if (scale > 3) { g.font = `11px ${css("--body")}`; g.fillStyle = css("--ink"); g.fillText(wk.word, x + 6, y + 4); }
    }
    for (const m of C.marks) {
      if (m.day > dayOf(t) || level !== "s") continue;
      const [x, y] = S2(m.at[0], m.at[1]);
      g.fillStyle = m.kind === "Grave" ? css("--ink") : css("--ink-soft");
      g.fillRect(x - 1, y - 4, 2, 8); g.fillRect(x - 3, y - 2, 6, 2);
    }
    // Moments of the last half day.
    for (const m of D.moments) {
      if (m.t > t || m.t < t - 720) continue;
      const [x, y] = S2(m.x, m.y);
      const a = 1 - (t - m.t) / 720;
      g.strokeStyle = alphaOf(css("--rubric"), 0.3 + 0.6 * a); g.lineWidth = 2;
      g.beginPath(); g.arc(x, y, 8 + 10 * (1 - a), 0, Math.PI * 2); g.stroke();
      if (m.major) { g.font = `600 12px ${css("--body")}`; g.fillStyle = css("--rubric"); g.fillText(m.title, x + 12, y - 8); }
    }
    // Creatures.
    const crow = C.creatures[k] || [];
    for (let j = 0; j < crow.length; j += 5) {
      const ck = D.creatureKind.get(crow[j]) || { kind: "?", name: "" };
      if (level !== "s" && String(crow[j + 3]) !== level) continue;
      const [x, y] = S2(crow[j + 1], crow[j + 2]);
      const hostile = isHostile(ck.kind);
      g.fillStyle = hostile ? css("--c-war") : ck.kind === "Trader" ? css("--c-town") : ck.kind === "Pet" ? css("--c-nature") : css("--ink-faint");
      g.beginPath(); g.moveTo(x, y - 5); g.lineTo(x + 5, y + 4); g.lineTo(x - 5, y + 4); g.closePath(); g.fill();
      if (hostile && /Beast|Raider/.test(ck.kind) && scale > 2.5 && $("#mp-names").checked) { g.font = `italic 11px ${css("--body")}`; g.fillText(clip(ck.name, 30), x + 7, y + 4); }
    }
    // Trails, then settlers.
    shown = [];
    const back = Math.round(360 / M.every);
    for (let i = 0; i < S.length; i++) {
      const p = posAt(D, k, i);
      if (!p || !(p.f & 1) || (p.f & 32)) continue;
      const below = !!(p.f & 2);
      if (level === "s" ? false : p.z !== +level) continue;
      if ($("#mp-trails").checked && (sel == null || sel === i)) {
        g.strokeStyle = alphaOf(personColor(i).startsWith("#") ? personColor(i) : "#382a20", 0.5);
        g.strokeStyle = personColor(i); g.globalAlpha = sel === i ? 0.85 : 0.35; g.lineWidth = sel === i ? 2 : 1;
        g.beginPath();
        let started = false;
        for (let j = Math.max(0, k - (sel === i ? back * 4 : back)); j <= k; j++) {
          const q2 = posAt(D, j, i); if (!q2 || !(q2.f & 1)) { started = false; continue; }
          const [x, y] = S2(q2.x, q2.y);
          if (!started) { g.moveTo(x, y); started = true; } else g.lineTo(x, y);
        }
        g.stroke(); g.globalAlpha = 1;
      }
      const s = spanAt(D, i, ticks[k]);
      const [x, y] = S2(p.x, p.y);
      const r = Math.max(3.5, Math.min(7, scale * 0.9));
      g.fillStyle = s ? vcol(s.v) : css("--ink-faint");
      g.strokeStyle = sel === i ? css("--rubric") : css("--ink"); g.lineWidth = sel === i ? 2.5 : 1;
      g.globalAlpha = below && level === "s" ? 0.45 : 1;
      g.beginPath(); g.arc(x, y, r, 0, Math.PI * 2); g.fill(); g.stroke();
      if (p.f & 128) { g.strokeStyle = css("--c-war"); g.beginPath(); g.arc(x, y, r + 3, 0, Math.PI * 2); g.stroke(); }
      g.globalAlpha = 1;
      if ($("#mp-names").checked && (scale > 2.2 || sel === i)) {
        g.font = `${sel === i ? "600 " : ""}11px ${css("--body")}`; g.lineWidth = 3; g.strokeStyle = alphaOf(css("--wash"), 0.85); g.strokeText(S[i].name, x + r + 3, y + 4); g.fillStyle = css("--ink"); g.fillText(S[i].name, x + r + 3, y + 4);
      }
      shown.push({ i, x, y, r, p, s });
    }
    cv._shown = shown;
    if (sel != null && !shown.some(o => o.i === sel)) { g.font = `12px ${css("--label")}`; g.fillStyle = css("--rubric"); const p = posAt(D, k, sel); g.fillText(`${S[sel].name}: ${!p || !(p.f & 1) ? "not with the camp" : p.f & 32 ? "away" : level === "s" ? "" : "on another level (z " + p.z + ")"}`, 10, 18); }
  }
  const setK = nk => { k = Math.max(0, Math.min(ticks.length - 1, nk)); slider.value = k; if (sel != null && playing) follow(); draw(); };
  const follow = () => {
    const p = posAt(D, k, sel); if (!p) return;
    const r = cv.getBoundingClientRect(); const [x, y] = S2(p.x, p.y);
    if (x < r.width * 0.2 || x > r.width * 0.8 || y < r.height * 0.2 || y > r.height * 0.8) { ox += r.width / 2 - x; oy += r.height / 2 - y; }
  };
  slider.oninput = () => setK(+slider.value);
  $("#mp-fit").onclick = () => { const r = cv.getBoundingClientRect(); scale = Math.min(r.width / W, r.height / Hh) * 0.98; ox = r.width / 2 - W / 2 * scale; oy = r.height / 2 - Hh / 2 * scale; draw(); };
  $("#mp-level").onchange = ev => { level = ev.target.value; draw(); };
  for (const id of ["#mp-trails", "#mp-names", "#mp-works"]) $(id).onchange = draw;
  $("#mp-who").onchange = ev => { sel = ev.target.value === "" ? null : +ev.target.value; if (sel != null) { scale = Math.max(scale, 5); follow(); } draw(); };
  const tickFn = now => {
    if (!playing) return;
    const dt = last ? (now - last) / 1000 : 0; last = now;
    acc += dt * +$("#mp-speed").value / M.every;
    if (acc >= 1) { const n = Math.floor(acc); acc -= n; if (k + n >= ticks.length - 1) { setK(ticks.length - 1); stop(); return; } setK(k + n); }
    raf = requestAnimationFrame(tickFn);
  };
  const stop = () => { playing = false; $("#mp-play").textContent = "▶"; cancelAnimationFrame(raf); };
  $("#mp-play").onclick = () => { if (playing) return stop(); if (k >= ticks.length - 1) setK(0); playing = true; last = 0; $("#mp-play").textContent = "❚❚"; raf = requestAnimationFrame(tickFn); };
  const keys = ev => { if (ev.target.tagName === "INPUT" && ev.target.type !== "range") return; if (ev.key === " ") { ev.preventDefault(); $("#mp-play").click(); } else if (ev.key === "ArrowRight") setK(k + 1); else if (ev.key === "ArrowLeft") setK(k - 1); };
  window.addEventListener("keydown", keys);
  let drag = null;
  const nearest = ev => {
    const r = cv.getBoundingClientRect(), x = ev.clientX - r.left, y = ev.clientY - r.top;
    let best = null, bd = 12;
    for (const o of shown) { const d = Math.hypot(o.x - x, o.y - y); if (d < bd) { bd = d; best = o; } }
    return best;
  };
  cv.addEventListener("pointerdown", ev => { drag = { x: ev.clientX, y: ev.clientY, ox, oy, moved: false }; });
  const move = ev => {
    if (drag) {
      const dx = ev.clientX - drag.x, dy = ev.clientY - drag.y;
      if (Math.abs(dx) + Math.abs(dy) > 3) drag.moved = true;
      if (drag.moved) { ox = drag.ox + dx; oy = drag.oy + dy; draw(); }
      return;
    }
    if (ev.target !== cv) return;
    const o = nearest(ev);
    cv.style.cursor = o ? "pointer" : "grab";
    if (o) showTip(`<b>${esc(S[o.i].name)}</b> · ${o.s ? VERBS[o.s.v] : "?"} · ${o.p.x},${o.p.y} z ${o.p.z}${o.p.f & 2 ? " (below)" : ""}<br>${esc(o.s?.why ?? "")}`, ev.clientX, ev.clientY);
    else {
      const r = cv.getBoundingClientRect();
      const cx = Math.floor((ev.clientX - r.left - ox) / scale), cy = Math.floor((ev.clientY - r.top - oy) / scale);
      const mk = C.marks.find(m => Math.abs(m.at[0] - cx) <= 1 && Math.abs(m.at[1] - cy) <= 1 && m.day <= dayOf(ticks[k]));
      if (mk) showTip(`<b>${esc(mk.title)}</b><br>${esc(clip(mk.text, 200))}`, ev.clientX, ev.clientY);
      else if (cx >= 0 && cy >= 0 && cx < W && cy < Hh) { closeTip(); $("#mp-hint").textContent = `cell ${cx},${cy} · wheel to zoom · drag to pan · click a settler`; }
    }
  };
  const up = ev => {
    if (drag && !drag.moved && ev.target === cv) {
      const o = nearest(ev);
      if (o) { sel = o.i; $("#mp-who").value = String(o.i); openDrawer(momentOf(o.i, ticks[k])); draw(); }
    }
    drag = null;
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  cv.addEventListener("wheel", ev => {
    ev.preventDefault();
    const r = cv.getBoundingClientRect(), mx = ev.clientX - r.left, my = ev.clientY - r.top;
    const f = Math.exp(-ev.deltaY * 0.0015);
    const ns = Math.max(1, Math.min(40, scale * f));
    ox = mx - (mx - ox) * ns / scale; oy = my - (my - oy) * ns / scale; scale = ns; draw();
  }, { passive: false });
  const onResize = () => draw();
  window.addEventListener("resize", onResize);
  cleanup = () => { stop(); window.removeEventListener("pointermove", move); window.removeEventListener("pointerup", up); window.removeEventListener("keydown", keys); window.removeEventListener("resize", onResize); };
  draw();
}

// ---------------------------------------------------------------------------------------------
// Bonds: what the settlers think of each other on a day.

function opinionsAt(k) {
  const m = new Map();
  const f = C.opinions[k]?.[1] || [];
  for (let j = 0; j < f.length; j += 3) m.set(f[j] + "," + f[j + 1], f[j + 2]);
  return m;
}
function bondsView(q) {
  if (!C.opinions.length) { view.innerHTML = `<p class="checks">No opinions were recorded (they are sampled at dawn).</p>`; return; }
  const ticks = C.opinions.map(o => o[0]);
  view.innerHTML = `<div class="tool"><div class="toolbar controls"><span class="title">Bonds</span>
    <input id="bd-t" type="range" min="0" max="${ticks.length - 1}" value="${ticks.length - 1}"><span class="stamp" id="bd-stamp"></span>
    <label>at least <input id="bd-min" type="number" value="${q.min ?? 8}" min="1" max="60" style="width:4rem"></label>
    <span class="legend"><label><span class="vkey" style="background:var(--c-nature)"></span>fond</label><label><span class="vkey" style="background:var(--c-war)"></span>at odds</label><label><span class="vkey" style="background:var(--c-faith)"></span>wed</label><label><span class="vkey" style="background:var(--ink-soft)"></span>parent</label></span>
    </div><div class="stage"><svg id="bd"></svg><div class="hint">click a line for the two of them over time · click a name for the settler</div></div></div>`;
  const svg = $("#bd");
  const focus = q.who != null ? +q.who : null;
  function draw() {
    const k = +$("#bd-t").value, t = ticks[k], min = +$("#bd-min").value;
    $("#bd-stamp").textContent = stamp(t);
    svg.innerHTML = "";
    const r = svg.getBoundingClientRect();
    const W = r.width, Hh = r.height;
    svg.setAttribute("viewBox", `0 0 ${W} ${Hh}`);
    const here = S.map((_, i) => i).filter(i => aliveAt(D, i, t));
    const R = Math.min(W - 260, Hh) / 2 - 60, cx = Math.min(W - 260, W) / 2 + 30, cy = Hh / 2;
    const pos = new Map(here.map((i, j) => [i, [cx + R * Math.cos(j / here.length * Math.PI * 2 - Math.PI / 2), cy + R * Math.sin(j / here.length * Math.PI * 2 - Math.PI / 2)]]));
    const op = opinionsAt(k);
    const edges = [];
    for (const [key, v] of op) { const [a, b] = key.split(",").map(Number); if (pos.has(a) && pos.has(b) && Math.abs(v) >= min) edges.push([a, b, v]); }
    edges.sort((x, y) => Math.abs(x[2]) - Math.abs(y[2]));
    const gE = svgEl("g"); svg.appendChild(gE);
    for (const [a, b, v] of edges) {
      const [x1, y1] = pos.get(a), [x2, y2] = pos.get(b);
      const dim = focus != null && a !== focus && b !== focus;
      const mx = (x1 + x2) / 2 + (cy - (y1 + y2) / 2) * 0.12, my = (y1 + y2) / 2 + ((x1 + x2) / 2 - cx) * 0.12;
      const d = `M${x1},${y1} Q${mx},${my} ${x2},${y2}`;
      gE.appendChild(svgEl("path", { d, fill: "none", stroke: v > 0 ? css("--c-nature") : css("--c-war"), "stroke-width": Math.min(7, 0.6 + Math.abs(v) / 8), opacity: dim ? 0.12 : 0.75 }));
      const hitp = svgEl("path", { d, fill: "none", stroke: "transparent", "stroke-width": 9, style: "cursor:pointer" });
      hitp.addEventListener("mousemove", ev => showTip(`<b>${esc(S[a].name)}</b> and <b>${esc(S[b].name)}</b>: ${v > 0 ? "+" : ""}${v}`, ev.clientX, ev.clientY));
      hitp.addEventListener("mouseleave", closeTip);
      hitp.addEventListener("click", () => openDrawer(pairPage(a, b)));
      gE.appendChild(hitp);
    }
    // Families: spouses and children (as recorded at the end).
    for (const i of here) {
      const s = S[i];
      if (s.spouse != null && pos.has(s.spouse) && i < s.spouse) { const [x1, y1] = pos.get(i), [x2, y2] = pos.get(s.spouse); gE.appendChild(svgEl("line", { x1, y1, x2, y2, stroke: css("--c-faith"), "stroke-width": 3, "stroke-dasharray": "1 3", "stroke-linecap": "round" })); }
    }
    for (const [c, m, f] of C.children || []) for (const par of [m, f]) if (pos.has(c) && pos.has(par)) { const [x1, y1] = pos.get(c), [x2, y2] = pos.get(par); gE.appendChild(svgEl("line", { x1, y1, x2, y2, stroke: css("--ink-soft"), "stroke-width": 1.2, "stroke-dasharray": "5 3" })); }
    for (const i of here) {
      const [x, y] = pos.get(i);
      const g = svgEl("g", { style: "cursor:pointer" });
      const sum = [...op].filter(([key]) => key.split(",").map(Number).includes(i)).reduce((a, [, v]) => a + v, 0);
      g.appendChild(svgEl("circle", { cx: x, cy: y, r: 7 + Math.min(8, Math.abs(sum) / 40), fill: personColor(i), stroke: focus === i ? css("--rubric") : css("--ink"), "stroke-width": focus === i ? 3 : 1 }));
      const right = x >= cx;
      g.appendChild(svgEl("text", { x: x + (right ? 14 : -14), y: y + 4, "text-anchor": right ? "start" : "end", "font-size": 13 }, S[i].name));
      g.addEventListener("click", () => openDrawer(settlerPage(i)));
      g.addEventListener("mousemove", ev => showTip(`<b>${esc(S[i].name)}</b><br>sum of what the others think: ${sum > 0 ? "+" : ""}${sum}`, ev.clientX, ev.clientY));
      g.addEventListener("mouseleave", closeTip);
      svg.appendChild(g);
    }
    // Who is fondest and most at odds, on the right.
    const best = [...edges].sort((x, y) => y[2] - x[2]).slice(0, 6), worst = [...edges].filter(e => e[2] < 0).sort((x, y) => x[2] - y[2]).slice(0, 6);
    let y = 30;
    const gx = W - 240;
    const line = (txt, col, bold) => { svg.appendChild(svgEl("text", { x: gx, y, "font-size": bold ? 14 : 12, fill: col || css("--ink") }, txt)); y += bold ? 20 : 16; };
    line("Fondest", null, true); best.forEach(e => line(`${S[e[0]].name} & ${S[e[1]].name} +${e[2]}`, css("--c-nature")));
    y += 10; line("At odds", null, true); worst.forEach(e => line(`${S[e[0]].name} & ${S[e[1]].name} ${e[2]}`, css("--c-war")));
    if (!worst.length) line("no one", css("--ink-faint"));
  }
  $("#bd-t").oninput = draw; $("#bd-min").oninput = draw;
  const onResize = () => draw();
  window.addEventListener("resize", onResize);
  cleanup = () => window.removeEventListener("resize", onResize);
  draw();
}
function pairPage(a, b) {
  const lines = D.log.filter(l => l.text.includes(S[a].name) && l.text.includes(S[b].name));
  return `<p class="eyebrow">two settlers</p><h1>${who(a)} and ${who(b)}</h1><canvas class="spark" data-kind="pair" data-a="${a}" data-b="${b}"></canvas><p class="muted">What they think of each other, each dawn.</p><h2>Together in the log</h2><div class="log">${lines.map(l => `<p><span class="t"><a href="#/day/${dayOf(l.t)}">${dayOf(l.t)}·${clock(l.t)}</a></span>${linkNames(l.text)}</p>`).join("") || `<p class="muted">Never named together.</p>`}</div>`;
}

// ---------------------------------------------------------------------------------------------
// Works

function worksView() {
  view.innerHTML = `<div class="tool"><div class="toolbar"><span class="title">The works rising</span><span class="legend"><label><span class="vkey" style="background:var(--c-town)"></span>built</label><label><span class="vkey" style="background:var(--c-other)"></span>dug</label><label><span class="vkey" style="background:var(--rubric)"></span>set aside</label></span><span class="muted">the darker part of a bar is the share of loads laid</span></div><div class="stage" style="overflow-y:auto"><canvas id="wk"></canvas><div class="checks" id="wk-made" style="height:auto"></div></div></div>`;
  const cv = $("#wk");
  const works = C.works.map((w, k) => ({ ...w, k })).sort((a, b) => a.first - b.first);
  cv.style.height = (works.length * 22 + 50) + "px";
  let geo;
  function draw() {
    const { g, w, h } = fitCanvas(cv);
    const L = 210, X = t => L + (t - T0) / Math.max(1, T1 - T0) * (w - L - 16);
    geo = { X, L, w };
    g.clearRect(0, 0, w, h);
    g.font = `11px ${css("--label")}`; g.fillStyle = css("--ink-faint"); g.strokeStyle = css("--rule");
    const st = niceStep((w - L - 16) / Math.max(1, (T1 - T0) / DAY), 50);
    for (let d = Math.ceil(T0 / DAY / st) * st; d * DAY <= T1; d += st) { const x = X(d * DAY); g.globalAlpha = 0.4; g.beginPath(); g.moveTo(x, 24); g.lineTo(x, h); g.stroke(); g.globalAlpha = 1; g.fillText(`day ${d + 1}`, x + 2, 16); }
    works.forEach((wk, r) => {
      const y = 28 + r * 22;
      g.fillStyle = css("--ink"); g.font = `12px ${css("--body")}`; g.fillText(clip(wk.word, 30), 4, y + 13);
      const end = wk.done ?? wk.gone ?? T1;
      const col = wk.gone != null ? css("--rubric") : wk.dig ? css("--c-other") : css("--c-town");
      const a = X(wk.first), b = Math.max(a + 2, X(end));
      g.fillStyle = alphaOf(col, 0.22); g.fillRect(a, y + 2, b - a, 15);
      // The share laid, as a step chart inside the bar.
      g.fillStyle = col;
      const steps = [[wk.first, 0], ...wk.steps];
      for (let j = 0; j < steps.length; j++) {
        const s0 = steps[j][0], s1 = j + 1 < steps.length ? steps[j + 1][0] : end;
        const hh = 15 * Math.min(1, steps[j][1] / Math.max(1, wk.needed));
        g.fillRect(X(s0), y + 17 - hh, Math.max(0.5, X(s1) - X(s0)), hh);
      }
      if (wk.done != null) { g.fillStyle = css("--ink"); g.fillText("✓", b + 3, y + 13); }
    });
  }
  cv.addEventListener("mousemove", ev => {
    const r = cv.getBoundingClientRect();
    const row = Math.floor((ev.clientY - r.top - 28) / 22);
    const wk = works[row];
    if (!wk) return closeTip();
    cv.style.cursor = "pointer";
    showTip(`<b>${esc(wk.word)}</b> at ${wk.at[0]},${wk.at[1]} · ${wk.used}/${wk.needed} loads<br>${stamp(wk.first)} → ${wk.done != null ? stamp(wk.done) : wk.gone != null ? "set aside " + stamp(wk.gone) : "rising"}<br>${esc(clip(wk.why, 240))}`, ev.clientX, ev.clientY);
  });
  cv.addEventListener("mouseleave", closeTip);
  cv.addEventListener("click", ev => { const r = cv.getBoundingClientRect(); const wk = works[Math.floor((ev.clientY - r.top - 28) / 22)]; if (wk) { openDrawer(workPage(wk.k)); } });
  draw();
  const q = ["ordinary", "well-crafted", "finely-crafted", "superior", "exceptional", "masterful"];
  $("#wk-made").innerHTML = `<h2>Things made (${C.made.length})</h2><table class="list"><tr><th>Day</th><th>Maker</th><th>What</th><th>Quality</th><th>Shows</th><th></th></tr>${C.made.map(m => `<tr><td>${m.day}</td><td>${who(m.maker)}</td><td>${esc(m.material)} ${esc(m.kind)}${m.called ? ` (${esc(m.called)})` : ""}</td><td>${q[m.quality] ?? m.quality}</td><td>${m.image ? esc(m.image) + (histEvent(m.event) ? ` <a class="why" href="${histEvent(m.event)}">history</a>` : "") : ""}</td><td class="muted">${m.traded ? "sold" : ""}</td></tr>`).join("")}</table>`;
  const onResize = () => draw();
  window.addEventListener("resize", onResize);
  cleanup = () => window.removeEventListener("resize", onResize);
}

// ---------------------------------------------------------------------------------------------
// Checks: what a debugger looks for in a colony.

function findings(d) {
  const C = d.C, S = C.settlers, out = [];
  const mapLink = (t, i) => `<a href="#/map?t=${t}${i != null ? "&who=" + i : ""}">${stamp(t)}</a>`;
  // Deaths, and what the store held.
  const food = t => { const k = at(C.series.ticks, t); return C.series.rows[k]?.[0]; };
  const deaths = S.map((s, i) => [s, i]).filter(([s]) => !s.alive && /died|killed|slain|starv|lost|carried|fell/i.test(s.fate));
  const deathRows = deaths.map(([s, i]) => { const t = C.status.filter(x => x[1] === i && !x[2]).pop()?.[0] ?? 0; return { t, i, text: `${s.fate}`, food: food(t) }; });
  const starvedWithFood = deathRows.filter(r => /hunger|starv/i.test(r.text) && r.food > 0);
  out.push({ name: "Deaths", n: deathRows.length, bad: starvedWithFood.length > 0, rows: deathRows.map(r => `${who(r.i)} ${esc(r.text)} · ${mapLink(r.t, r.i)} · the store held ${r.food ?? "?"} meals`) });
  // Hungry with food in the store, three hours running.
  const hungry = []; const run = S.map(() => 0);
  C.needs.rows.forEach((row, k) => {
    const t = C.needs.ticks[k], f = C.series.rows[k]?.[0] ?? 0;
    for (let i = 0; i * 6 < row.length; i++) {
      const p = posAt(d, at(C.pos.ticks, t), i);
      const ok = p && (p.f & 1) && !(p.f & 32);
      run[i] = ok && row[i * 6] >= 95 && f >= 1 ? run[i] + 1 : 0;
      if (run[i] === 3) hungry.push(`${who(i)} from ${mapLink(t - 120, i)}: hunger ${row[i * 6]}% with ${f} meals in the store`);
    }
  });
  out.push({ name: "Hungry three hours with food in the store", n: hungry.length, bad: hungry.length > 0, rows: hungry });
  const stuck = S.map((s, i) => [s.stuck, i]).filter(x => x[0] > 0).sort((a, b) => b[0] - a[0]);
  out.push({ name: "Minutes stuck (wanting to move, unable)", n: stuck.reduce((a, b) => a + b[0], 0), bad: stuck.some(x => x[0] > 100), rows: stuck.map(([n, i]) => `${who(i)}: ${n}`) });
  out.push({ name: "Standing where no one can stand", n: C.odd.unstandable_count, bad: C.odd.unstandable_count > 0, rows: C.odd.unstandable.map(u => `${mapLink(u[0], u[1])} ${who(u[1])} at ${u[2]},${u[3]} level ${u[4]} (${VERBS[u[5]]})`) });
  out.push({ name: "Jumps no walk could make", n: C.odd.jumps.length, bad: C.odd.jumps.length > 0, rows: C.odd.jumps.map(j => `${mapLink(j[0], j[1])} ${who(j[1])} ${j[2]},${j[3]} → ${j[4]},${j[5]} (${Math.max(Math.abs(j[4] - j[2]), Math.abs(j[5] - j[3]))} cells)`) });
  const long = d.spans.filter(s => s.t1 - s.t0 >= DAY && s.v !== 2 && s.v !== 12);
  out.push({ name: "A job held a day or more", n: long.length, bad: long.length > 0, rows: long.map(s => `${who(s.who)} ${VERBS[s.v]} from ${mapLink(s.t0, s.who)} for ${dur(s.t1 - s.t0)}: ${esc(s.why)}`) });
  const thrash = [];
  d.byWho.forEach((L, i) => {
    let k0 = 0, hits = 0, first = null;
    for (let k = 0; k < L.length; k++) { while (L[k].t0 - L[k0].t0 > 60) k0++; if (k - k0 + 1 >= 8) { hits++; if (first == null) first = L[k0].t0; } }
    if (hits) thrash.push(`${who(i)}: ${hits} times, first ${mapLink(first, i)} <a class="why" href="#/activity?who=${i}&from=${dayOf(first)}&to=${dayOf(first)}">activity</a>`);
  });
  out.push({ name: "Eight choices within an hour", n: thrash.length, bad: thrash.length > 0, rows: thrash });
  const stalled = C.works.filter(w => w.done == null && w.gone == null && T1 - (w.steps.length ? w.steps[w.steps.length - 1][0] : w.first) >= 8 * DAY);
  out.push({ name: "Works stalled eight days or more", n: stalled.length, bad: stalled.length > 0, rows: stalled.map(w => `${esc(w.word)} at ${w.at[0]},${w.at[1]}: ${w.used}/${w.needed}`) });
  const gone = C.works.filter(w => w.gone != null);
  out.push({ name: "Works set aside or struck off", n: gone.length, bad: false, rows: gone.map(w => `${esc(w.word)}: ${w.used}/${w.needed}, ${stamp(w.gone)}`) });
  const lt = d.log.map(l => l.t), gaps = [];
  for (let k = 1; k < lt.length; k++) if (lt[k] - lt[k - 1] > 2 * DAY) gaps.push(`${stamp(lt[k - 1])} to ${stamp(lt[k])}`);
  out.push({ name: "Two days or more with nothing in the log", n: gaps.length, bad: false, rows: gaps });
  out.push({ name: "The patron's acts that failed", n: C.odd.failures.length, bad: C.odd.failures.length > 0, rows: C.odd.failures.map(esc) });
  return out;
}

function checksView() {
  const F = findings(D);
  let h = `<div class="checks"><h1>Checks</h1><p class="muted">What a debugger looks for in a colony, read off the recording. The lab's <code>colony_checks {"replay":true}</code> also runs the colony a second time to see that it tells the same story.</p>`;
  h += `<div class="cards">${F.map(f => card(f.n, esc(f.name), f.bad)).join("")}</div>`;
  for (const f of F) h += `<h2>${esc(f.name)} <span class="${f.bad ? "flag" : "muted"}">${f.n}</span></h2>${f.rows.length ? `<div class="log">${f.rows.slice(0, 40).map(r => `<p>${r}</p>`).join("")}${f.rows.length > 40 ? `<p class="muted">… and ${f.rows.length - 40} more.</p>` : ""}</div>` : `<p class="ok">None.</p>`}`;
  h += `<h2>Each settler's time</h2><table class="list">${S.map((s, i) => { const { tot, all } = timeShare(i); return `<tr><td>${who(i)}</td><td style="width:60%"><div class="share" style="margin:0">${tot.map((t, v) => t ? `<span style="width:${t / all * 100}%;background:${vcol(v)}" title="${VERBS[v]} ${Math.round(t / all * 100)}%"></span>` : "").join("")}</div></td><td class="muted">idle ${Math.round(tot[0] / all * 100)}%</td></tr>`; }).join("")}</table><p class="muted">${VERBS.map((v, k) => `<span class="vkey" style="background:${vcol(k)}"></span>${v}`).join(" · ")}</p>`;
  // The reasons that take the most time.
  const why = new Map();
  for (const s of D.spans) { const key = s.why.replace(/\d+(\.\d+)?/g, "N").replace(/ at N,N/g, "").slice(0, 90); const o = why.get(key) || { t: 0, n: 0, v: s.v }; o.t += s.t1 - s.t0; o.n++; why.set(key, o); }
  const top = [...why].sort((a, b) => b[1].t - a[1].t).slice(0, 30);
  const most = top[0]?.[1].t || 1;
  h += `<h2>The reasons that fill their days</h2><p class="muted">Every why, numbers taken out, by the time spent on it.</p><div class="bars">${top.map(([k, o]) => `<span>${esc(k)}</span><span class="track"><span class="fill" style="width:${o.t / most * 100}%;background:${vcol(o.v)}"></span></span><span class="muted">${dur(o.t)} · ${o.n}×</span>`).join("")}</div>`;
  h += `<h2>Choices a day</h2><canvas id="ck-day"></canvas></div>`;
  view.innerHTML = h;
  const perDay = new Map();
  for (const s of D.spans) perDay.set(dayOf(s.t0), (perDay.get(dayOf(s.t0)) || 0) + 1);
  const xs = [], ys = [];
  for (let d = FIRST_DAY; d <= LAST_DAY; d++) { xs.push((d - 1) * DAY); ys.push(perDay.get(d) || 0); }
  lineChart($("#ck-day"), xs, [{ name: "choices", color: css("--c-culture"), ys, step: true }], { min: 0, onClick: t => { location.hash = `#/day/${dayOf(t)}`; } });
}

// ---------------------------------------------------------------------------------------------
// Compare: two colonies (a branch and its original) side by side.

function compareView() {
  if (!B) { view.innerHTML = `<div class="checks"><p>No second colony was written. In the lab: <code>colony_branch</code>, then <code>colony_explorer {"compare": "&lt;branch id&gt;"}</code>.</p></div>`; return; }
  const a = D.log, b = DB.log;
  let k = 0; while (k < a.length && k < b.length && a[k].text === b[k].text && a[k].t === b[k].t) k++;
  const SB = B.settlers;
  let h = `<div class="checks"><h1>${esc(M.name)}: two tellings</h1><p class="muted">A: <b>${esc(M.id)}</b>${M.script.length ? ` (${M.script.length} acts)` : ""}. B: <b>${esc(B.meta.id)}</b>${B.meta.script.length ? ` (acts: ${B.meta.script.map(esc).join("; ")})` : ""}.</p>`;
  h += k < Math.max(a.length, b.length) ? `<h2>Where they part</h2><p>The logs agree for ${k} lines, to ${stamp(a[Math.max(0, k - 1)]?.t ?? 0)}. Then:</p><div class="log"><p><span class="t">A ${a[k] ? clock(a[k].t) : ""}</span>${a[k] ? `${dayLink(a[k].t, "day " + dayOf(a[k].t))} ${linkNames(a[k].text)}` : "<i>the log ends</i>"}</p><p><span class="t">B ${b[k] ? clock(b[k].t) : ""}</span>${b[k] ? `day ${dayOf(b[k].t)} ${esc(b[k].text)}` : "<i>the log ends</i>"}</p></div>` : `<h2>The same story</h2><p>All ${a.length} lines agree.</p>`;
  const count = (d) => { const Cc = d.C; return { alive: Cc.settlers.filter(s => s.alive).length, of: Cc.settlers.length, dead: Cc.settlers.filter(s => !s.alive && /died|killed|slain|starv|lost|carried|fell/i.test(s.fate)).length, works: Cc.works.filter(w => w.done != null).length, made: Cc.made.length, moments: d.moments.length, major: d.moments.filter(m => m.major).length, lines: d.log.length, food: Cc.series.rows.at(-1)?.[0] }; };
  const ca = count(D), cb = count(DB);
  h += `<h2>Counts</h2><table class="list"><tr><th></th><th>A</th><th>B</th></tr>${[["alive", `${ca.alive} of ${ca.of}`, `${cb.alive} of ${cb.of}`], ["dead or lost", ca.dead, cb.dead], ["works standing", ca.works, cb.works], ["things made", ca.made, cb.made], ["moments (great)", `${ca.moments} (${ca.major})`, `${cb.moments} (${cb.major})`], ["log lines", ca.lines, cb.lines], ["meals stored at the end", ca.food, cb.food]].map(r => `<tr><td>${r[0]}</td><td>${r[1]}</td><td${r[1] !== r[2] ? ' class="flag"' : ""}>${r[2]}</td></tr>`).join("")}</table>`;
  h += `<h2>Over time</h2><div class="chart"><h3>Alive</h3><canvas id="cp-alive"></canvas></div><div class="chart"><h3>Meals stored</h3><canvas id="cp-food"></canvas></div><div class="chart"><h3>Works standing</h3><canvas id="cp-works"></canvas></div>`;
  h += `<h2>Fates</h2><table class="list"><tr><th>Settler</th><th>A</th><th>B</th></tr>${S.map((s, i) => { const fb = SB.find(x => x.name === s.name); return `<tr><td>${who(i)}</td><td>${esc(s.fate)}</td><td${fb && fb.fate !== s.fate ? ' class="flag"' : ""}>${esc(fb?.fate ?? "not in B")}</td></tr>`; }).join("")}${SB.filter(x => !S.some(s => s.name === x.name)).map(x => `<tr><td>${esc(x.name)}</td><td class="muted">not in A</td><td>${esc(x.fate)}</td></tr>`).join("")}</table>`;
  const order = d => d.C.works.filter(w => w.done != null).sort((x, y) => x.done - y.done).map(w => `${w.word} (day ${dayOf(w.done)})`);
  const oa = order(D), ob = order(DB);
  h += `<h2>Works in the order they stood</h2><table class="list"><tr><th>A</th><th>B</th></tr>${Array.from({ length: Math.max(oa.length, ob.length) }, (_, j) => `<tr><td>${esc(oa[j] ?? "")}</td><td${oa[j] !== ob[j] ? ' class="flag"' : ""}>${esc(ob[j] ?? "")}</td></tr>`).join("")}</table>`;
  const key = m => m.title + "|" + dayOf(m.t);
  const ka = new Set(D.moments.map(key)), kb = new Set(DB.moments.map(key));
  h += `<h2>Moments in one telling only</h2><div class="cols"><div><h3>Only in A</h3>${D.moments.filter(m => !kb.has(key(m))).map(m => `<p>${dayLink(m.t, "day " + dayOf(m.t))} ${esc(m.title)}</p>`).join("") || "<p class='muted'>none</p>"}</div><div><h3>Only in B</h3>${DB.moments.filter(m => !ka.has(key(m))).map(m => `<p>day ${dayOf(m.t)} ${esc(m.title)}</p>`).join("") || "<p class='muted'>none</p>"}</div></div></div>`;
  view.innerHTML = h;
  const both = (id, f) => { const xa = C.series.ticks, xb = B.series.ticks; const t0 = Math.min(xa[0] ?? 0, xb[0] ?? 0), t1 = Math.max(xa.at(-1) ?? 1, xb.at(-1) ?? 1);
    const cv = $(id); const merged = [...new Set([...xa, ...xb])].sort((x, y) => x - y);
    const pick = (xs, ys) => merged.map(t => { const k = at(xs, t); return t >= xs[0] && t <= xs.at(-1) ? ys[k] : null; });
    lineChart(cv, merged, [{ name: "A", color: css("--c-trade"), ys: pick(xa, D.series[f]), width: 1.8 }, { name: "B", color: css("--rubric"), ys: pick(xb, DB.series[f]), width: 1.4 }], { t0, t1 }); };
  both("#cp-alive", "alive"); both("#cp-food", "food"); both("#cp-works", "works_done");
}

route();
