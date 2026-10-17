// The history explorer (src/lab/explorer.rs writes it with data.js beside it).
// Views: Legends (the reader), Timeline, Causes, Family, Connections, Map, Checks.
// No outside libraries: SVG and canvas by hand, so the folder opens from disk.
"use strict";

const H = window.HISTORY;
const $ = (s, el = document) => el.querySelector(s);
const esc = s => String(s ?? "").replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const SEASONS = ["spring", "summer", "autumn", "winter"];
const css = name => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

window.addEventListener("error", e => { document.body.dataset.error = (document.body.dataset.error || "") + e.message + " @" + e.lineno + "; "; });

// ---------------------------------------------------------------------------------------------
// Data and indexes

const E = H.events.map(a => ({ id: a[0], year: a[1], season: a[2], kind: H.kinds[a[3]], t: a[4], d: a[5], x: a[6], y: a[7], major: !!a[8], parts: a[9], peoples: a[10], causes: a[11], trig: a[12] }));
const EV = new Map(E.map(e => [e.id, e]));
const EFFECTS = new Map();   // event id -> ids of events it caused or triggered
const BY = new Map();        // entity code -> events naming it (in order)
const AT = new Map();        // "x,y" -> events there
for (const e of E) {
  for (const c of e.trig >= 0 && !e.causes.includes(e.trig) ? [...e.causes, e.trig] : e.causes) {
    if (!EFFECTS.has(c)) EFFECTS.set(c, []);
    EFFECTS.get(c).push(e.id);
  }
  const codes = new Set([...e.parts, ...e.peoples.map(f => "F" + f)]);
  for (const c of codes) { if (!BY.has(c)) BY.set(c, []); BY.get(c).push(e); }
  if (e.x >= 0) { const k = e.x + "," + e.y; if (!AT.has(k)) AT.set(k, []); AT.get(k).push(e); }
}
const causesOf = e => (e.trig >= 0 && !e.causes.includes(e.trig) ? [...e.causes, e.trig] : e.causes).map(id => EV.get(id)).filter(Boolean);
const effectsOf = e => (EFFECTS.get(e.id) || []).map(id => EV.get(id)).filter(Boolean);

const TYPES = {
  F: { list: H.peoples, word: "people", plural: "Peoples", legends: "people" },
  S: { list: H.towns, word: "town", plural: "Towns", legends: "site" },
  P: { list: H.figures, word: "person", plural: "People of note", legends: "figure" },
  H: { list: H.houses, word: "house", plural: "Houses", legends: "house" },
  W: { list: H.wars, word: "war", plural: "Wars", legends: "war" },
  B: { list: H.beasts, word: "beast", plural: "Beasts", legends: "beast" },
  A: { list: H.artifacts, word: "artifact", plural: "Artifacts", legends: "artifact" },
  M: { list: H.monuments, word: "monument", plural: "Monuments", legends: "monument" },
  R: { list: H.faiths, word: "faith", plural: "Faiths", legends: "religion" },
  G: { list: H.gods, word: "god", plural: "Gods", legends: "god" },
  C: { list: H.pops, word: "population", plural: "Populations", legends: null },
  L: { list: H.lands || [], word: "land", plural: "Lands", legends: null },
};
const ENT = new Map();
for (const [t, def] of Object.entries(TYPES)) for (const o of def.list) ENT.set(t + o.i, o);
const ent = code => ENT.get(code);
const nameOf = code => ent(code)?.n ?? code;
const people = id => ENT.get("F" + id);
const peopleColor = id => people(id)?.c ?? css("--ink-faint");
const townAt = new Map(H.towns.map(t => [t.x + "," + t.y, t]));
const warOf = new Map();     // event id -> war
for (const w of H.wars) for (const id of [w.decl, ...w.b, ...w.g]) if (id != null) warOf.set(id, w);
const figurePeople = new Map(H.figures.map(f => [f.i, f.p]));

// Categories of events, by kind.
const CATS = [
  ["war", "War", /War|Battle|Siege|Raid|Conquest|Shadow|Army|Rebellion|Revolt|Massacre|Coup|Ambush|Sack|Invasion|Skirmish/],
  ["beast", "Beasts and quests", /Monster|Creature|Beast|Lair|Quest|Dragon|Hunt|Brood/],
  ["politics", "Rule and dealings", /Treaty|Alliance|Crown|Ruler|Succession|Assassin|Abdicat|Marriage|Peace|Tribute|Diplomac|Vassal|Dispute|Quarrel|Insult|Exile|Defect|Election|Regent|Heir/],
  ["town", "Towns", /Settlement|Founded|Abandon|Destroyed|Town|City|Colony|Capital|Migration|Refugee/],
  ["faith", "Faith", /Relig|Convert|Conversion|Miracle|Schism|Temple|Prophet|Cult|Heresy|Deity|Holy|Pilgrim/],
  ["culture", "Things made", /Artifact|Monument|Art|Book|Song|Poem|Craft|Built|Construct|Discovery|Invention/],
  ["trade", "Trade", /Trade|Caravan|Market/],
  ["nature", "Land and sky", /Disaster|Plague|Famine|Forest|Game|Wildlife|Scar|Celestial|Flood|Drought|Earthquake|Eruption|Storm|Fire|Ecology/],
  ["people", "Lives", /Hero|Born|Birth|Died|Death|Figure|Moved|Child|Mentor|Rival|Duel/],
];
const catCache = new Map();
function cat(kind) {
  if (catCache.has(kind)) return catCache.get(kind);
  const c = (CATS.find(c => c[2].test(kind)) || ["other"])[0];
  catCache.set(kind, c);
  return c;
}
const catColor = c => css("--c-" + c);
const CAT_NAMES = Object.fromEntries([...CATS.map(c => [c[0], c[1]]), ["other", "Other"]]);
const kindWords = k => k.replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase();

const ageOf = y => H.ages.findIndex(a => y >= a[1] && (a[2] == null || y <= a[2]));
const FIRST = H.meta.first, LAST = H.meta.last;
const lifeYears = (b, d) => d == null ? `b. ${b}` : `${b}–${d}`;

// The tiles of each named land, and the lands a tile lies in.
let LAND_TILES = null;
function landTiles() {
  if (LAND_TILES) return LAND_TILES;
  LAND_TILES = new Map();
  for (const layer of Object.values(H.land_layers || {})) layer.forEach((v, i) => { if (v < 0) return; if (!LAND_TILES.has(v)) LAND_TILES.set(v, []); LAND_TILES.get(v).push(i); });
  return LAND_TILES;
}
// The events of a thing: those naming it, or for a land those that happened on it.
function eventsOf(code) {
  if (code[0] !== "L") return BY.get(code) || [];
  const tiles = new Set(landTiles().get(+code.slice(1)) || []);
  return E.filter(e => e.x >= 0 && tiles.has(e.y * H.meta.w + e.x));
}
const landsAt = (x, y) => Object.values(H.land_layers || {}).map(l => l[y * H.meta.w + x]).filter(v => v >= 0).map(v => ENT.get("L" + v)).filter(Boolean);

// A small map on a page: the map cropped round a place, a land's tiles washed in rubric.
function drawMinimaps(root) {
  for (const cv of root.querySelectorAll("canvas.minimap")) {
    const draw = () => {
      const { g, w, h } = fitCanvas(cv);
      const W = H.meta.w, Ht = H.meta.h, x = +cv.dataset.x, y = +cv.dataset.y;
      let tiles = null, span = 18;
      if (cv.dataset.tiles) {
        tiles = landTiles().get(+cv.dataset.tiles.slice(1)) || [];
        let x0 = W, x1 = 0, y0 = Ht, y1 = 0;
        for (const i of tiles) { const tx = i % W, ty = Math.floor(i / W); x0 = Math.min(x0, tx); x1 = Math.max(x1, tx); y0 = Math.min(y0, ty); y1 = Math.max(y1, ty); }
        span = Math.max(14, (x1 - x0) * 1.3, (y1 - y0) * 1.3 * w / h);
      }
      const k = w / span, cx = x + 0.5, cy = y + 0.5;
      g.fillStyle = "#e4d6b8"; g.fillRect(0, 0, w, h);
      g.save(); g.translate(w / 2 - cx * k, h / 2 - cy * k); g.scale(k, k);
      if (MAP_IMG && MAP_IMG.complete && MAP_IMG.naturalWidth) g.drawImage(MAP_IMG, 0, 0, W, Ht);
      if (tiles) { g.fillStyle = "rgba(156,51,36,0.35)"; for (const i of tiles) g.fillRect(i % W, Math.floor(i / W), 1, 1); }
      g.restore();
      for (const t of H.towns) {
        const px = w / 2 + (t.x + 0.5 - cx) * k, py = h / 2 + (t.y + 0.5 - cy) * k;
        if (px < -5 || py < -5 || px > w + 5 || py > h + 5) continue;
        const me = t.x === x && t.y === y && !tiles;
        g.fillStyle = t.d != null ? "#9a8770" : peopleColor(t.p); g.strokeStyle = "#382a20"; g.lineWidth = me ? 2 : 1;
        g.beginPath(); g.arc(px, py, me ? 6 : 3.5, 0, Math.PI * 2); g.fill(); g.stroke();
        if (k > 14 || me) { g.font = `${me ? "600 " : ""}12px ${css("--body")}`; g.lineWidth = 3; g.strokeStyle = "rgba(246,239,223,0.9)"; g.strokeText(t.n, px + 7, py + 4); g.fillStyle = "#382a20"; g.fillText(t.n, px + 7, py + 4); }
      }
      g.strokeStyle = "#c8b48f"; g.lineWidth = 1; g.strokeRect(0.5, 0.5, w - 1, h - 1);
    };
    if (!MAP_IMG && H.meta.map) { MAP_IMG = new Image(); MAP_IMG.src = H.meta.map.file; }
    if (MAP_IMG && !MAP_IMG.complete) MAP_IMG.addEventListener("load", draw, { once: true });
    draw();
  }
}

// ---------------------------------------------------------------------------------------------
// Links and linked text

const href = code => code[0] === "E" ? `#/event/${code.slice(1)}` : `#/page/${code}`;
const link = (code, text) => ent(code) || code[0] === "E" ? `<a href="${href(code)}">${esc(text ?? nameOf(code))}</a>` : esc(text ?? nameOf(code));
const evLink = e => `<a href="#/event/${e.id}">${esc(e.t)}</a>`;
const legendsUrl = code => {
  const t = TYPES[code[0]];
  if (!H.meta.legends || !t || !t.legends) return null;
  return `legends/${t.legends}-${code.slice(1)}.html`;
};

// The event's text with the names of what it names linked (longest names first, once each).
function linked(e, text) {
  const codes = new Set([...e.parts, ...e.peoples.map(f => "F" + f)]);
  const t = townAt.get(e.x + "," + e.y); if (t) codes.add("S" + t.i);
  const w = warOf.get(e.id); if (w) codes.add("W" + w.i);
  const names = [];
  for (const c of codes) { const o = ent(c); if (o && o.n && o.n.length > 2) names.push([o.n, c]); }
  names.sort((a, b) => b[0].length - a[0].length);
  let out = esc(text);
  const slots = [];
  for (const [n, c] of names) {
    const en = esc(n);
    const i = out.indexOf(en);
    if (i < 0) continue;
    slots.push(`<a href="${href(c)}">${en}</a>`);
    out = out.slice(0, i) + `\u0001${slots.length - 1}\u0002` + out.slice(i + en.length);
  }
  return out.replace(/\u0001(\d+)\u0002/g, (_, k) => slots[+k]);
}

// One event as a line of the chronicle.
function evLine(e, opts = {}) {
  const c = cat(e.kind);
  const why = causesOf(e).length ? `<a class="why" href="#/causes/${e.id}" title="the causal graph">why?</a>` : "";
  const body = e.d && e.d !== e.t ? ` ${linked(e, e.d)}` : "";
  const yr = opts.year ? `<span class="muted">${e.year}</span> ` : "";
  return `<p class="${e.major ? "major" : ""}"><span class="dot" style="background:${catColor(c)}" title="${esc(kindWords(e.kind))}"></span>${yr}<b><a href="#/event/${e.id}">${linked(e, e.t).replace(/<\/?a[^>]*>/g, "")}</a>.</b>${body}${why}</p>`;
}

// Events grouped by year, as the legends tell them.
function chronicle(evs, limit = 400) {
  if (!evs.length) return `<p class="muted">Nothing is written.</p>`;
  let out = `<div class="chron">`, last = null;
  for (const e of evs.slice(0, limit)) {
    if (e.year !== last) { out += `<span class="yr"><a href="#/year/${e.year}">${e.year}</a></span>`; last = e.year; }
    out += evLine(e);
  }
  if (evs.length > limit) out += `<p class="muted">… and ${evs.length - limit} more.</p>`;
  return out + `</div>`;
}

// ---------------------------------------------------------------------------------------------
// Router

const view = $("#view");
let cleanup = null;
function route() {
  if (cleanup) { cleanup(); cleanup = null; }
  closeTip();
  const [path, query] = location.hash.replace(/^#\/?/, "").split("?");
  const parts = path.split("/").filter(Boolean);
  const q = Object.fromEntries(new URLSearchParams(query || ""));
  const tab = { timeline: "timeline", causes: "causes", family: "family", web: "web", map: "map", checks: "checks", compare: "compare" }[parts[0]] || "read";
  for (const a of document.querySelectorAll("#tabs a")) a.classList.toggle("on", a.dataset.tab === tab);
  if (tab !== "read") $("#drawer").hidden = true;
  switch (parts[0]) {
    case "timeline": return timelineView(q);
    case "causes": return causesView(parts[1] ? +parts[1] : null, q);
    case "family": return familyView(parts[1] || null, q);
    case "web": return webView(parts[1] || null, q);
    case "map": return mapView(q);
    case "checks": return checksView();
    case "compare": return compareView();
    default: return readView(parts, q);
  }
}
window.addEventListener("hashchange", route);

// The drawer: a page beside a graph, so the graph stays where it is.
function openDrawer(html) {
  const d = $("#drawer");
  $("#drawer-body").innerHTML = html;
  d.hidden = false;
  drawMinimaps($("#drawer-body"));
  d.scrollTop = 0;
}
$("#drawer-close").onclick = () => { $("#drawer").hidden = true; };
// Links in the drawer to pages and events open in the drawer while a graph is shown.
$("#drawer").addEventListener("click", ev => {
  const a = ev.target.closest("a"); if (!a) return;
  const m = a.getAttribute("href")?.match(/^#\/(page|event|year)\/(.+)$/);
  if (m && !location.hash.match(/^#\/?($|page|event|year|age)/)) { ev.preventDefault(); openDrawer(pageHtml(m[1], m[2])); }
});

// Tooltips.
const tip = $("#tip");
function showTip(html, x, y) {
  tip.innerHTML = html; tip.hidden = false;
  const w = tip.offsetWidth, h = tip.offsetHeight;
  tip.style.left = Math.min(x + 14, innerWidth - w - 8) + "px";
  tip.style.top = (y + h + 18 > innerHeight ? y - h - 10 : y + 14) + "px";
}
function closeTip() { tip.hidden = true; }

// Theme.
$("#theme").onclick = () => {
  const root = document.documentElement;
  const dark = root.dataset.theme ? root.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
  root.dataset.theme = dark ? "light" : "dark";
  try { localStorage.setItem("annals-theme", root.dataset.theme); } catch (_) {}
  route();
};
try { const t = localStorage.getItem("annals-theme"); if (t) document.documentElement.dataset.theme = t; } catch (_) {}
{ const t = new URLSearchParams(location.search).get("theme"); if (t === "light" || t === "dark") document.documentElement.dataset.theme = t; }

// Search: entities first, then events.
const SEARCH = [];
for (const [code, o] of ENT) if (o.n) SEARCH.push([o.n.toLowerCase(), code, o.n, TYPES[code[0]].word]);
const q = $("#q"), hits = $("#hits");
let hitSel = -1;
q.addEventListener("input", () => {
  const s = q.value.trim().toLowerCase();
  hitSel = -1;
  if (s.length < 2) { hits.innerHTML = ""; return; }
  const ents = SEARCH.filter(r => r[0].includes(s)).sort((a, b) => (a[0].startsWith(s) ? 0 : 1) - (b[0].startsWith(s) ? 0 : 1) || a[0].length - b[0].length).slice(0, 14);
  const evs = E.filter(e => e.t.toLowerCase().includes(s)).slice(0, 10);
  hits.innerHTML = ents.map(r => `<li><a href="${href(r[1])}">${esc(r[2])}<span>${r[3]}</span></a></li>`).join("")
    + evs.map(e => `<li><a href="#/event/${e.id}">${esc(e.t)}<span>${e.year}</span></a></li>`).join("");
});
q.addEventListener("keydown", ev => {
  const items = [...hits.querySelectorAll("a")];
  if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
    ev.preventDefault();
    hitSel = (hitSel + (ev.key === "ArrowDown" ? 1 : -1) + items.length) % Math.max(1, items.length);
    items.forEach((a, i) => a.classList.toggle("sel", i === hitSel));
  } else if (ev.key === "Enter" && items.length) { location.hash = items[Math.max(0, hitSel)].getAttribute("href"); hits.innerHTML = ""; q.blur(); }
  else if (ev.key === "Escape") { hits.innerHTML = ""; q.blur(); }
});
hits.addEventListener("click", () => { hits.innerHTML = ""; });
document.addEventListener("click", ev => { if (!ev.target.closest(".find")) hits.innerHTML = ""; });

// Pan and zoom for an SVG by its viewBox.
function panZoom(svg, box, onChange) {
  let vb = { ...box }, drag = null;
  const set = () => { svg.setAttribute("viewBox", `${vb.x} ${vb.y} ${vb.w} ${vb.h}`); onChange && onChange(vb); };
  set();
  svg.addEventListener("wheel", ev => {
    ev.preventDefault();
    const r = svg.getBoundingClientRect();
    const k = Math.exp(ev.deltaY * 0.0015);
    const mx = vb.x + (ev.clientX - r.left) / r.width * vb.w, my = vb.y + (ev.clientY - r.top) / r.height * vb.h;
    vb = { x: mx - (mx - vb.x) * k, y: my - (my - vb.y) * k, w: vb.w * k, h: vb.h * k };
    set();
  }, { passive: false });
  svg.addEventListener("pointerdown", ev => { if (ev.button !== 0) return; drag = { x: ev.clientX, y: ev.clientY, vb: { ...vb }, moved: false }; });
  window.addEventListener("pointermove", ev => {
    if (!drag) return;
    const r = svg.getBoundingClientRect();
    const dx = (ev.clientX - drag.x) / r.width * vb.w, dy = (ev.clientY - drag.y) / r.height * vb.h;
    if (Math.abs(ev.clientX - drag.x) + Math.abs(ev.clientY - drag.y) > 3) drag.moved = true;
    vb = { ...vb, x: drag.vb.x - dx, y: drag.vb.y - dy };
    set();
  });
  window.addEventListener("pointerup", () => { if (drag) svg.dataset.dragged = drag.moved ? "1" : ""; drag = null; });
  return { fit: b => { vb = { ...b }; set(); } };
}
const svgEl = (tag, attrs = {}, text) => {
  const el = document.createElementNS("http://www.w3.org/2000/svg", tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
  if (text != null) el.textContent = text;
  return el;
};
const clip = (s, n) => s.length > n ? s.slice(0, n - 1) + "…" : s;

// The view box that shows `box` (graph units) in the part of the stage the drawer leaves free,
// centred on (cx, cy), shrunk no further than `maxScale` units a pixel (bigger graphs are panned).
function fitBox(svg, box, cx, cy, maxScale = 1.7) {
  const r = svg.getBoundingClientRect();
  const free = Math.max(320, r.width - Math.min(480, innerWidth * 0.92));
  const s = Math.min(maxScale, Math.max(box.w / free, box.h / r.height, 0.8));
  return { x: cx - (free / 2) * s, y: cy - (r.height / 2) * s, w: r.width * s, h: r.height * s };
}

// ---------------------------------------------------------------------------------------------
// The reader (Legends): an index on the left, a page on the right.

let indexCat = "F";
function readView(parts, q) {
  view.innerHTML = `<div class="split"><nav class="index" id="index"></nav><article class="reader"><div class="page" id="page"></div></article></div>`;
  drawIndex();
  const kind = parts[0] || "home";
  $("#page").innerHTML = pageHtml(kind, parts[1]);
  $(".reader").scrollTop = 0;
  drawMinimaps($("#page"));
}

function drawIndex() {
  const idx = $("#index");
  const cats = Object.entries(TYPES).filter(([, d]) => d.list.length).map(([t, d]) => `<button data-c="${t}" class="${t === indexCat ? "on" : ""}">${d.plural} ${d.list.length}</button>`).join("")
    + `<button data-c="Y" class="${indexCat === "Y" ? "on" : ""}">Ages</button>`;
  let list = "";
  if (indexCat === "Y") {
    list = H.ages.map((a, i) => `<li><a href="#/age/${i}">${esc(a[0])}</a><span>${a[1]}–${a[2] ?? "now"}</span></li>`).join("");
  } else {
    const items = [...TYPES[indexCat].list];
    const by = {
      F: o => -(H.towns.filter(t => t.p === o.i && t.d == null).length * 1000 + (BY.get("F" + o.i)?.length || 0)),
      P: o => -(BY.get("P" + o.i)?.length || 0), S: o => -(o.pop || 0), W: o => o.s, L: o => -o.size,
    }[indexCat];
    if (by) items.sort((a, b) => by(a) - by(b)); else items.sort((a, b) => a.n.localeCompare(b.n));
    list = items.slice(0, 600).map(o => {
      const code = indexCat + o.i;
      const side = { F: () => (o.d ? `${o.f}–${o.d}` : `since ${o.f}`), S: () => (o.d ? "ruin" : (o.pop || "")), P: () => lifeYears(o.b, o.d), W: () => `${o.s}–${o.e ?? ""}`, H: () => `${o.m.length}`, L: () => kindWords(o.k) }[indexCat];
      return `<li><a href="#/page/${code}">${esc(o.n)}</a><span>${side ? esc(side()) : ""}</span></li>`;
    }).join("");
    if (items.length > 600) list += `<li class="muted">… ${items.length - 600} more (search above)</li>`;
  }
  idx.innerHTML = `<div class="cats">${cats}</div><ol>${list}</ol>`;
  idx.querySelectorAll(".cats button").forEach(b => b.onclick = () => { indexCat = b.dataset.c; drawIndex(); });
}

function actions(list) {
  return `<div class="actions">${list.filter(Boolean).map(([h, t]) => `<a href="${h}"${h.startsWith("legends/") ? ' target="_blank"' : ""}>${t}</a>`).join("")}</div>`;
}
function facts(rows) {
  return `<dl class="facts">${rows.filter(r => r && r[1] != null && r[1] !== "").map(([k, v]) => `<dt>${k}</dt><dd>${v}</dd>`).join("")}</dl>`;
}
const legendsAction = code => { const u = legendsUrl(code); return u ? [u, "Full legends page ↗"] : null; };

function pageHtml(kind, arg) {
  switch (kind) {
    case "home": return homePage();
    case "page": return entityPage(arg);
    case "event": return eventPage(+arg);
    case "year": return yearPage(+arg);
    case "age": return agePage(+arg);
    default: return `<p>Unknown page.</p>`;
  }
}

function homePage() {
  const m = H.meta;
  const living = H.towns.filter(t => t.d == null).length;
  const great = E.filter(e => e.major);
  let o = `<p class="eyebrow">the annals of</p><h1>${esc(m.land)}</h1><p class="lede">${esc(m.sentence)}</p>`;
  o += facts([["Years", `${FIRST} to ${LAST}`], ["Written", `${E.length} events, ${great.length} of them great`], ["Peoples", `${H.peoples.length} (${H.peoples.filter(p => p.d == null).length} endure)`],
    ["Towns", `${H.towns.length} (${living} standing, ${H.towns.length - living} in ruins)`], ["Wars", H.wars.length], ["People of note", H.figures.length], ["Today", esc(m.present)], ["World", `${esc(m.id)}, seed ${m.seed}, ${m.w}×${m.h}`]]);
  o += actions([["#/timeline", "Timeline"], ["#/web", "Peoples and their dealings"], ["#/map", "The map through the years"], ["#/checks", "Checks"], H.meta.legends ? ["legends/index.html", "Full legends ↗"] : null]);
  o += `<h2>The ages</h2>`;
  o += H.ages.map((a, i) => `<p><a href="#/age/${i}">${esc(a[0])}</a> <span class="muted">${a[1]}–${a[2] ?? "now"}</span></p>`).join("") || `<p class="muted">No ages are named.</p>`;
  if (H.tales.length) {
    o += `<h2>Tales worth telling</h2>`;
    o += H.tales.slice(0, 12).map(t => `<div class="tale"><b>${esc(t.t)}</b> <span class="muted">${t.y}</span><br>${esc(t.x)} ${t.e.length ? `<a class="why" href="#/causes/${t.e[t.e.length - 1]}">the chain</a>` : ""}</div>`).join("");
  }
  o += `<h2>The great events</h2>` + chronicle(great, 120);
  return o;
}

function eventPage(id) {
  const e = EV.get(id);
  if (!e) return `<p>No event #${id}.</p>`;
  const c = cat(e.kind);
  const town = townAt.get(e.x + "," + e.y);
  const w = warOf.get(e.id);
  const age = ageOf(e.year);
  let o = `<p class="eyebrow"><span class="dot" style="background:${catColor(c)}"></span>${esc(kindWords(e.kind))} · event ${e.id}</p><h1>${linked(e, e.t)}</h1>`;
  o += `<p class="lede">In the ${SEASONS[e.season]} of <a href="#/year/${e.year}">${e.year}</a>${age >= 0 ? `, in <a href="#/age/${age}">${esc(H.ages[age][0])}</a>` : ""}${e.x >= 0 ? `, at ${town ? link("S" + town.i) : `${e.x},${e.y}`}` : ""}.</p>`;
  if (e.d && e.d !== e.t) o += `<p>${linked(e, e.d)}</p>`;
  o += actions([[`#/causes/${e.id}`, "Causal graph"], e.x >= 0 ? [`#/map?year=${e.year}&x=${e.x}&y=${e.y}`, "On the map"] : null, [`#/timeline?year=${e.year}`, "On the timeline"],
    H.meta.legends ? [`legends/year-${e.year}.html#e${e.id}`, "In the legends ↗"] : null]);
  const who = [...e.parts.map(c => link(c)), ...e.peoples.map(f => link("F" + f))];
  o += facts([["Named", who.join(", ") || null], ["Part of", w ? link("W" + w.i) : null], ["In", e.x >= 0 ? landsAt(e.x, e.y).map(l => link("L" + l.i)).join(", ") || null : null], ["Major", e.major ? "yes" : null]]);
  const tale = H.tales.find(t => t.e.includes(e.id));
  if (tale) o += `<h2>A tale worth telling</h2><div class="tale"><b>${esc(tale.t)}</b><br>${esc(tale.x)}</div>`;
  // The chain back, as far as it goes (five links), then what came of it.
  const back = []; let cur = e; const seen = new Set([e.id]);
  for (let k = 0; k < 6; k++) { const cs = causesOf(cur).filter(x => !seen.has(x.id)); if (!cs.length) break; cur = cs[0]; seen.add(cur.id); back.push([cur, cs.length - 1]); }
  if (back.length) {
    o += `<h2>Because</h2><div class="chain">`;
    back.forEach(([x, more], k) => { o += `<div style="margin-left:${k * 0.8}rem"><span class="muted">${x.year}</span> ${evLink(x)}${more ? ` <span class="muted">(and ${more} other cause${more > 1 ? "s" : ""})</span>` : ""}</div>`; });
    o += `</div>`;
  } else o += `<p class="muted">No cause is recorded.</p>`;
  const fx = effectsOf(e);
  if (fx.length) o += `<h2>It led to</h2>` + chronicle(fx, 60);
  return o;
}

function yearPage(y) {
  const evs = E.filter(e => e.year === y);
  const age = ageOf(y);
  const years = [...new Set(E.map(e => e.year))];
  const i = years.indexOf(y);
  let o = `<p class="eyebrow">${age >= 0 ? `<a href="#/age/${age}">${esc(H.ages[age][0])}</a>` : "the record"}</p><h1>The Year ${y}</h1>`;
  o += `<p class="lede">${i > 0 ? `<a href="#/year/${years[i - 1]}">‹ ${years[i - 1]}</a> · ` : ""}${evs.length} events${i >= 0 && i < years.length - 1 ? ` · <a href="#/year/${years[i + 1]}">${years[i + 1]} ›</a>` : ""}</p>`;
  o += actions([[`#/map?year=${y}`, "The map this year"], [`#/timeline?year=${y}`, "On the timeline"]]);
  for (const s of [0, 1, 2, 3]) {
    const sv = evs.filter(e => e.season === s);
    if (sv.length) o += `<h2>${SEASONS[s][0].toUpperCase() + SEASONS[s].slice(1)}</h2><div class="chron">${sv.map(e => evLine(e)).join("")}</div>`;
  }
  return o;
}

function agePage(i) {
  const a = H.ages[i];
  if (!a) return `<p>No such age.</p>`;
  const evs = E.filter(e => e.year >= a[1] && (a[2] == null || e.year <= a[2]));
  const wars = H.wars.filter(w => w.s >= a[1] && (a[2] == null || w.s <= a[2]));
  const count = re => evs.filter(e => re.test(e.kind)).length;
  let o = `<p class="eyebrow">age ${i + 1} of ${H.ages.length}</p><h1>${esc(a[0])}</h1><p class="lede">From ${a[1]} to ${a[2] ?? "the present"}: ${evs.length} events, ${count(/BattleFought/)} battles, ${count(/SiegeBegun/)} sieges, ${count(/SettlementDestroyed/)} towns razed, ${count(/SettlementFounded/)} founded.</p>`;
  o += actions([[`#/timeline?from=${a[1]}&to=${a[2] ?? LAST}`, "This age on the timeline"], [`#/web?from=${a[1]}&to=${a[2] ?? LAST}`, "Dealings in this age"], [`#/map?year=${a[2] ?? LAST}`, "The map at its end"]]);
  if (wars.length) o += `<h2>Wars begun in it</h2><div class="cols">${wars.map(w => `<p>${link("W" + w.i)} <span class="muted">${w.s}–${w.e ?? ""}</span></p>`).join("")}</div>`;
  o += `<h2>Its great events</h2>` + chronicle(evs.filter(e => e.major), 200);
  return o;
}

// The page of a person, people, town, war, house, beast, thing or faith.
function entityPage(code) {
  const o = ent(code);
  if (!o) return `<p>Nothing is known of ${esc(code)}.</p>`;
  const t = code[0], def = TYPES[t];
  const evs = BY.get(code) || [];
  let head = `<p class="eyebrow">${def.word}</p><h1>${esc(o.n)}</h1>`;
  let body = "";
  const acts = [];
  if (t === "F") {
    const towns = H.towns.filter(x => x.p === o.i);
    const standing = towns.filter(x => x.d == null), fallen = H.towns.filter(x => x.own.some(r => r[0] === o.i) && (x.d != null || x.p !== o.i));
    const wars = H.wars.filter(w => w.a.includes(o.i) || w.df.includes(o.i));
    const crowned = evs.filter(e => /Crown/.test(e.kind));
    const houses = [...new Set(H.figures.filter(f => f.p === o.i && f.h != null).map(f => f.h))];
    head += `<p class="lede"><span class="swatch" style="background:${o.c}"></span>${esc(o.race ?? "")} ${esc(kindWords(o.gov))}, ${o.d ? `from ${o.f} until it fell in ${o.d}` : `since ${o.f}`}.</p>`;
    acts.push([`#/timeline?focus=${code}`, "On the timeline"], [`#/web?focus=${o.i}`, "Its dealings"], [`#/map?year=${o.d ?? LAST}&focus=${o.i}`, "Its lands on the map"]);
    body += facts([["Seat", o.cap != null ? link("S" + o.cap) : null], ["Ruler", o.lead != null ? link("P" + o.lead) : null], ["House", o.house != null ? link("H" + o.house) : null],
      ["Faith", o.faith != null ? link("R" + o.faith) : null], ["Souls", o.pop || null], ["Wealth", o.wealth || null]]);
    body += `<h2>Towns</h2><div class="cols">${standing.map(x => `<p>${link("S" + x.i)} <span class="muted">${x.pop}</span></p>`).join("") || `<p class="muted">None stand.</p>`}</div>`;
    if (fallen.length) body += `<h3>Lost or fallen</h3><div class="cols">${fallen.map(x => `<p>${link("S" + x.i)} <span class="muted">${x.d ? `razed ${x.d}` : `now ${esc(people(x.p)?.n ?? "")}`}</span></p>`).join("")}</div>`;
    if (crowned.length) body += `<h2>Its rulers</h2><div class="chron">${crowned.map(e => evLine(e, { year: true })).join("")}</div>`;
    if (wars.length) body += `<h2>Its wars</h2><div class="cols">${wars.map(w => `<p>${link("W" + w.i)} <span class="muted">${w.s}–${w.e ?? ""}${w.v != null ? (w.v === o.i ? ", won" : ", lost") : ""}</span></p>`).join("")}</div>`;
    if (houses.length) body += `<h2>Its houses</h2><p>${houses.map(h => link("H" + h)).join(", ")}</p>`;
    const rel = (o.rel || []).filter(r => people(r[0])?.d == null && o.d == null).sort((a, b) => a[1] - b[1]);
    if (rel.length) body += `<h2>How it regards the others today</h2><div class="cols">${rel.map(r => `<p>${link("F" + r[0])} <span class="muted">${r[1] > 0 ? "+" : ""}${r[1]}, ${esc(kindWords(r[2]))}</span></p>`).join("")}</div>`;
  } else if (t === "S") {
    // Consecutive records by the same people read as one holding.
    const runs = [];
    for (const r of o.own) { const l = runs[runs.length - 1]; if (l && l[0] === r[0] && l[2] === r[1]) l[2] = r[2]; else runs.push([...r]); }
    const owners = runs.map(r => `${link("F" + r[0])} <span class="muted">${r[1]}–${r[2] < 0 ? (o.d ?? "now") : r[2]}</span>`);
    const here = (AT.get(o.x + "," + o.y) || []);
    head += `<canvas class="minimap" data-x="${o.x}" data-y="${o.y}"></canvas>`;
    head += `<p class="lede">${esc(kindWords(o.t))} of ${link("F" + o.p)}, founded in ${o.f}${o.d ? `, razed in ${o.d}` : `, ${o.pop} souls`}.</p>`;
    acts.push([`#/map?year=${o.d ?? LAST}&x=${o.x}&y=${o.y}`, "On the map"]);
    body += facts([["Place", `${o.x},${o.y}`], ["In", landsAt(o.x, o.y).map(l => link("L" + l.i)).join(", ") || null], ["Walls", o.walls !== "None" ? esc(kindWords(o.walls)) : null], ["Held by", owners.join("<br>") || null]]);
    const all = [...new Map([...evs, ...here].map(e => [e.id, e])).values()].sort((a, b) => a.year - b.year || a.id - b.id);
    body += `<h2>Its story</h2>` + chronicle(all);
    return head + actions([...acts, legendsAction(code)]) + body;
  } else if (t === "P") {
    const p = o.p != null ? link("F" + o.p) : "no people";
    const kin = (ids) => ids.filter(x => x != null).map(x => link("P" + x)).join(", ");
    head += `<p class="lede">${o.t.length ? esc(o.t.join(", ")) + ", " : ""}of ${p}. ${o.d == null ? `Born in ${o.b}, living.` : `${o.b}–${o.d}${o.dc ? `, ${esc(kindWords(o.dc))}` : ""}.`}</p>`;
    acts.push([`#/family/${code}`, "Family tree"], [`#/web/${code}`, "Connections"]);
    body += facts([["Race", esc(o.race)], ["Parents", kin(o.pa) || null], ["Spouse", o.sp != null ? link("P" + o.sp) : null], ["Children", kin(o.ch) || null], ["House", o.h != null ? link("H" + o.h) : null],
      ["Enemies", kin(o.en) || null], ["Mentors", kin(o.me) || null]]);
  } else if (t === "H") {
    const members = o.m.map(id => ENT.get("P" + id)).filter(Boolean).sort((a, b) => a.b - b.b);
    head += `<p class="lede">Founded in ${o.f} by ${link("P" + o.fo)}; ${members.length} of its blood are remembered.</p>`;
    acts.push([`#/family/P${o.fo}?down=6&up=0`, "Family tree from its founder"]);
    body += facts([["Head", o.head != null ? link("P" + o.head) : null], ["Ruled", o.ruled.map(f => link("F" + f)).join(", ") || null]]);
    body += `<h2>Of its blood</h2><div class="cols">${members.map(f => `<p>${link("P" + f.i)} <span class="muted">${lifeYears(f.b, f.d)}</span></p>`).join("")}</div>`;
  } else if (t === "W") {
    const side = ids => ids.map(f => link("F" + f)).join(", ");
    const fights = [o.decl, ...o.b, ...o.g].filter(x => x != null).map(id => EV.get(id)).filter(Boolean).sort((a, b) => a.year - b.year || a.id - b.id);
    head += `<p class="lede">${side(o.a)} against ${side(o.df)}, ${o.s}–${o.e ?? "unended"}${o.v != null ? `; ${link("F" + o.v)} prevailed` : ""}.</p>`;
    acts.push([`#/timeline?from=${o.s - 5}&to=${(o.e ?? LAST) + 5}`, "On the timeline"], o.decl != null ? [`#/causes/${o.decl}`, "Why it began"] : null, [`#/map?year=${o.e ?? LAST}`, "The map at its end"]);
    body += facts([["Cause", esc(kindWords(o.cause))], ["Battles", o.b.length], ["Sieges", o.g.length]]);
    body += `<h2>Its course</h2>` + chronicle(fights);
    return head + actions([...acts, legendsAction(code)]) + body;
  } else if (t === "L") {
    const tiles = new Set(landTiles().get(o.i) || []);
    const W = H.meta.w;
    const towns = H.towns.filter(x => tiles.has(x.y * W + x.x));
    const here = E.filter(e => e.x >= 0 && tiles.has(e.y * W + e.x));
    const wars = new Map(); for (const e of here) { const w = warOf.get(e.id); if (w) wars.set(w.i, w); }
    const kinds = {}; for (const e of here) { const c = cat(e.kind); kinds[c] = (kinds[c] || 0) + 1; }
    head += `<canvas class="minimap" data-tiles="L${o.i}" data-x="${o.a[0]}" data-y="${o.a[1]}"></canvas>`;
    head += `<p class="lede">${esc(kindWords(o.k))}${o.hm ? `, ${o.hm} m at its highest` : ""}${o.by != null ? `, named by ${link("F" + o.by)}` : ", its name older than the peoples"}; ${tiles.size} tiles across.</p>`;
    acts.push([`#/map?year=${LAST}&x=${o.a[0]}&y=${o.a[1]}`, "On the map"], [`#/timeline?focus=${code}`, "On the timeline"]);
    body += facts([["Towns", towns.length ? `${towns.filter(x => x.d == null).length} standing, ${towns.filter(x => x.d != null).length} in ruins` : null],
      ["Written there", here.length ? Object.entries(kinds).sort((a, b) => b[1] - a[1]).map(([c, n]) => `${n} ${CAT_NAMES[c].toLowerCase()}`).join(", ") : null]]);
    if (towns.length) body += `<h2>Its towns</h2><div class="cols">${towns.map(x => `<p>${link("S" + x.i)} <span class="muted">${x.d ? `ruin, ${x.d}` : esc(people(x.p)?.n ?? "")}</span></p>`).join("")}</div>`;
    if (wars.size) body += `<h2>Wars fought on it</h2><div class="cols">${[...wars.values()].map(w => `<p>${link("W" + w.i)} <span class="muted">${w.s}–${w.e ?? ""}</span></p>`).join("")}</div>`;
    return head + actions(acts) + body + `<h2>What happened there</h2>` + chronicle(here);
  } else if (t === "B") {
    head += `<p class="lede">${o.d ? `Slain in ${o.d}` : "It lives"}${o.lair ? `; its lair at ${o.lair[0]},${o.lair[1]}` : ""}; ${o.kills} killed.</p>`;
  } else if (t === "M") {
    head += `<p class="lede">Raised in ${o.f} at ${townAt.get(o.x + "," + o.y) ? link("S" + townAt.get(o.x + "," + o.y).i) : `${o.x},${o.y}`}.</p>`;
  }
  return head + actions([...acts, legendsAction(code)]) + body + `<h2>In the chronicle</h2>` + chronicle(evs);
}

// ---------------------------------------------------------------------------------------------
// Shared bits for the canvas views.

const hiddenCats = new Set();
function catLegend(onChange) {
  const keys = [...CATS.map(c => c[0]), "other"];
  const el = document.createElement("div");
  el.className = "legend";
  el.innerHTML = keys.map(k => `<label><input type="checkbox" data-k="${k}" ${hiddenCats.has(k) ? "" : "checked"}><span class="dot" style="background:${catColor(k)}"></span>${CAT_NAMES[k]}</label>`).join("");
  el.addEventListener("change", ev => { const k = ev.target.dataset.k; if (ev.target.checked) hiddenCats.delete(k); else hiddenCats.add(k); onChange(); });
  return el;
}
function fitCanvas(cv) {
  const r = cv.getBoundingClientRect(), dpr = devicePixelRatio || 1;
  cv.width = Math.max(1, Math.round(r.width * dpr)); cv.height = Math.max(1, Math.round(r.height * dpr));
  const g = cv.getContext("2d");
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  return { g, w: r.width, h: r.height };
}
const alpha = (hex, a) => { const m = hex.match(/^#?([0-9a-f]{6})$/i); if (!m) return hex; const n = parseInt(m[1], 16); return `rgba(${n >> 16},${(n >> 8) & 255},${n & 255},${a})`; };
function niceStep(pxPerUnit, minPx) { for (const s of [1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500]) if (s * pxPerUnit >= minPx) return s; return 1000; }

// The people an event belongs to (its own peoples, else those of who it names).
function peoplesOfEvent(e) {
  if (e.peoples.length) return e.peoples;
  for (const c of e.parts) {
    if (c[0] === "P") { const p = figurePeople.get(+c.slice(1)); if (p != null) return [p]; }
    if (c[0] === "S") { const t = ent(c); if (t) return [t.p]; }
  }
  const t = townAt.get(e.x + "," + e.y);
  return t ? [t.p] : [];
}

// Tiles held by each people in each year (from the tiles' ownership records), and towns.
let LANDS = null, TOWNS = null;
function landsByYear() {
  if (LANDS) return LANDS;
  const n = LAST - FIRST + 2, delta = new Map();
  for (let i = 0; i < H.territory.length; i += 4) {
    const p = H.territory[i + 1], g = Math.max(FIRST, H.territory[i + 2]), l = H.territory[i + 3];
    if (!delta.has(p)) delta.set(p, new Int32Array(n + 1));
    const d = delta.get(p); d[g - FIRST]++; if (l >= 0 && l <= LAST) d[Math.max(0, l - FIRST)]--;
  }
  LANDS = new Map();
  for (const [p, d] of delta) { const v = new Int32Array(n); let a = 0; for (let k = 0; k < n; k++) { a += d[k]; v[k] = a; } LANDS.set(p, v); }
  return LANDS;
}
function townsByYear() {
  if (TOWNS) return TOWNS;
  const n = LAST - FIRST + 2;
  TOWNS = new Map();
  for (const t of H.towns) for (let y = Math.max(FIRST, t.f); y <= Math.min(LAST, (t.d ?? LAST + 1) - 1); y++) {
    const o = ownerAt(t, y); if (o == null) continue;
    if (!TOWNS.has(o)) TOWNS.set(o, new Int32Array(n));
    TOWNS.get(o)[y - FIRST]++;
  }
  return TOWNS;
}

// ---------------------------------------------------------------------------------------------
// Timeline

function timelineView(q) {
  const focus = q.focus || null;
  const focusSet = focus ? new Set(eventsOf(focus).map(e => e.id)) : null;
  view.innerHTML = `<div class="tool"><div class="toolbar"><span class="title">Timeline</span>
    <label>rows <select id="tl-sort"><option value="f">by founding</option><option value="n">by events</option><option value="t">by towns</option></select></label>
    <label><input type="checkbox" id="tl-major"> great events only</label>
    <label>below <select id="tl-chart"><option value="events">events</option><option value="lands">lands held</option><option value="towns">towns standing</option></select></label>
    ${focus ? `<span>focus: ${link(focus)} <a href="#/timeline">clear</a></span>` : ""}
    <button id="tl-fit">Whole history</button></div><div class="stage"><canvas id="tl"></canvas><div class="hint">wheel zooms · drag pans · shift+wheel scrolls rows · click a mark to read it</div></div></div>`;
  $(".toolbar").appendChild(catLegend(draw));
  const cv = $("#tl");
  const GUTTER = 190, TOP = 38;
  let DENS = 96;
  let rows = [], x0 = FIRST - 2, ppy = 1, sy = 0, hit = [], warHit = [], rowH = 24;
  const evRows = new Map();   // event -> people ids
  for (const e of E) evRows.set(e.id, peoplesOfEvent(e));
  function buildRows() {
    const sort = $("#tl-sort").value;
    const score = { f: p => p.f, n: p => -(BY.get("F" + p.i)?.length || 0), t: p => -H.towns.filter(t => t.own.some(r => r[0] === p.i)).length }[sort];
    rows = [...H.peoples].sort((a, b) => score(a) - score(b) || a.i - b.i).map(p => ({ id: p.i, p }));
    rows.push({ id: null, p: { n: "Beyond the peoples", c: css("--ink-faint"), f: FIRST, d: null } });
  }
  function fit(from = FIRST - 2, to = LAST + 2) {
    const r = cv.getBoundingClientRect();
    ppy = Math.max(0.05, (r.width - GUTTER - 20) / Math.max(1, to - from));
    x0 = from;
  }
  const X = y => GUTTER + (y - x0) * ppy;
  function draw() {
    DENS = $("#tl-chart").value === "events" ? 96 : 230;
    const { g, w, h } = fitCanvas(cv);
    const ink = css("--ink"), soft = css("--ink-soft"), faint = css("--ink-faint"), rule = css("--rule"), deep = css("--page-deep"), page = css("--page");
    g.fillStyle = page; g.fillRect(0, 0, w, h);
    rowH = Math.max(13, Math.min(30, (h - TOP - DENS - 10) / rows.length));
    const plotH = h - TOP - DENS;
    sy = Math.max(0, Math.min(sy, rows.length * rowH - plotH + 10));
    const y1 = x0 + (w - GUTTER) / ppy;
    // Ages: bands across the whole height, names on top.
    H.ages.forEach((a, i) => {
      const xa = Math.max(GUTTER, X(a[1])), xb = Math.min(w, X((a[2] ?? LAST) + 1));
      if (xb <= xa) return;
      g.fillStyle = i % 2 ? alpha(css("--rule").startsWith("#") ? css("--rule") : "#c8b48f", 0.16) : "transparent";
      g.fillRect(xa, 0, xb - xa, h - DENS);
      g.fillStyle = soft; g.font = `italic 12px ${css("--display")}`;
      g.save(); g.beginPath(); g.rect(xa, 0, xb - xa, 18); g.clip(); g.fillText(a[0], xa + 4, 13); g.restore();
    });
    // Year axis.
    const step = niceStep(ppy, 60);
    g.font = `11px ${css("--label")}`; g.fillStyle = soft; g.strokeStyle = rule; g.lineWidth = 1;
    for (let y = Math.ceil(x0 / step) * step; y <= y1; y += step) {
      const x = X(y); if (x < GUTTER) continue;
      g.beginPath(); g.moveTo(x + 0.5, 20); g.lineTo(x + 0.5, h - DENS); g.stroke();
      g.fillText(String(y), x + 3, 32);
    }
    // Rows.
    hit = []; warHit = [];
    const major = $("#tl-major").checked;
    g.save(); g.beginPath(); g.rect(0, TOP, w, plotH); g.clip();
    rows.forEach((r, k) => {
      const cy = TOP + k * rowH - sy + rowH / 2;
      if (cy < TOP - rowH || cy > TOP + plotH + rowH) return;
      const p = r.p, col = p.c || faint;
      const dim = focus && focus[0] === "F" && r.id !== +focus.slice(1);
      g.globalAlpha = dim ? 0.45 : 1;
      g.fillStyle = col; g.fillRect(8, cy - 5, 10, 10);
      g.fillStyle = ink; g.font = `13px ${css("--body")}`;
      g.fillText(clip(p.n, 25), 24, cy + 4);
      const xa = Math.max(GUTTER, X(p.f)), xb = Math.min(w, X((p.d ?? LAST) + 1));
      if (xb > xa) { g.fillStyle = alpha(col, 0.22); g.fillRect(xa, cy - rowH * 0.3, xb - xa, rowH * 0.6); g.strokeStyle = alpha(col, 0.8); g.strokeRect(xa + 0.5, cy - rowH * 0.3 + 0.5, xb - xa - 1, rowH * 0.6 - 1); }
      if (p.d != null && X(p.d) > GUTTER) { g.fillStyle = ink; g.fillText("†", X(p.d + 1) + 3, cy + 4); }
      r.cy = cy;
      g.globalAlpha = 1;
    });
    // Wars: a red band on each side's row.
    const warCol = catColor("war");
    for (const war of H.wars) {
      const xa = Math.max(GUTTER, X(war.s)), xb = Math.min(w, X((war.e ?? LAST) + 1));
      if (xb <= xa) continue;
      for (const f of [...war.a, ...war.df]) {
        const r = rows.find(r => r.id === f); if (!r || r.cy == null) continue;
        const yb = r.cy + rowH * 0.3 - 4;
        g.fillStyle = alpha(warCol, focus && focus !== "W" + war.i && focus[0] === "W" ? 0.15 : 0.55);
        g.fillRect(xa, yb, Math.max(2, xb - xa), 3.5);
        warHit.push({ x0: xa, x1: xb, y0: yb - 2, y1: yb + 6, war });
      }
    }
    // Events.
    const rowOf = new Map(rows.map(r => [r.id, r]));
    for (const e of E) {
      if (major && !e.major) continue;
      const c = cat(e.kind); if (hiddenCats.has(c)) continue;
      const x = X(e.year + e.season / 4 + 0.125); if (x < GUTTER || x > w) continue;
      const ps = evRows.get(e.id);
      const targets = ps.length ? ps.map(p => rowOf.get(p)).filter(Boolean) : [rowOf.get(null)];
      for (const r of targets) {
        if (r.cy == null) continue;
        const len = e.major ? rowH * 0.42 : rowH * 0.26;
        g.strokeStyle = catColor(c); g.globalAlpha = focusSet ? (focusSet.has(e.id) ? 1 : 0.12) : (e.major ? 0.95 : 0.6);
        g.lineWidth = e.major ? 2 : 1;
        g.beginPath(); g.moveTo(x, r.cy - len); g.lineTo(x, r.cy + len); g.stroke();
        hit.push({ x, y: r.cy, e });
      }
    }
    g.globalAlpha = 1; g.lineWidth = 1;
    g.restore();
    // Gutter rule and the chart below.
    g.strokeStyle = rule; g.beginPath(); g.moveTo(GUTTER - 6.5, TOP); g.lineTo(GUTTER - 6.5, h - DENS); g.stroke();
    const dy = h - DENS + 8, dh = DENS - 26;
    g.fillStyle = deep; g.fillRect(GUTTER, dy, w - GUTTER, dh);
    const chart = $("#tl-chart").value;
    if (chart !== "events") { drawRealms(g, chart, dy, dh, w); return; }
    const bin = ppy > 10 ? 1 : ppy > 3 ? 5 : ppy > 1 ? 10 : 25;
    const bins = new Map();
    for (const e of E) {
      if (e.year < x0 - bin || e.year > y1 + bin) continue;
      const c = cat(e.kind); if (hiddenCats.has(c)) continue;
      if (major && !e.major) continue;
      if (focusSet && !focusSet.has(e.id)) continue;
      const b = Math.floor(e.year / bin) * bin;
      if (!bins.has(b)) bins.set(b, {});
      bins.get(b)[c] = (bins.get(b)[c] || 0) + 1;
    }
    let maxv = 1; for (const v of bins.values()) maxv = Math.max(maxv, Object.values(v).reduce((a, b) => a + b, 0));
    const order = [...CATS.map(c => c[0]), "other"];
    for (const [b, v] of bins) {
      let acc = 0; const xa = X(b), bw = Math.max(1, bin * ppy - 1);
      for (const c of order) { const n = v[c] || 0; if (!n) continue; const hh = n / maxv * dh; g.fillStyle = catColor(c); g.fillRect(xa, dy + dh - acc - hh, bw, hh); acc += hh; }
    }
    g.fillStyle = soft; g.font = `11px ${css("--label")}`;
    g.fillText(`events per ${bin === 1 ? "year" : bin + " years"} (most ${maxv})`, 8, dy + 12);
  }
  // Lands held (or towns standing) by each people, year by year, stacked in the rows' order.
  function drawRealms(g, chart, dy, dh, w) {
    const series = chart === "lands" ? landsByYear() : townsByYear();
    const y0 = Math.max(FIRST, Math.floor(x0)), y1 = Math.min(LAST, Math.ceil(x0 + (w - GUTTER) / ppy));
    // Each people's share of the whole in that year (the rise and fall reads better than totals).
    const total = new Float64Array(y1 - y0 + 2);
    for (let y = y0; y <= y1; y++) for (const r of rows) if (r.id != null) total[y - y0] += series.get(r.id)?.[y - FIRST] || 0;
    let peak = 0; for (const t of total) peak = Math.max(peak, t);
    const share = (v, y) => total[y - y0] ? v / total[y - y0] : 0;
    const maxv = 1;
    const acc = new Float64Array(y1 - y0 + 2);
    for (const r of rows) {
      if (r.id == null) continue;
      const v = series.get(r.id); if (!v) continue;
      g.beginPath();
      for (let y = y0; y <= y1; y++) { const k = y - y0; const x = X(y), top = dy + dh - (acc[k] + share(v[y - FIRST] || 0, y)) * dh; if (y === y0) g.moveTo(x, top); else g.lineTo(x, top); }
      for (let y = y1; y >= y0; y--) { const k = y - y0; g.lineTo(X(y), dy + dh - acc[k] * dh); }
      g.closePath(); g.fillStyle = alpha(r.p.c, focus && focus !== "F" + r.id ? 0.25 : 0.85); g.fill();
      g.strokeStyle = alpha("#382a20", 0.25); g.lineWidth = 0.5; g.stroke();
      for (let y = y0; y <= y1; y++) acc[y - y0] += share(v[y - FIRST] || 0, y);
    }
    g.fillStyle = css("--ink-soft"); g.font = `11px ${css("--label")}`;
    g.fillText(chart === "lands" ? `share of the held land` : `share of the towns`, 8, dy + 12);
    g.fillText(`(at most ${peak} ${chart === "lands" ? "tiles" : "towns"})`, 8, dy + 26);
  }
  cv.addEventListener("wheel", ev => {
    ev.preventDefault();
    if (ev.shiftKey) { sy += ev.deltaY; draw(); return; }
    const r = cv.getBoundingClientRect(), mx = ev.clientX - r.left;
    if (mx < GUTTER) { sy += ev.deltaY; draw(); return; }
    const yAt = x0 + (mx - GUTTER) / ppy;
    ppy = Math.max(0.05, Math.min(400, ppy * Math.exp(-ev.deltaY * 0.0015)));
    x0 = yAt - (mx - GUTTER) / ppy;
    draw();
  }, { passive: false });
  let drag = null;
  cv.addEventListener("pointerdown", ev => { drag = { x: ev.clientX, y: ev.clientY, x0, sy, moved: false }; });
  const move = ev => {
    const r = cv.getBoundingClientRect(), mx = ev.clientX - r.left, my = ev.clientY - r.top;
    if (drag) {
      if (Math.abs(ev.clientX - drag.x) + Math.abs(ev.clientY - drag.y) > 3) drag.moved = true;
      x0 = drag.x0 - (ev.clientX - drag.x) / ppy; sy = drag.sy - (ev.clientY - drag.y); draw(); closeTip(); return;
    }
    if (ev.target !== cv) return;
    const best = nearest(mx, my);
    if (best?.e) showTip(`<b>${esc(best.e.t)}</b><br><span class="muted">${best.e.year}, ${esc(kindWords(best.e.kind))}</span>${best.e.d && best.e.d !== best.e.t ? `<br>${esc(clip(best.e.d, 220))}` : ""}`, ev.clientX, ev.clientY);
    else if (best?.war) showTip(`<b>${esc(best.war.n)}</b><br><span class="muted">${best.war.s}–${best.war.e ?? ""}, ${best.war.b.length} battles</span>`, ev.clientX, ev.clientY);
    else closeTip();
    cv.style.cursor = best ? "pointer" : "default";
  };
  function nearest(mx, my) {
    let best = null, bd = 7;
    for (const t of hit) { const d = Math.abs(t.x - mx) + (Math.abs(t.y - my) > rowH * 0.5 ? 99 : 0); if (d < bd) { bd = d; best = t; } }
    if (best) return best;
    for (const t of warHit) if (mx >= t.x0 && mx <= t.x1 && my >= t.y0 && my <= t.y1) return t;
    if (mx < GUTTER) { const r = rows.find(r => r.cy != null && Math.abs(r.cy - my) < rowH / 2); if (r && r.id != null) return { people: r.id }; }
    return null;
  }
  const up = ev => {
    if (!drag) return;
    const moved = drag.moved; drag = null;
    if (moved || ev.target !== cv) return;
    const r = cv.getBoundingClientRect(), b = nearest(ev.clientX - r.left, ev.clientY - r.top);
    if (b?.e) openDrawer(eventPage(b.e.id));
    else if (b?.war) openDrawer(entityPage("W" + b.war.i));
    else if (b?.people != null) openDrawer(entityPage("F" + b.people));
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  const onResize = () => draw();
  window.addEventListener("resize", onResize);
  cleanup = () => { window.removeEventListener("pointermove", move); window.removeEventListener("pointerup", up); window.removeEventListener("resize", onResize); };
  $("#tl-sort").onchange = () => { buildRows(); draw(); };
  $("#tl-major").onchange = draw;
  $("#tl-chart").onchange = draw;
  if (q.chart) $("#tl-chart").value = q.chart;
  $("#tl-fit").onclick = () => { fit(); draw(); };
  buildRows();
  if (q.from) fit(+q.from, +(q.to || LAST)); else if (q.year) fit(+q.year - 25, +q.year + 25); else fit();
  draw();
}

// ---------------------------------------------------------------------------------------------
// Causes: the causal graph round an event

function causesView(id, q) {
  if (id == null || !EV.has(id)) {
    // No event chosen: offer the ones with the longest chains and the busiest hubs.
    const hubs = E.map(e => [e, (EFFECTS.get(e.id) || []).length]).filter(x => x[1] > 0).sort((a, b) => b[1] - a[1]).slice(0, 15);
    const deep = depths(); const longest = [...deep.entries()].sort((a, b) => b[1] - a[1]).slice(0, 15);
    view.innerHTML = `<div class="reader"><div class="page"><p class="eyebrow">causes</p><h1>Why things happened</h1>
      <p class="lede">Pick an event to see what caused it and what came of it. Every event page has a <i>why?</i> link; or start from one of these.</p>
      <h2>The longest chains</h2><div class="chron">${longest.map(([eid, d]) => { const e = EV.get(eid); return `<p><span class="muted">${d} links ·</span> <a href="#/causes/${eid}">${esc(e.t)}</a> <span class="muted">${e.year}</span></p>`; }).join("")}</div>
      <h2>Events that led to the most</h2><div class="chron">${hubs.map(([e, n]) => `<p><span class="muted">${n} effects ·</span> <a href="#/causes/${e.id}">${esc(e.t)}</a> <span class="muted">${e.year}</span></p>`).join("")}</div></div></div>`;
    return;
  }
  const back = +(q.back ?? 4), fwd = +(q.fwd ?? 3), per = 28;
  const root = EV.get(id);
  // Levels: negative are causes, positive effects; each event at the first level it is reached.
  const level = new Map([[id, 0]]);
  const cols = new Map([[0, [root]]]);
  const more = new Map();
  for (const [dir, depth] of [[-1, back], [1, fwd]]) {
    let frontier = [root];
    for (let k = 1; k <= depth && frontier.length; k++) {
      const next = [];
      for (const e of frontier) for (const n of (dir < 0 ? causesOf(e) : effectsOf(e))) if (!level.has(n.id)) { level.set(n.id, dir * k); next.push(n); }
      next.sort((a, b) => a.year - b.year || a.id - b.id);
      if (next.length > per) { more.set(dir * k, next.length - per); next.length = per; for (const n of [...level.keys()]) if (level.get(n) === dir * k && !next.some(x => x.id === n)) level.delete(n); }
      cols.set(dir * k, next);
      frontier = next;
    }
  }
  view.innerHTML = `<div class="tool"><div class="toolbar"><span class="title">Causes of</span> <a href="#/event/${id}">${esc(clip(root.t, 60))}</a> <span class="muted">${root.year}</span>
    <label>back <select id="cz-back">${[1, 2, 3, 4, 6, 8, 12].map(n => `<option ${n === back ? "selected" : ""}>${n}</option>`).join("")}</select></label>
    <label>forward <select id="cz-fwd">${[0, 1, 2, 3, 4, 6].map(n => `<option ${n === fwd ? "selected" : ""}>${n}</option>`).join("")}</select></label>
    <span class="muted">${level.size} events</span></div><div class="stage"><svg id="cz"></svg><div class="hint">click an event to centre on it · shift+click to read it · wheel zooms · drag pans</div></div></div>`;
  $("#cz-back").onchange = ev => { location.hash = `#/causes/${id}?back=${ev.target.value}&fwd=${fwd}`; };
  $("#cz-fwd").onchange = ev => { location.hash = `#/causes/${id}?back=${back}&fwd=${ev.target.value}`; };
  const svg = $("#cz");
  const NW = 230, NH = 44, GX = 70, GY = 14;
  const pos = new Map();
  const keys = [...cols.keys()].sort((a, b) => a - b);
  const tallest = Math.max(...keys.map(k => cols.get(k).length + (more.has(k) ? 1 : 0)));
  for (const k of keys) {
    const col = cols.get(k);
    const x = (k - keys[0]) * (NW + GX);
    const off = (tallest - col.length) * (NH + GY) / 2;
    col.forEach((e, i) => pos.set(e.id, { x, y: off + i * (NH + GY), e, k }));
  }
  const ink = css("--ink"), soft = css("--ink-soft"), rule = css("--rule"), wash = css("--wash"), rub = css("--rubric");
  const gEdges = svgEl("g"), gNodes = svgEl("g");
  svg.append(svgEl("defs", {}), gEdges, gNodes);
  svg.querySelector("defs").innerHTML = `<marker id="arr" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L10,5 L0,10 z" fill="${soft}"/></marker>`;
  // Edges: every cause link between two shown events (left to right where the levels allow).
  for (const p of pos.values()) {
    for (const c of causesOf(p.e)) {
      const a = pos.get(c.id); if (!a) continue;
      const x1 = a.x + NW, y1 = a.y + NH / 2, x2 = p.x, y2 = p.y + NH / 2;
      const back = x2 <= x1;
      const d = back ? `M${a.x + NW / 2},${a.y + NH} C${a.x + NW / 2},${a.y + NH + 40} ${p.x + NW / 2},${p.y + NH + 40} ${p.x + NW / 2},${p.y + NH}`
        : `M${x1},${y1} C${x1 + GX * 0.6},${y1} ${x2 - GX * 0.6},${y2} ${x2},${y2}`;
      const onRoot = p.e.id === id || c.id === id;
      gEdges.append(svgEl("path", { d, fill: "none", stroke: onRoot ? rub : soft, "stroke-width": onRoot ? 1.8 : 1, "stroke-opacity": 0.75, "marker-end": "url(#arr)" }));
    }
  }
  for (const p of pos.values()) {
    const e = p.e, c = cat(e.kind), isRoot = e.id === id;
    const g = svgEl("g", { transform: `translate(${p.x},${p.y})`, style: "cursor:pointer" });
    g.append(svgEl("rect", { width: NW, height: NH, rx: 4, fill: wash, stroke: isRoot ? rub : rule, "stroke-width": isRoot ? 2.5 : 1 }));
    g.append(svgEl("rect", { width: 5, height: NH, rx: 2, fill: catColor(c) }));
    g.append(svgEl("text", { x: 12, y: 15, class: "small" }, `${e.year} · ${kindWords(e.kind)}${causesOf(e).length && p.k === keys[0] ? " · ←" : ""}${effectsOf(e).length && p.k === keys[keys.length - 1] ? " · →" : ""}`));
    g.append(svgEl("text", { x: 12, y: 33, "font-size": 13 }, clip(e.t, 31)));
    g.append(svgEl("title", {}, `${e.year}: ${e.t}${e.d && e.d !== e.t ? "\n" + e.d : ""}`));
    g.addEventListener("click", ev => {
      if (svg.dataset.dragged === "1") return;
      if (ev.shiftKey || isRoot) openDrawer(eventPage(e.id)); else location.hash = `#/causes/${e.id}?back=${back}&fwd=${fwd}`;
    });
    gNodes.append(g);
  }
  for (const [k, n] of more) {
    const x = (k - keys[0]) * (NW + GX), y = (tallest) * (NH + GY) - NH / 2;
    gNodes.append(svgEl("text", { x: x + 12, y, class: "small" }, `… and ${n} more`));
  }
  // Column heads.
  for (const k of keys) {
    const x = (k - keys[0]) * (NW + GX);
    gNodes.append(svgEl("text", { x, y: -14, class: "small" }, k === 0 ? "the event" : k < 0 ? `${-k} link${k < -1 ? "s" : ""} back` : `${k} link${k > 1 ? "s" : ""} on`));
  }
  const bw = keys.length * (NW + GX), bh = tallest * (NH + GY) + 40;
  const rp = pos.get(id);
  panZoom(svg, fitBox(svg, { w: bw + 60, h: bh + 60 }, bw / 2 - GX / 2, bh / 2 - 20));
  openDrawer(eventPage(id));
}

// The longest chain back from each event (memoised; safe against cycles).
let DEPTHS = null;
function depths() {
  if (DEPTHS) return DEPTHS;
  DEPTHS = new Map();
  const visiting = new Set();
  const depth = e => {
    if (DEPTHS.has(e.id)) return DEPTHS.get(e.id);
    if (visiting.has(e.id)) return 0;
    visiting.add(e.id);
    let d = 0; for (const c of causesOf(e)) d = Math.max(d, 1 + depth(c));
    visiting.delete(e.id);
    DEPTHS.set(e.id, d);
    return d;
  };
  for (const e of E) depth(e);
  return DEPTHS;
}

// ---------------------------------------------------------------------------------------------
// Family: a person's ancestors above and descendants below

const RULER = /King|Queen|Emperor|Empress|Chief|Chieftain|Lord|Lady|Prince|Princess|Duke|Duchess|Khan|Ruler|Warlord|Consul|Elder|Monarch|Sovereign|High|Tyrant|Overlord|Jarl|Count|Countess|Baron|Archon|Doge|Regent/;
const fig = id => id == null ? null : ENT.get("P" + id);

function familyView(code, q) {
  if (code && code[0] === "H") { const h = ent(code); if (h) code = "P" + h.fo; }
  const root = code ? ent(code) : null;
  if (!root || code[0] !== "P") {
    const houses = [...H.houses].sort((a, b) => b.m.length - a.m.length).slice(0, 30);
    const fertile = [...H.figures].sort((a, b) => b.ch.length - a.ch.length).filter(f => f.ch.length).slice(0, 20);
    view.innerHTML = `<div class="reader"><div class="page"><p class="eyebrow">family</p><h1>Houses and bloodlines</h1>
      <p class="lede">Pick a house or a person (search above, or a person's page has a <i>Family tree</i> link).</p>
      <h2>The houses</h2><div class="cols">${houses.map(h => `<p><a href="#/family/P${h.fo}?up=0&down=8">${esc(h.n)}</a> <span class="muted">${h.m.length} of its blood, from ${h.f}</span></p>`).join("") || `<p class="muted">No houses.</p>`}</div>
      <h2>Those with most children</h2><div class="cols">${fertile.map(f => `<p><a href="#/family/P${f.i}">${esc(f.n)}</a> <span class="muted">${f.ch.length} children</span></p>`).join("")}</div></div></div>`;
    return;
  }
  const up = +(q.up ?? 3), down = +(q.down ?? 3);
  view.innerHTML = `<div class="tool"><div class="toolbar"><span class="title">Family of</span> ${link(code)} <span class="muted">${lifeYears(root.b, root.d)}</span>
    <label>generations up <select id="fm-up">${[0, 1, 2, 3, 4, 6].map(n => `<option ${n === up ? "selected" : ""}>${n}</option>`).join("")}</select></label>
    <label>down <select id="fm-down">${[0, 1, 2, 3, 4, 6, 8, 12].map(n => `<option ${n === down ? "selected" : ""}>${n}</option>`).join("")}</select></label>
    ${root.h != null ? `<span>house: ${link("H" + root.h)}</span>` : ""}<span class="muted" id="fm-count"></span></div>
    <div class="stage"><svg id="fm"></svg><div class="hint">click a person to put them at the root · shift+click to read · ♔ held a title of rule · faded: dead</div></div></div>`;
  $("#fm-up").onchange = ev => { location.hash = `#/family/${code}?up=${ev.target.value}&down=${down}`; };
  $("#fm-down").onchange = ev => { location.hash = `#/family/${code}?up=${up}&down=${ev.target.value}`; };
  const BW = 168, BH = 50, GX = 16, GY = 46, UNIT = BW + GX;
  let count = 0; const LIMIT = 600;
  const seen = new Set([root.i]);
  // Descendants as a tree of couples.
  function downTree(f, d) {
    count++;
    const sp = fig(f.sp);
    const node = { f, sp, kids: [], d };
    if (d < down) for (const c of f.ch) { const cf = fig(c); if (cf && !seen.has(cf.i) && count < LIMIT) { seen.add(cf.i); node.kids.push(downTree(cf, d + 1)); } }
    node.kids.sort((a, b) => a.f.b - b.f.b);
    node.units = sp ? 2 : 1;
    node.width = Math.max(node.units, node.kids.reduce((s, k) => s + k.width, 0));
    return node;
  }
  function upTree(f, d) {
    count++;
    const node = { f, parents: [], d };
    if (d < up) for (const p of f.pa) { const pf = fig(p); if (pf && !seen.has(pf.i) && count < LIMIT) { seen.add(pf.i); node.parents.push(upTree(pf, d + 1)); } }
    node.width = Math.max(1, node.parents.reduce((s, k) => s + k.width, 0));
    return node;
  }
  const dt = downTree(root, 0);
  const ut = upTree(root, 0);
  $("#fm-count").textContent = `${count} people${count >= LIMIT ? " (cut short)" : ""}`;
  const svg = $("#fm");
  const gE = svgEl("g"), gN = svgEl("g");
  svg.append(gE, gN);
  const soft = css("--ink-soft"), rule = css("--rule"), wash = css("--wash"), rub = css("--rubric"), deep = css("--page-deep");
  const boxes = [];
  function box(f, x, y, isRoot) {
    const dead = f.d != null;
    const g = svgEl("g", { transform: `translate(${x},${y})`, style: "cursor:pointer", opacity: dead ? 0.72 : 1 });
    g.append(svgEl("rect", { width: BW, height: BH, rx: 4, fill: dead ? deep : wash, stroke: isRoot ? rub : rule, "stroke-width": isRoot ? 2.4 : 1 }));
    g.append(svgEl("rect", { width: BW, height: 4, fill: f.p != null ? peopleColor(f.p) : rule }));
    const ruler = f.t.some(t => RULER.test(t));
    g.append(svgEl("text", { x: 8, y: 21, "font-size": 13 }, (ruler ? "♔ " : "") + clip(f.n, ruler ? 21 : 23)));
    g.append(svgEl("text", { x: 8, y: 38, class: "small" }, `${lifeYears(f.b, f.d)}${f.t.length ? " · " + clip(f.t[0], 18) : ""}`));
    g.append(svgEl("title", {}, `${f.n}\n${lifeYears(f.b, f.d)}${f.t.length ? "\n" + f.t.join(", ") : ""}${f.p != null ? "\n" + (people(f.p)?.n ?? "") : ""}`));
    g.addEventListener("click", ev => { if (svg.dataset.dragged === "1") return; if (ev.shiftKey || isRoot) openDrawer(entityPage("P" + f.i)); else location.hash = `#/family/P${f.i}?up=${up}&down=${down}`; });
    gN.append(g);
    boxes.push([x, y]);
  }
  const line = d => gE.append(svgEl("path", { d, fill: "none", stroke: soft, "stroke-width": 1.2 }));
  // Place descendants: a couple centred over its children.
  function placeDown(n, left) {
    const y = n.d * (BH + GY);
    let cx;
    if (n.kids.length) {
      let l = left; const centers = [];
      for (const k of n.kids) { centers.push(placeDown(k, l)); l += k.width * UNIT; }
      cx = (centers[0] + centers[centers.length - 1]) / 2;
    } else cx = left + n.width * UNIT / 2;
    const pairW = n.units * UNIT - GX;
    const x = cx - pairW / 2;
    box(n.f, x, y, n.d === 0);
    if (n.sp) { box(n.sp, x + UNIT, y, false); line(`M${x + BW},${y + BH / 2 - 3} h${GX} M${x + BW},${y + BH / 2 + 3} h${GX}`); }
    const from = n.sp ? x + BW + GX / 2 : cx;
    if (n.kids.length) {
      const midY = y + BH + GY / 2;
      line(`M${from},${n.sp ? y + BH / 2 + 3 : y + BH} V${midY}`);
      for (const k of n.kids) line(`M${k.cx},${k.d * (BH + GY)} V${midY} H${from}`);
    }
    n.cx = n.sp ? x + BW / 2 : cx;
    return n.cx;
  }
  placeDown(dt, 0);
  const rootCx = dt.cx;
  // Place ancestors above the root, centred on it.
  function placeUp(n, left) {
    const y = -n.d * (BH + GY);
    let cx;
    if (n.parents.length) {
      let l = left; const centers = [];
      for (const p of n.parents) { centers.push(placeUp(p, l)); l += p.width * UNIT; }
      cx = (centers[0] + centers[centers.length - 1]) / 2;
      const midY = y - GY / 2;
      line(`M${cx},${y} V${midY}`);
      for (const p of n.parents) line(`M${p.cx},${-p.d * (BH + GY) + BH} V${midY} H${cx}`);
    } else cx = left + n.width * UNIT / 2;
    if (n.d > 0) box(n.f, cx - BW / 2, y, false);
    n.cx = cx;
    return cx;
  }
  if (ut.parents.length) {
    const shift = rootCx - ut.width * UNIT / 2;
    placeUp(ut, shift);
    // The root's own link up joins its box (placed by placeDown) to its parents' line.
    const dx = rootCx - ut.cx;
    if (Math.abs(dx) > 0.5) line(`M${rootCx},0 V${-GY / 2} H${ut.cx}`);
  }
  const xs = boxes.map(b => b[0]), ys = boxes.map(b => b[1]);
  const bx = Math.min(...xs) - 40, by = Math.min(...ys) - 40, bw = Math.max(...xs) + BW + 40 - bx, bh = Math.max(...ys) + BH + 40 - by;
  // Centred on the root when the tree is wider than the screen, else on the whole tree.
  const vb = fitBox(svg, { w: bw, h: bh }, bx + bw / 2, by + bh / 2, 1.6);
  if (bw / vb.w > 0.98) { const c = fitBox(svg, { w: bw, h: bh }, rootCx, by + bh / 2, 1.6); vb.x = c.x; }
  panZoom(svg, vb);
  openDrawer(entityPage(code));
}

// ---------------------------------------------------------------------------------------------
// Connections: the web of peoples, or of one person

const LINKS = [
  ["war", "war and battle", /WarDeclared|Battle|Siege|HolyWar|Conquest|Shadow/, "--c-war"],
  ["strife", "raids and quarrels", /Raid|Dispute|Quarrel|Insult|Assassin|Border/, "--c-beast"],
  ["peace", "treaties and alliances", /Treaty|Alliance|Peace|WarEnded|JoinWar|Tribute/, "--c-people"],
  ["trade", "trade", /Trade|Caravan/, "--c-trade"],
  ["marriage", "marriages", /Marriage/, "--c-faith"],
  ["faith", "faith", /Convert|Conversion|Relig|Miracle|Schism/, "--c-politics"],
  ["other", "other dealings", /./, "--c-other"],
];
const linkClass = kind => (LINKS.find(l => l[2].test(kind)) || LINKS[LINKS.length - 1])[0];

function webView(code, q) {
  if (code && code[0] === "P") return egoWeb(code);
  const from = +(q.from ?? FIRST), to = +(q.to ?? LAST);
  const focus = q.focus != null ? +q.focus : null;
  const mode = q.mode || "deeds";
  const shown = new Set((q.show || "war,strife,peace,trade,marriage").split(","));
  const layout = q.layout || (H.peoples.length > 14 ? "geo" : "circle");
  let minN = q.min != null ? +q.min : null;
  view.innerHTML = `<div class="tool"><div class="toolbar"><span class="title">Peoples and their dealings</span>
    <label>from <input id="wb-from" type="number" value="${from}" min="${FIRST}" max="${LAST}" style="width:5rem"></label>
    <label>to <input id="wb-to" type="number" value="${to}" min="${FIRST}" max="${LAST}" style="width:5rem"></label>
    <label>show <select id="wb-mode"><option value="deeds" ${mode === "deeds" ? "selected" : ""}>what passed between them</option><option value="regard" ${mode === "regard" ? "selected" : ""}>how they regard each other today</option></select></label>
    <label>layout <select id="wb-layout"><option value="geo" ${layout === "geo" ? "selected" : ""}>where they lived</option><option value="circle" ${layout === "circle" ? "selected" : ""}>a ring, west to east</option></select></label>
    <label>at least <input id="wb-min" type="number" min="1" value="${minN}" style="width:4rem"> dealings</label>
    <span class="legend" id="wb-kinds">${mode === "deeds" ? LINKS.map(l => `<label><input type="checkbox" data-k="${l[0]}" ${shown.has(l[0]) ? "checked" : ""}><span class="dot" style="background:${css(l[3])}"></span>${l[1]}</label>`).join("") : `<span>green: friendly · red: hostile · width: strength</span>`}</span>
    </div><div class="stage"><svg id="wb"></svg><div class="hint">click a line for what passed between two peoples · click a people to single it out · node size: towns held at the end</div></div></div>`;
  const go = (patch) => {
    const p = { from, to, mode, layout, show: [...shown].join(","), ...(minN != null && q.min != null ? { min: minN } : {}), ...(focus != null ? { focus } : {}), ...patch };
    for (const k of Object.keys(p)) if (p[k] == null) delete p[k];
    location.hash = "#/web?" + new URLSearchParams(p).toString();
  };
  $("#wb-from").onchange = ev => go({ from: ev.target.value });
  $("#wb-to").onchange = ev => go({ to: ev.target.value });
  $("#wb-mode").onchange = ev => go({ mode: ev.target.value });
  $("#wb-layout").onchange = ev => go({ layout: ev.target.value });
  $("#wb-min").onchange = ev => go({ min: ev.target.value });
  $("#wb-kinds").addEventListener("change", ev => { const k = ev.target.dataset.k; if (!k) return; if (ev.target.checked) shown.add(k); else shown.delete(k); go({ show: [...shown].join(",") }); });
  const alive = H.peoples.filter(p => p.f <= to && (p.d == null || p.d >= from));
  const townsAt = (pid, y) => H.towns.filter(t => t.f <= y && (t.d == null || t.d > y) && ownerAt(t, y) === pid).length;
  // Pairs.
  const pairs = new Map();
  const key = (a, b) => a < b ? a + "-" + b : b + "-" + a;
  if (mode === "deeds") {
    for (const e of E) {
      if (e.year < from || e.year > to) continue;
      const ps = [...new Set(e.peoples)]; if (ps.length < 2) continue;
      const cl = linkClass(e.kind); if (!shown.has(cl)) continue;
      for (let i = 0; i < ps.length; i++) for (let j = i + 1; j < ps.length; j++) {
        const k = key(ps[i], ps[j]);
        if (!pairs.has(k)) pairs.set(k, { a: Math.min(ps[i], ps[j]), b: Math.max(ps[i], ps[j]), n: {}, evs: [] });
        const p = pairs.get(k); p.n[cl] = (p.n[cl] || 0) + 1; p.evs.push(e);
      }
    }
  } else {
    for (const p of H.peoples) for (const r of p.rel || []) {
      if (p.d != null || people(r[0])?.d != null) continue;
      const k = key(p.i, r[0]);
      if (!pairs.has(k)) pairs.set(k, { a: Math.min(p.i, r[0]), b: Math.max(p.i, r[0]), op: [], st: [] });
      pairs.get(k).op.push(r[1]); pairs.get(k).st.push(r[2]);
    }
  }
  // Where each people lived: the middle of its towns at the end of the span (else ever).
  const home = new Map();
  for (const p of H.peoples) {
    const y = Math.min(to, p.d ?? to);
    let ts = H.towns.filter(t => t.f <= y && (t.d == null || t.d > y) && ownerAt(t, y) === p.i);
    if (!ts.length) ts = H.towns.filter(t => t.own.some(r => r[0] === p.i) || t.p === p.i);
    if (!ts.length) continue;
    // Round the date line: average the longitudes as angles.
    const a = ts.reduce((s, t) => [s[0] + Math.cos(t.x / H.meta.w * 2 * Math.PI), s[1] + Math.sin(t.x / H.meta.w * 2 * Math.PI)], [0, 0]);
    const lon = ((Math.atan2(a[1], a[0]) / (2 * Math.PI) + 1) % 1) * H.meta.w;
    home.set(p.i, [lon, ts.reduce((s, t) => s + t.y, 0) / ts.length]);
  }
  const nodes = (mode === "regard" ? H.peoples.filter(p => p.d == null) : alive).filter(p => layout !== "geo" || home.has(p.i))
    .sort((a, b) => (home.get(a.i)?.[0] ?? 0) - (home.get(b.i)?.[0] ?? 0) || a.i - b.i);
  const svg = $("#wb");
  const K = 1400 / H.meta.w;   // graph units a tile in the geography layout
  const R = layout === "geo" ? H.meta.w * K / 2 : Math.max(220, nodes.length * 26);
  const pos = new Map(nodes.map((p, i) => {
    if (layout === "geo") { const h = home.get(p.i); const x = (h[0] - H.meta.w / 2) * K, y = (h[1] - H.meta.h / 2) * K; return [p.i, { x, y, t: Math.atan2(y, x), p }]; }
    const t = i / nodes.length * Math.PI * 2 - Math.PI / 2; return [p.i, { x: Math.cos(t) * R, y: Math.sin(t) * R, t, p }];
  }));
  if (layout === "geo" && H.meta.map) svg.append(svgEl("image", { href: H.meta.map.file, x: -H.meta.w * K / 2, y: -H.meta.h * K / 2, width: H.meta.w * K, height: H.meta.h * K, opacity: 0.55, preserveAspectRatio: "none" }));
  // The threshold: by default the strongest few hundred ties.
  if (minN == null) {
    const counts = [];
    for (const pr of pairs.values()) if (pr.n) for (const n of Object.values(pr.n)) counts.push(n);
    counts.sort((a, b) => b - a);
    minN = counts.length > 260 ? Math.max(1, counts[259]) : 1;
    $("#wb-min").value = minN;
  }
  const gE = svgEl("g"), gN = svgEl("g");
  svg.append(gE, gN);
  const soft = css("--ink-soft"), rub = css("--rubric");
  const linkOrder = LINKS.map(l => l[0]);
  for (const pr of pairs.values()) {
    const A = pos.get(pr.a), B = pos.get(pr.b); if (!A || !B) continue;
    const dim = focus != null && pr.a !== focus && pr.b !== focus;
    const draws = mode === "deeds"
      ? Object.entries(pr.n).filter(([, n]) => n >= minN || (focus != null && (pr.a === focus || pr.b === focus))).map(([cl, n]) => [cl, n, css(LINKS.find(l => l[0] === cl)[3]), 0.7 + Math.log2(n) * 0.8, `${n} ${LINKS.find(l => l[0] === cl)[1]}`])
      : (() => { const op = pr.op.reduce((s, x) => s + x, 0) / pr.op.length; return [["op", op, op >= 0 ? css("--c-people") : css("--c-war"), 0.6 + Math.abs(op) / 18, `opinion ${op > 0 ? "+" : ""}${Math.round(op)}; ${[...new Set(pr.st)].map(kindWords).join(", ")}`]]; })();
    draws.forEach(([cl, n, col, wdt, words], k) => {
      const off = (linkOrder.indexOf(cl) - 2.5) * 0.06;
      let mx = (A.x + B.x) / 2 * (0.55 + off), my = (A.y + B.y) / 2 * (0.55 + off);
      if (layout === "geo") { const dx = B.x - A.x, dy = B.y - A.y, k = 0.12 + off; mx = (A.x + B.x) / 2 - dy * k; my = (A.y + B.y) / 2 + dx * k; }
      const d = `M${A.x},${A.y} Q${mx},${my} ${B.x},${B.y}`;
      gE.append(svgEl("path", { d, fill: "none", stroke: col, "stroke-width": wdt, "stroke-opacity": dim ? 0.06 : 0.55 }));
      const hitp = svgEl("path", { d, fill: "none", stroke: "transparent", "stroke-width": Math.max(8, wdt + 6), style: "cursor:pointer" });
      const tipText = `<b>${esc(A.p.n)} · ${esc(B.p.n)}</b><br>${esc(words)}`;
      hitp.addEventListener("mousemove", ev => showTip(tipText, ev.clientX, ev.clientY));
      hitp.addEventListener("mouseleave", closeTip);
      hitp.addEventListener("click", () => {
        if (svg.dataset.dragged === "1") return;
        const evs = (pr.evs || []).filter(e => mode !== "deeds" || linkClass(e.kind) === cl);
        openDrawer(`<p class="eyebrow">between ${from} and ${to}</p><h1>${link("F" + A.p.i)} and ${link("F" + B.p.i)}</h1><p class="lede">${esc(words)}</p>${mode === "deeds" ? chronicle(evs) : ""}`);
      });
      gE.append(hitp);
    });
  }
  for (const n of pos.values()) {
    const towns = townsAt(n.p.i, Math.min(to, n.p.d ?? to));
    const r = 6 + Math.sqrt(towns) * 4;
    const dim = focus != null && n.p.i !== focus && !pairs.has(key(n.p.i, focus));
    const g = svgEl("g", { transform: `translate(${n.x},${n.y})`, style: "cursor:pointer", opacity: dim ? 0.3 : 1 });
    g.append(svgEl("circle", { r, fill: n.p.c, stroke: n.p.i === focus ? rub : soft, "stroke-width": n.p.i === focus ? 3 : 1 }));
    const fs = layout === "geo" ? 11 : Math.max(13, R / 32);
    const [lx, ly, anchor] = layout === "geo" ? [0, r + fs + 1, "middle"]
      : [Math.cos(n.t) * (R + r + 6) - n.x, Math.sin(n.t) * (R + r + 6) - n.y, Math.cos(n.t) > 0.2 ? "start" : Math.cos(n.t) < -0.2 ? "end" : "middle"];
    const label = svgEl("text", { x: lx, y: ly + 4, "text-anchor": anchor, "font-size": fs, "paint-order": "stroke", stroke: layout === "geo" ? "#f6efdf" : "none", "stroke-width": 3, fill: layout === "geo" ? "#382a20" : css("--ink") }, `${n.p.n.replace(/^The /, "")}${n.p.d != null ? " †" : ""}`);
    g.append(label);
    g.append(svgEl("title", {}, `${n.p.n}: ${n.p.f}–${n.p.d ?? "now"}, ${towns} towns`));
    g.addEventListener("click", () => { if (svg.dataset.dragged === "1") return; openDrawer(entityPage("F" + n.p.i)); go({ focus: focus === n.p.i ? null : n.p.i }); });
    gN.append(g);
  }
  const sw = layout === "geo" ? H.meta.w * K + 40 : (R + 200) * 2, sh = layout === "geo" ? H.meta.h * K + 40 : sw;
  panZoom(svg, focus != null ? fitBox(svg, { w: sw, h: sh }, 0, 0, 99) : (() => { const r = svg.getBoundingClientRect(), k = Math.max(sw / r.width, sh / r.height); return { x: -r.width * k / 2, y: -r.height * k / 2, w: r.width * k, h: r.height * k }; })());
  if (focus != null) openDrawer(entityPage("F" + focus));
}

// Who held a town in a year (its tile's records; else its people).
function ownerAt(t, y) {
  let o = null;
  for (const r of t.own) if (r[1] <= y && (r[2] < 0 || r[2] > y)) o = r[0];
  return o ?? (t.f <= y ? t.p : null);
}

// One person and everyone tied to them: kin, enemies, mentors, and those named beside them.
function egoWeb(code) {
  const f = ent(code); if (!f) { view.innerHTML = `<p>No one.</p>`; return; }
  const ties = new Map();   // figure id -> {kind, n}
  const add = (id, kind, n = 1) => { if (id == null || id === f.i || !fig(id)) return; const t = ties.get(id); if (!t || t.rank > RANK[kind]) ties.set(id, { kind, n: (t?.n || 0) + n, rank: RANK[kind] }); else t.n += n; };
  const RANK = { parent: 0, spouse: 0, child: 0, sibling: 1, enemy: 1, mentor: 1, pupil: 1, events: 2 };
  f.pa.forEach(p => add(p, "parent")); add(f.sp, "spouse"); f.ch.forEach(c => add(c, "child"));
  for (const p of f.pa) for (const s of (fig(p)?.ch || [])) add(s, "sibling");
  f.en.forEach(x => add(x, "enemy")); f.me.forEach(x => add(x, "mentor"));
  for (const o of H.figures) { if (o.me.includes(f.i)) add(o.i, "pupil"); if (o.en.includes(f.i)) add(o.i, "enemy"); }
  const shared = new Map();
  for (const e of BY.get(code) || []) for (const c of e.parts) if (c[0] === "P" && c !== code) shared.set(+c.slice(1), (shared.get(+c.slice(1)) || 0) + 1);
  [...shared.entries()].sort((a, b) => b[1] - a[1]).slice(0, 24).forEach(([id, n]) => add(id, "events", n));
  const COL = { parent: "--c-people", spouse: "--c-faith", child: "--c-people", sibling: "--c-nature", enemy: "--c-war", mentor: "--c-culture", pupil: "--c-culture", events: "--c-other" };
  view.innerHTML = `<div class="tool"><div class="toolbar"><span class="title">Connections of</span> ${link(code)} <span class="muted">${lifeYears(f.b, f.d)}</span>
    <span class="legend">${Object.entries(COL).filter(([k]) => k !== "child" && k !== "pupil").map(([k, c]) => `<span><span class="dot" style="background:${css(c)}"></span>${k === "events" ? "named together" : k}</span>`).join("")}</span>
    <a href="#/family/${code}">family tree</a></div><div class="stage"><svg id="eg"></svg><div class="hint">click someone to centre on them · shift+click to read · inner ring: kin, foes, teachers · outer: named in the same events</div></div></div>`;
  const svg = $("#eg");
  const gE = svgEl("g"), gN = svgEl("g"); svg.append(gE, gN);
  const inner = [...ties.entries()].filter(([, t]) => t.rank < 2), outer = [...ties.entries()].filter(([, t]) => t.rank === 2);
  const place = (list, R) => list.map(([id, t], i) => { const a = i / Math.max(1, list.length) * Math.PI * 2 - Math.PI / 2 + (R > 300 ? 0.13 : 0); return { id, t, x: Math.cos(a) * R, y: Math.sin(a) * R }; });
  const nodes = [...place(inner, 220), ...place(outer, 430)];
  const node = (o, x, y, big, kind, n) => {
    const g = svgEl("g", { transform: `translate(${x},${y})`, style: "cursor:pointer", opacity: o.d != null && !big ? 0.75 : 1 });
    g.append(svgEl("circle", { r: big ? 16 : 9, fill: o.p != null ? peopleColor(o.p) : css("--rule"), stroke: big ? css("--rubric") : css("--ink-soft"), "stroke-width": big ? 3 : 1 }));
    g.append(svgEl("text", { y: big ? 34 : 24, "text-anchor": "middle", "font-size": big ? 15 : 12 }, clip(o.n, 26)));
    g.append(svgEl("text", { y: big ? 50 : 37, "text-anchor": "middle", class: "small" }, `${lifeYears(o.b, o.d)}${kind ? " · " + (kind === "events" ? `${n} events together` : kind) : ""}`));
    g.addEventListener("click", ev => { if (svg.dataset.dragged === "1") return; if (ev.shiftKey || big) openDrawer(entityPage("P" + o.i)); else location.hash = `#/web/P${o.i}`; });
    gN.append(g);
  };
  for (const n of nodes) gE.append(svgEl("line", { x1: 0, y1: 0, x2: n.x, y2: n.y, stroke: css(COL[n.t.kind]), "stroke-width": n.t.kind === "events" ? 0.8 + Math.log2(n.t.n) : 2, "stroke-opacity": 0.7 }));
  for (const n of nodes) node(fig(n.id), n.x, n.y, false, n.t.kind, n.t.n);
  node(f, 0, 0, true);
  panZoom(svg, fitBox(svg, { w: 1100, h: 1100 }, 0, 0, 99));
  openDrawer(entityPage(code));
}

// ---------------------------------------------------------------------------------------------
// Map: territories, towns and events at any year

const TERR = new Map();   // tile index -> [[people, gained, lost], ...]
for (let i = 0; i < H.territory.length; i += 4) {
  const t = H.territory[i];
  if (!TERR.has(t)) TERR.set(t, []);
  TERR.get(t).push([H.territory[i + 1], H.territory[i + 2], H.territory[i + 3]]);
}
const ownerOfTile = (i, y) => { let o = null; for (const r of TERR.get(i) || []) if (r[1] <= y && (r[2] < 0 || r[2] > y)) o = r[0]; return o; };
let MAP_IMG = null;

function mapView(q) {
  const W = H.meta.w, Ht = H.meta.h;
  let year = +(q.year ?? LAST);
  const focus = q.focus != null ? +q.focus : null;
  view.innerHTML = `<div class="tool"><div class="toolbar"><span class="title">The map in</span>
    <input id="mp-year" type="range" min="${FIRST}" max="${LAST}" value="${year}" style="flex:1 1 14rem">
    <input id="mp-yn" type="number" min="${FIRST}" max="${LAST}" value="${year}" style="width:5rem">
    <button id="mp-play">▶ play</button>
    <label>events of the last <select id="mp-win">${[1, 5, 10, 25, 50].map(n => `<option ${n === 10 ? "selected" : ""}>${n}</option>`).join("")}</select> years</label>
    <label><input type="checkbox" id="mp-terr" checked> lands</label><label><input type="checkbox" id="mp-names" checked> names</label>
    ${focus != null ? `<span>focus: ${link("F" + focus)} <a href="#/map?year=${year}">clear</a></span>` : ""}
    </div><div class="stage"><canvas id="mp"></canvas><div class="hint">wheel zooms · drag pans · click a town or a mark to read it</div></div></div>`;
  $(".toolbar").appendChild(catLegend(draw));
  const cv = $("#mp");
  let scale = 1, ox = 0, oy = 0, hits = [];
  const terr = document.createElement("canvas"); terr.width = W; terr.height = Ht;
  let borders = [];
  function buildTerritory() {
    const g = terr.getContext("2d"), img = g.createImageData(W, Ht);
    const own = new Int32Array(W * Ht).fill(-1);
    for (const [i] of TERR) { const o = ownerOfTile(i, year); if (o != null) own[i] = o; }
    const rgb = new Map();
    for (let i = 0; i < W * Ht; i++) {
      const o = own[i]; if (o < 0) continue;
      if (!rgb.has(o)) { const n = parseInt((people(o)?.c || "#888888").slice(1), 16); rgb.set(o, [n >> 16, (n >> 8) & 255, n & 255]); }
      const c = rgb.get(o), a = focus == null || o === focus ? 110 : 30;
      img.data.set([c[0], c[1], c[2], a], i * 4);
    }
    g.putImageData(img, 0, 0);
    borders = [];
    for (let y = 0; y < Ht; y++) for (let x = 0; x < W; x++) {
      const o = own[y * W + x];
      const r = x + 1 < W ? own[y * W + x + 1] : -2, d = y + 1 < Ht ? own[(y + 1) * W + x] : -2;
      if (r !== -2 && r !== o && (o >= 0 || r >= 0)) borders.push([x + 1, y, x + 1, y + 1]);
      if (d !== -2 && d !== o && (o >= 0 || d >= 0)) borders.push([x, y + 1, x + 1, y + 1]);
    }
  }
  function fit() {
    const r = cv.getBoundingClientRect();
    scale = Math.min(r.width / W, r.height / Ht);
    ox = (r.width - W * scale) / 2; oy = (r.height - Ht * scale) / 2;
    if (q.x != null) { scale *= 4; ox = r.width / 2 - (+q.x + 0.5) * scale; oy = r.height / 2 - (+q.y + 0.5) * scale; }
  }
  function draw() {
    const { g, w, h } = fitCanvas(cv);
    g.fillStyle = css("--page"); g.fillRect(0, 0, w, h);
    g.save(); g.translate(ox, oy); g.scale(scale, scale);
    if (MAP_IMG && MAP_IMG.complete && MAP_IMG.naturalWidth) g.drawImage(MAP_IMG, 0, 0, W, Ht);
    else { g.fillStyle = css("--page-deep"); g.fillRect(0, 0, W, Ht); }
    if ($("#mp-terr").checked) {
      g.imageSmoothingEnabled = false; g.drawImage(terr, 0, 0); g.imageSmoothingEnabled = true;
      g.strokeStyle = alpha("#382a20", 0.55); g.lineWidth = 1.2 / scale; g.beginPath();
      for (const b of borders) { g.moveTo(b[0], b[1]); g.lineTo(b[2], b[3]); }
      g.stroke();
    }
    g.restore();
    const P = (x, y) => [ox + (x + 0.5) * scale, oy + (y + 0.5) * scale];
    hits = [];
    // Events of the last years.
    const win = +$("#mp-win").value;
    for (const e of E) {
      if (e.x < 0 || e.year > year || e.year <= year - win) continue;
      const c = cat(e.kind); if (hiddenCats.has(c)) continue;
      const [x, y] = P(e.x, e.y); if (x < -10 || y < -10 || x > w + 10 || y > h + 10) continue;
      const age = (year - e.year) / win;
      g.globalAlpha = 1 - age * 0.75; g.strokeStyle = catColor(c); g.fillStyle = catColor(c); g.lineWidth = e.major ? 2.2 : 1.4;
      const s = (e.major ? 6 : 4) + Math.min(6, scale / 6);
      if (c === "war") { g.beginPath(); g.moveTo(x - s, y - s); g.lineTo(x + s, y + s); g.moveTo(x + s, y - s); g.lineTo(x - s, y + s); g.stroke(); }
      else if (/Destroyed|Abandon/.test(e.kind)) { g.strokeStyle = "#382a20"; g.beginPath(); g.moveTo(x - s, y - s); g.lineTo(x + s, y + s); g.moveTo(x + s, y - s); g.lineTo(x - s, y + s); g.stroke(); }
      else if (/Founded/.test(e.kind)) { g.beginPath(); g.arc(x, y, s, 0, Math.PI * 2); g.stroke(); }
      else { g.beginPath(); g.arc(x, y, s * 0.45, 0, Math.PI * 2); g.fill(); }
      hits.push({ x, y, r: s + 2, e });
    }
    g.globalAlpha = 1;
    // Towns.
    const names = $("#mp-names").checked;
    g.font = `${Math.round(11 + Math.min(4, scale / 8))}px ${css("--body")}`;
    const labels = [];
    for (const t of H.towns) {
      if (t.f > year) continue;
      const [x, y] = P(t.x, t.y); if (x < -20 || y < -20 || x > w + 20 || y > h + 20) continue;
      if (t.d != null && t.d <= year) {
        g.strokeStyle = alpha("#6b5846", 0.8); g.lineWidth = 1; const s = 2.5 + Math.min(3, scale / 10);
        g.beginPath(); g.moveTo(x - s, y - s); g.lineTo(x + s, y + s); g.moveTo(x + s, y - s); g.lineTo(x - s, y + s); g.stroke();
        hits.push({ x, y, r: 6, town: t });
        continue;
      }
      const o = ownerAt(t, year), big = /City|Capital|Metropolis/.test(t.t);
      const r = (big ? 4.5 : 3) + Math.min(4, scale / 10);
      g.fillStyle = o != null ? peopleColor(o) : "#888"; g.strokeStyle = "#382a20"; g.lineWidth = 1.2;
      g.globalAlpha = focus == null || o === focus ? 1 : 0.35;
      g.beginPath(); if (big) g.rect(x - r, y - r, 2 * r, 2 * r); else g.arc(x, y, r, 0, Math.PI * 2); g.fill(); g.stroke();
      g.globalAlpha = 1;
      hits.push({ x, y, r: r + 3, town: t });
      if (names && (scale > 7 || big)) labels.push([t.n, x + r + 3, y + 4, big]);
    }
    // Labels, greatest first, never over another.
    const taken = [];
    labels.sort((a, b) => b[3] - a[3]);
    for (const [n, x, y, big] of labels) {
      const wdt = g.measureText(n).width, box = [x - 2, y - 12, x + wdt + 2, y + 3];
      if (taken.some(b => !(box[2] < b[0] || box[0] > b[2] || box[3] < b[1] || box[1] > b[3]))) continue;
      taken.push(box);
      g.lineWidth = 3; g.strokeStyle = alpha("#f6efdf", 0.85); g.strokeText(n, x, y);
      g.fillStyle = "#382a20"; g.font = `${big ? "600 " : ""}${Math.round(11 + Math.min(4, scale / 8))}px ${css("--body")}`; g.fillText(n, x, y);
    }
    // The year and its age.
    const ai = ageOf(year);
    g.fillStyle = alpha("#f6efdf", 0.85); g.fillRect(10, 10, 270, 48);
    g.fillStyle = "#382a20"; g.font = `26px ${css("--display")}`; g.fillText(String(year), 18, 40);
    g.font = `italic 12px ${css("--display")}`; g.fillText(ai >= 0 ? clip(H.ages[ai][0], 34) : "", 90, 38);
  }
  const setYear = y => { year = Math.max(FIRST, Math.min(LAST, Math.round(y))); $("#mp-year").value = year; $("#mp-yn").value = year; buildTerritory(); draw(); };
  $("#mp-year").oninput = ev => setYear(+ev.target.value);
  $("#mp-yn").onchange = ev => setYear(+ev.target.value);
  ["mp-win", "mp-terr", "mp-names"].forEach(id => $("#" + id).onchange = draw);
  let timer = null;
  $("#mp-play").onclick = () => {
    if (timer) { clearInterval(timer); timer = null; $("#mp-play").textContent = "▶ play"; return; }
    if (year >= LAST) setYear(FIRST);
    $("#mp-play").textContent = "❚❚ pause";
    timer = setInterval(() => { if (year >= LAST) { clearInterval(timer); timer = null; $("#mp-play").textContent = "▶ play"; return; } setYear(year + 1); }, 140);
  };
  cv.addEventListener("wheel", ev => {
    ev.preventDefault();
    const r = cv.getBoundingClientRect(), mx = ev.clientX - r.left, my = ev.clientY - r.top;
    const k = Math.exp(-ev.deltaY * 0.0015), ns = Math.max(0.5, Math.min(200, scale * k));
    ox = mx - (mx - ox) * ns / scale; oy = my - (my - oy) * ns / scale; scale = ns; draw();
  }, { passive: false });
  let drag = null;
  cv.addEventListener("pointerdown", ev => { drag = { x: ev.clientX, y: ev.clientY, ox, oy, moved: false }; });
  const move = ev => {
    if (drag) { if (Math.abs(ev.clientX - drag.x) + Math.abs(ev.clientY - drag.y) > 3) drag.moved = true; ox = drag.ox + ev.clientX - drag.x; oy = drag.oy + ev.clientY - drag.y; draw(); return; }
    if (ev.target !== cv) return;
    const r = cv.getBoundingClientRect(), mx = ev.clientX - r.left, my = ev.clientY - r.top;
    const hit = hits.find(h => Math.hypot(h.x - mx, h.y - my) <= h.r);
    const tx = Math.floor((mx - ox) / scale), ty = Math.floor((my - oy) / scale);
    const owner = tx >= 0 && ty >= 0 && tx < W && ty < Ht ? ownerOfTile(ty * W + tx, year) : null;
    if (hit?.e) showTip(`<b>${esc(hit.e.t)}</b><br><span class="muted">${hit.e.year}, ${esc(kindWords(hit.e.kind))}</span>`, ev.clientX, ev.clientY);
    else if (hit?.town) { const t = hit.town, o = ownerAt(t, year); showTip(`<b>${esc(t.n)}</b><br><span class="muted">${t.d != null && t.d <= year ? `ruin since ${t.d}` : `${esc(kindWords(t.t))} of ${esc(people(o)?.n ?? "")}`}</span>`, ev.clientX, ev.clientY); }
    else if (owner != null) showTip(`${esc(people(owner)?.n ?? "")}<br><span class="muted">${tx},${ty}</span>`, ev.clientX, ev.clientY);
    else closeTip();
    cv.style.cursor = hit ? "pointer" : "default";
  };
  const up = ev => {
    if (!drag) return; const moved = drag.moved; drag = null;
    if (moved || ev.target !== cv) return;
    const r = cv.getBoundingClientRect(), mx = ev.clientX - r.left, my = ev.clientY - r.top;
    const hit = hits.find(h => Math.hypot(h.x - mx, h.y - my) <= h.r);
    if (hit?.e) openDrawer(eventPage(hit.e.id)); else if (hit?.town) openDrawer(entityPage("S" + hit.town.i));
  };
  window.addEventListener("pointermove", move); window.addEventListener("pointerup", up);
  const onResize = () => draw(); window.addEventListener("resize", onResize);
  cleanup = () => { if (timer) clearInterval(timer); window.removeEventListener("pointermove", move); window.removeEventListener("pointerup", up); window.removeEventListener("resize", onResize); };
  if (!MAP_IMG && H.meta.map) { MAP_IMG = new Image(); MAP_IMG.onload = () => draw(); MAP_IMG.src = H.meta.map.file; }
  fit(); buildTerritory(); draw();
}

// ---------------------------------------------------------------------------------------------
// Checks: what a debugger of the history wants to see at once

function checksView() {
  const byKind = new Map();
  for (const e of E) {
    if (!byKind.has(e.kind)) byKind.set(e.kind, { n: 0, caused: 0, placed: 0, fx: 0 });
    const k = byKind.get(e.kind); k.n++;
    if (causesOf(e).length) k.caused++;
    if (e.x >= 0) k.placed++;
    if (effectsOf(e).length) k.fx++;
  }
  const caused = E.filter(e => causesOf(e).length).length;
  const orphans = E.filter(e => !causesOf(e).length && !effectsOf(e).length);
  const unplaced = E.filter(e => e.x < 0).length;
  const cycles = findCycles();
  const deep = depths();
  const hist = new Map(); for (const d of deep.values()) hist.set(d, (hist.get(d) || 0) + 1);
  const maxDepth = Math.max(0, ...hist.keys());
  const odd = oddLives(), oddTowns = oddTownsList();
  const v = H.checks.violations || [];
  const card = (n, l, bad) => `<div class="card ${bad ? "bad" : ""}"><div class="n">${n}</div><div class="l">${l}</div></div>`;
  const pct = (a, b) => b ? Math.round(100 * a / b) + "%" : "–";
  let o = `<div class="checks"><p class="eyebrow">checks</p><h1>The history examined</h1>`;
  o += `<div class="cards">${card(E.length, "events")}${card(pct(caused, E.length), "with a recorded cause")}${card(v.length, "contradictions", v.length > 0)}${card(cycles.length, "causal cycles", cycles.length > 0)}${card(orphans.length, "events tied to nothing")}${card(pct(unplaced, E.length), "with no place")}${card(odd.length, "impossible lives", odd.length > 0)}${card(oddTowns.length, "impossible towns", oddTowns.length > 0)}${card(maxDepth, "longest causal chain")}</div>`;
  o += `<h2>Events per decade</h2><canvas id="ck-dec"></canvas>`;
  o += `<h2>Contradictions</h2>${v.length ? `<ul>${v.slice(0, 200).map(x => `<li>${esc(x)}</li>`).join("")}</ul>` : `<p class="muted">None: the chronicle is consistent.</p>`}`;
  if (cycles.length) o += `<h2>Causal cycles</h2>${cycles.slice(0, 20).map(c => `<p>${c.map(id => `<a href="#/causes/${id}">#${id}</a>`).join(" → ")}</p>`).join("")}`;
  const kinds = [...byKind.entries()].sort((a, b) => (b[1].n - b[1].caused) - (a[1].n - a[1].caused));
  const bar = (label, a, b, color, href) => `<span>${href ? `<a href="${href}">${esc(label)}</a>` : esc(label)}</span><span class="track"><span class="fill" style="width:${b ? 100 * a / b : 0}%;background:${color}"></span></span><span class="muted">${a} of ${b}</span>`;
  o += `<h2>Causes recorded, by kind</h2><p class="muted">The share of each kind of event that names its cause; uncaused kinds first.</p><div class="bars">${kinds.map(([k, s]) => bar(kindWords(k), s.caused, s.n, catColor(cat(k)), `#/causes`)).join("")}</div>`;
  const unplacedKinds = [...byKind.entries()].filter(([, s]) => s.placed < s.n).sort((a, b) => (b[1].n - b[1].placed) - (a[1].n - a[1].placed));
  o += `<h2>Where it happened, by kind</h2><p class="muted">Kinds recorded without a place cannot be found on the map or by a tile.</p><div class="bars">${unplacedKinds.map(([k, s]) => bar(kindWords(k), s.placed, s.n, catColor(cat(k)))).join("")}</div>`;
  const hubs = E.map(e => [e, effectsOf(e).length]).filter(x => x[1] > 1).sort((a, b) => b[1] - a[1]).slice(0, 20);
  o += `<h2>Causal hubs</h2><p class="muted">Events most others name as their cause: a hub with many unlike effects may be a rule that reaches too far.</p><table><tr><th>effects</th><th>event</th><th>year</th><th>kinds of its effects</th></tr>${hubs.map(([e, n]) => {
    const ks = {}; for (const f of effectsOf(e)) ks[f.kind] = (ks[f.kind] || 0) + 1;
    return `<tr><td>${n}</td><td><a href="#/causes/${e.id}?back=2&fwd=1">${esc(clip(e.t, 60))}</a></td><td>${e.year}</td><td class="muted">${Object.entries(ks).sort((a, b) => b[1] - a[1]).slice(0, 4).map(([k, c]) => `${kindWords(k)} ${c}`).join(", ")}</td></tr>`;
  }).join("")}</table>`;
  o += `<h2>Which kinds cause which</h2><p class="muted">Rows: the kind of the cause; columns: the kind of the effect; the darker, the more links. Click a cell for examples. A rule that links unlike kinds shows as a cell where none is expected.</p><div id="ck-matrix" style="overflow:auto"></div>`;
  o += `<h2>Chain depths</h2><p class="muted">How many links back each event's longest chain of causes runs.</p><div class="bars">${[...Array(Math.min(maxDepth, 40) + 1).keys()].map(d => bar(d + (d === 40 && maxDepth > 40 ? "+" : "") + " links", hist.get(d) || 0, E.length, catColor("politics"))).join("")}</div>`;
  o += `<h2>Impossible lives</h2>${odd.length ? `<ul>${odd.slice(0, 120).map(([code, why]) => `<li>${link(code)}: ${esc(why)}</li>`).join("")}</ul>` : `<p class="muted">None found (births, deaths, ages, parents, spouses and children agree).</p>`}`;
  o += `<h2>Impossible towns</h2>${oddTowns.length ? `<ul>${oddTowns.slice(0, 120).map(([code, why]) => `<li>${link(code)}: ${esc(why)}</li>`).join("")}</ul>` : `<p class="muted">None found.</p>`}`;
  o += `<h2>Events tied to nothing</h2><p class="muted">No cause and no effect, by kind.</p><div class="bars">${Object.entries(orphans.reduce((m, e) => (m[e.kind] = (m[e.kind] || 0) + 1, m), {})).sort((a, b) => b[1] - a[1]).slice(0, 25).map(([k, n]) => bar(kindWords(k), n, byKind.get(k).n, catColor(cat(k)))).join("")}</div>`;
  view.innerHTML = o + `</div>`;
  causeMatrix($("#ck-matrix"));
  // Events per decade, stacked by category.
  const cv = $("#ck-dec"); const { g, w, h } = fitCanvas(cv);
  const dec = new Map(); for (const e of E) { const d = Math.floor(e.year / 10) * 10; if (!dec.has(d)) dec.set(d, {}); const c = cat(e.kind); dec.get(d)[c] = (dec.get(d)[c] || 0) + 1; }
  const ds = [...dec.keys()].sort((a, b) => a - b); if (!ds.length) return;
  const d0 = ds[0], d1 = ds[ds.length - 1] + 10;
  let maxv = 1; for (const v of dec.values()) maxv = Math.max(maxv, Object.values(v).reduce((a, b) => a + b, 0));
  const order = [...CATS.map(c => c[0]), "other"];
  const bw = (w - 40) / ((d1 - d0) / 10);
  for (const [d, v] of dec) { let acc = 0; for (const c of order) { const n = v[c] || 0; if (!n) continue; const hh = n / maxv * (h - 30); g.fillStyle = catColor(c); g.fillRect(30 + (d - d0) / 10 * bw, h - 18 - acc - hh, Math.max(1, bw - 1), hh); acc += hh; } }
  g.fillStyle = css("--ink-soft"); g.font = `11px ${css("--label")}`;
  for (let d = d0; d < d1; d += niceStep(bw / 10, 50)) { if (d % 10) continue; g.fillText(String(d), 30 + (d - d0) / 10 * bw, h - 4); }
  g.fillText(String(maxv), 2, 12);
}

// A heatmap of cause kind -> effect kind, with examples on click.
function causeMatrix(el) {
  const cells = new Map(), rowsN = new Map(), colsN = new Map();
  for (const e of E) for (const c of causesOf(e)) {
    const k = c.kind + "\u0000" + e.kind;
    if (!cells.has(k)) cells.set(k, []);
    cells.get(k).push([c, e]);
    rowsN.set(c.kind, (rowsN.get(c.kind) || 0) + 1); colsN.set(e.kind, (colsN.get(e.kind) || 0) + 1);
  }
  const rk = [...rowsN.entries()].sort((a, b) => b[1] - a[1]).slice(0, 30).map(x => x[0]);
  const ck = [...colsN.entries()].sort((a, b) => b[1] - a[1]).slice(0, 30).map(x => x[0]);
  let max = 1; for (const v of cells.values()) max = Math.max(max, v.length);
  const cs = 16;
  let html = `<table style="border-collapse:collapse;font-size:11px"><tr><td></td>${ck.map(k => `<th style="writing-mode:vertical-rl;transform:rotate(180deg);font-family:var(--label);font-weight:400;padding:2px;height:9rem;text-align:left;border:0">${esc(kindWords(k))}</th>`).join("")}</tr>`;
  for (const r of rk) {
    html += `<tr><th style="text-align:right;font-family:var(--label);font-weight:400;padding:0 6px;white-space:nowrap;border:0">${esc(kindWords(r))}</th>`;
    for (const c of ck) {
      const v = cells.get(r + "\u0000" + c);
      const a = v ? 0.15 + 0.85 * Math.sqrt(v.length / max) : 0;
      html += `<td data-k="${esc(r)}|${esc(c)}" title="${esc(kindWords(r))} → ${esc(kindWords(c))}: ${v ? v.length : 0}" style="width:${cs}px;height:${cs}px;padding:0;border:1px solid var(--page);background:${v ? alpha(catColor(cat(c)).startsWith("#") ? catColor(cat(c)) : "#9c3324", a) : "var(--page-deep)"};cursor:${v ? "pointer" : "default"}"></td>`;
    }
    html += `</tr>`;
  }
  el.innerHTML = html + `</table>`;
  el.addEventListener("click", ev => {
    const td = ev.target.closest("td[data-k]"); if (!td) return;
    const [r, c] = td.dataset.k.split("|");
    const v = cells.get(r + "\u0000" + c); if (!v) return;
    openDrawer(`<p class="eyebrow">${v.length} links</p><h1>${esc(kindWords(r))} → ${esc(kindWords(c))}</h1><div class="chron">${v.slice(0, 80).map(([a, b]) => `<p><span class="muted">${a.year}</span> <a href="#/event/${a.id}">${esc(a.t)}</a><br>&nbsp;&nbsp;→ <span class="muted">${b.year}</span> <a href="#/causes/${b.id}?back=2&fwd=1">${esc(b.t)}</a></p>`).join("")}</div>`);
  });
}

// Cycles in the cause graph (there should be none: a cause comes before its effect).
function findCycles() {
  const state = new Map(), out = [];
  for (const start of E) {
    if (state.has(start.id)) continue;
    const stack = [[start, 0]], path = [];
    while (stack.length && out.length < 50) {
      const [e, i] = stack[stack.length - 1];
      if (i === 0) { state.set(e.id, 1); path.push(e.id); }
      const cs = causesOf(e);
      if (i < cs.length) {
        stack[stack.length - 1][1]++;
        const c = cs[i], s = state.get(c.id);
        if (s === 1) out.push([...path.slice(path.indexOf(c.id)), c.id]);
        else if (!s) stack.push([c, 0]);
      } else { state.set(e.id, 2); path.pop(); stack.pop(); }
    }
  }
  return out;
}

function oddLives() {
  const out = [];
  for (const f of H.figures) {
    const code = "P" + f.i;
    if (f.d != null && f.d < f.b) out.push([code, `dies in ${f.d}, before being born in ${f.b}`]);
    if ((f.d ?? LAST) - f.b > 110) out.push([code, `lives ${(f.d ?? LAST) - f.b} years`]);
    for (const p of f.pa) {
      const pf = fig(p); if (!pf) continue;
      if (f.b - pf.b < 12) out.push([code, `born when ${pf.n} was ${f.b - pf.b}`]);
      if (!pf.ch.includes(f.i)) out.push([code, `${pf.n} is a parent who does not list them as a child`]);
      if (pf.d != null && f.b > pf.d + 1) out.push([code, `born in ${f.b}, after their parent ${pf.n} died in ${pf.d}`]);
    }
    if (f.sp != null) { const s = fig(f.sp); if (s && s.sp !== f.i) out.push([code, `married to ${s.n}, who is married to ${s.sp != null ? fig(s.sp)?.n ?? "someone unknown" : "no one"}`]); }
    for (const c of f.ch) { const cf = fig(c); if (cf && !cf.pa.includes(f.i)) out.push([code, `lists ${cf.n} as a child, who does not name them as a parent`]); }
  }
  // Events naming the dead.
  const died = new Map(H.figures.filter(f => f.d != null).map(f => [f.i, f.d]));
  for (const e of E) for (const c of e.parts) if (c[0] === "P") { const d = died.get(+c.slice(1)); if (d != null && e.year > d + 1 && !/Died|Death|Memorial|Remember|Artifact|Monument|Ghost|Legend|Tale|Found|Lost/.test(e.kind)) out.push([c, `named in "${e.t}" (${e.year}), ${e.year - d} years after dying`]); }
  return out;
}

function oddTownsList() {
  const out = [];
  for (const t of H.towns) {
    const code = "S" + t.i;
    if (t.d != null && t.d < t.f) out.push([code, `razed in ${t.d}, before its founding in ${t.f}`]);
    if (t.d == null && !t.pop) out.push([code, "standing with no one living there"]);
    const evs = BY.get(code) || [];
    for (const e of evs) if (t.d != null && e.year > t.d + 1 && /Siege|Battle|Raid|Crown|Trade/.test(e.kind)) { out.push([code, `${kindWords(e.kind)} in ${e.year}, after it was razed in ${t.d}`]); break; }
    for (const e of evs) if (e.year < t.f - 1) { out.push([code, `named in ${e.year}, before its founding in ${t.f}`]); break; }
  }
  return out;
}

// ---------------------------------------------------------------------------------------------
// Compare: this history beside another of the same land (a branch, or after a change)

const B = window.HISTORY_B || null;
if (B) $("#tab-compare").hidden = false;
function compareView() {
  if (!B) { view.innerHTML = `<div class="reader"><div class="page"><h1>Nothing to compare</h1><p class="lede">Write the explorer with a second history: <code>history_explorer {"compare": "&lt;world id&gt;"}</code> (a <code>history_branch</code> gives one).</p></div></div>`; return; }
  const EB = B.events;
  const firstDiff = (() => { const n = Math.min(E.length, EB.length); for (let i = 0; i < n; i++) if (E[i].t !== EB[i][4]) return i; return n < Math.max(E.length, EB.length) ? n : -1; })();
  const aliveA = new Set(H.peoples.filter(p => p.d == null).map(p => `${p.n} (from ${p.f})`)), aliveB = new Set(B.peoples.filter(p => p.d == null).map(p => `${p.n} (from ${p.f})`));
  const standA = new Set(H.towns.filter(t => t.d == null).map(t => t.n)), standB = new Set(B.towns.filter(t => t.d == null).map(t => t.n));
  const only = (a, b) => [...a].filter(x => !b.has(x)).sort();
  const count = (evs, re) => evs.filter(e => re.test(Array.isArray(e) ? B.kinds[e[3]] : e.kind)).length;
  const row = (label, a, b) => `<tr><td>${label}</td><td>${a}</td><td>${b}</td><td class="muted">${typeof a === "number" && typeof b === "number" ? (b - a > 0 ? "+" : "") + (b - a) : ""}</td></tr>`;
  let o = `<div class="checks"><p class="eyebrow">compare</p><h1>${esc(H.meta.id)} and ${esc(B.meta.id)}</h1>`;
  o += firstDiff < 0 ? `<p class="lede">The two chronicles are the same.</p>` : `<p class="lede">They part at event ${firstDiff + 1} of ${E.length}, in ${E[firstDiff]?.year ?? EB[firstDiff][1]}: here “${esc(E[firstDiff]?.t ?? "")}”, there “${esc(EB[firstDiff]?.[4] ?? "")}”.</p>`;
  o += `<table><tr><th></th><th>${esc(H.meta.id)}</th><th>${esc(B.meta.id)}</th><th></th></tr>`
    + row("events", E.length, EB.length) + row("wars", H.wars.length, B.wars.length) + row("battles", count(E, /BattleFought/), count(EB, /BattleFought/))
    + row("towns razed", count(E, /SettlementDestroyed/), count(EB, /SettlementDestroyed/)) + row("peoples enduring", aliveA.size, aliveB.size)
    + row("towns standing", standA.size, standB.size) + row("people of note", H.figures.length, B.figures.length) + `</table>`;
  o += `<p class="muted">${esc(H.meta.sentence)}<br>${esc(B.meta.sentence)}</p>`;
  o += `<h2>The peoples' lives in both</h2><p class="muted">Upper bar: ${esc(H.meta.id)}; lower: ${esc(B.meta.id)}; a full colour where the two differ. Peoples are matched by name and founding.</p><canvas id="cmp-lives" style="height:${Math.max(160, (new Set([...H.peoples, ...B.peoples].map(p => p.n + "|" + p.f)).size) * 22 + 40)}px"></canvas>`;
  o += `<h2>Events per decade</h2><canvas id="cmp-dec"></canvas>`;
  const lists = [["Peoples enduring only here", only(aliveA, aliveB)], ["Peoples enduring only there", only(aliveB, aliveA)], ["Towns standing only here", only(standA, standB)], ["Towns standing only there", only(standB, standA)]];
  for (const [t, l] of lists) o += `<h2>${t} <span class="muted">${l.length}</span></h2><p>${l.map(esc).join(", ") || `<span class="muted">none</span>`}</p>`;
  view.innerHTML = o + `</div>`;
  // Lives.
  { const cv = $("#cmp-lives"), { g, w } = fitCanvas(cv);
    // Peoples are matched by name and founding year (two peoples may share a name).
    const key = p => p.n + "|" + p.f;
    const keys = [...new Set([...H.peoples, ...B.peoples].sort((a, b) => a.f - b.f).map(key))];
    const first = Math.min(...H.peoples.map(p => p.f), ...B.peoples.map(p => p.f), H.meta.first), last = Math.max(LAST, B.meta.last), L = 210, X = y => L + (y - first) / (last - first) * (w - L - 20);
    g.font = `12px ${css("--body")}`;
    keys.forEach((k, i) => {
      const y = 24 + i * 22, a = H.peoples.find(p => key(p) === k), b = B.peoples.find(p => key(p) === k), n = (a || b).n;
      g.fillStyle = css("--ink"); g.fillText(clip(n, 26), 6, y + 12);
      for (const [p, dy, other] of [[a, 0, b], [b, 8, a]]) {
        if (!p) continue;
        const differs = !other || other.d !== p.d;
        g.fillStyle = alpha(p.c, differs ? 0.9 : 0.4); g.fillRect(X(p.f), y + dy + 2, X(p.d ?? last) - X(p.f), 6);
        if (p.d != null) { g.fillStyle = css("--ink"); g.fillText("†", X(p.d) + 2, y + dy + 9); }
      }
    });
    g.fillStyle = css("--ink-soft"); g.font = `11px ${css("--label")}`;
    for (let y = Math.ceil(first / 50) * 50; y <= last; y += 50) g.fillText(String(y), X(y), 14);
  }
  // Decades.
  { const cv = $("#cmp-dec"), { g, w, h } = fitCanvas(cv);
    const dec = (evs, yr) => { const m = new Map(); for (const e of evs) { const d = Math.floor(yr(e) / 10) * 10; m.set(d, (m.get(d) || 0) + 1); } return m; };
    const a = dec(E, e => e.year), b = dec(EB, e => e[1]);
    const ds = [...new Set([...a.keys(), ...b.keys()])].sort((x, y) => x - y); if (!ds.length) return;
    const max = Math.max(...a.values(), ...b.values()), bw = (w - 40) / ds.length;
    ds.forEach((d, i) => {
      const x = 30 + i * bw;
      g.fillStyle = css("--c-trade"); g.fillRect(x, h - 18 - (a.get(d) || 0) / max * (h - 30), bw / 2 - 1, (a.get(d) || 0) / max * (h - 30));
      g.fillStyle = css("--c-war"); g.fillRect(x + bw / 2, h - 18 - (b.get(d) || 0) / max * (h - 30), bw / 2 - 1, (b.get(d) || 0) / max * (h - 30));
      if (d % 50 === 0) { g.fillStyle = css("--ink-soft"); g.font = `11px ${css("--label")}`; g.fillText(String(d), x, h - 4); }
    });
    g.fillStyle = css("--c-trade"); g.fillRect(w - 230, 6, 10, 10); g.fillStyle = css("--ink"); g.fillText(H.meta.id, w - 215, 15);
    g.fillStyle = css("--c-war"); g.fillRect(w - 230, 22, 10, 10); g.fillStyle = css("--ink"); g.fillText(B.meta.id, w - 215, 31);
  }
}

route();
