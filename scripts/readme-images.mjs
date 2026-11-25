// Generates the README illustrations (docs/images/*.svg) from the real shell code, so the
// pictures always match what HUE draws. Run: node scripts/readme-images.mjs
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { buildGhostSvg, buildShellSvg, setDecor } from "../src/pet/shell.ts";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outDir = path.join(root, "docs", "images");
mkdirSync(outDir, { recursive: true });

const FONT = `'Segoe UI', 'Helvetica Neue', Arial, sans-serif`;
const GLYPH_FONT = `'Segoe UI', 'Segoe UI Symbol', 'Yu Gothic', 'Helvetica Neue', Arial, sans-serif`;
const C = {
  bg0: "#0d1118",
  bg1: "#161d2b",
  card: "#161c28",
  line: "#263042",
  text: "#f2f4f8",
  soft: "#c3cad6",
  mute: "#8a95a8",
  orange: "#ff8a1f",
  blue: "#5ec8ff",
};

// ───────────────────────── example machines (fictional) ─────────────────────────

const PAL = {
  graphite: ["#2a2f3a", "#1d2129", "#3d4555"],
  slate: ["#323848", "#232836", "#4a5266"],
  navy: ["#1f2a3c", "#151d2b", "#34445f"],
  mocha: ["#3a3232", "#2a2424", "#564a4a"],
  silver: ["#c9ced8", "#a8aebb", "#e6e9ef"],
  beige: ["#e8e6e1", "#c9c6bf", "#ffffff"],
  forest: ["#263832", "#1a2823", "#3a5248"],
  plum: ["#35283f", "#261c2e", "#4c3a5a"],
};

function spec(form, pal, accent, extra = {}) {
  const [body, body_dark, trim] = PAL[pal];
  return {
    form, body, body_dark, trim, accent,
    corner: 12, width_scale: 1, vents: 4, pattern: "none", feet: "pads",
    side_fins: false, leds: 1, ears: false,
    ...extra,
  };
}

const NVIDIA = "#76b900";
const AMD = "#ed1c24";
const INTEL = "#0071c5";

const M = {
  tower: spec("tower", "graphite", NVIDIA, { pattern: "grid", side_fins: true, leds: 3, vents: 8, corner: 10, width_scale: 1.04 }),
  laptop: spec("laptop", "silver", INTEL, { leds: 1, vents: 4, feet: "none", corner: 10, width_scale: 0.98 }),
  desktop: spec("desktop", "mocha", AMD, { pattern: "stripes", side_fins: true, leds: 2, vents: 6, feet: "round", corner: 18, width_scale: 1.08, ears: true }),
  aio: spec("aio", "beige", "hsl(200, 70%, 55%)", { pattern: "dots", feet: "block", corner: 8 }),
  mini: spec("mini", "navy", INTEL, { pattern: "circuit", leds: 2, vents: 2, feet: "round", corner: 14, width_scale: 0.95 }),
  handheld: spec("handheld", "plum", AMD, { feet: "none" }),
  plain: spec("desktop", "graphite", "#ff8a1f", { leds: 1, vents: 4, corner: 16 }),
};

/** Where the feet touch the ground, in the shell's 120-unit space. */
const GROUND = { tower: 108, desktop: 103, laptop: 99.5, aio: 100.5, mini: 98, handheld: 85 };

// ───────────────────────── pet builder ─────────────────────────

let uid = 0;

/** Make ids unique so several pets can live in one SVG. */
function scopeIds(markup) {
  const n = ++uid;
  return markup
    .replace(/id="([^"]+)"/g, `id="$1-${n}"`)
    .replace(/url\(#([^)]+)\)/g, `url(#$1-${n})`);
}

function innerOf(svg) {
  return svg.replace(/^[\s\S]*?<svg[^>]*>/, "").replace(/<\/svg>\s*$/, "");
}

/**
 * Pet markup in its own 120×120 space.
 * opts: face, blush, hat, gaze [x, y], screen ({ big } | { label, value } | { glyph })
 */
function petInner(s, opts = {}) {
  let svg = buildShellSvg(s);
  if (opts.hat) {
    const g = { innerHTML: "" };
    setDecor({ querySelector: () => g }, opts.hat);
    svg = svg.replace('<g id="decor"></g>', `<g id="decor">${g.innerHTML}</g>`);
  }
  svg = svg.replace(/<g class="face face-(\w+)"[^>]*>[\s\S]*?<\/g>/g, (m, name) =>
    name === opts.face ? m.replace('class="face ', 'class="shown-face ') : "",
  );
  if (opts.face) svg = svg.replace(/<g id="eyes-normal">[\s\S]*?<\/g>/, "");
  svg = svg.replace(/<g id="dizzy-fx"[\s\S]*?<\/g>/, "");
  svg = svg.replace('<g id="blush" opacity="0">', `<g id="blush" opacity="${opts.blush ?? 0}">`);
  if (opts.gaze) {
    svg = svg.replace('<g id="eyes" ', `<g id="eyes" transform="translate(${opts.gaze[0]} ${opts.gaze[1]})" `);
  }
  if (opts.screen) {
    svg = svg.replace('<g id="eyes" ', '<g id="eyes" opacity="0" ');
    const t = opts.screen;
    let text = "";
    const base = `text-anchor="middle" fill="${C.orange}" font-family="${GLYPH_FONT}"`;
    if (t.big) text = `<text x="60" y="58.5" ${base} font-size="12.2" font-weight="700" letter-spacing="0.5">${t.big}</text>`;
    else if (t.glyph) text = `<text x="60" y="57.5" ${base} font-size="9.8" font-weight="600">${t.glyph}</text>`;
    else
      text =
        `<text x="60" y="51" ${base} font-size="5.7" font-weight="700" letter-spacing="0.6" opacity="0.85">${t.label}</text>` +
        `<text x="60" y="62" ${base} font-size="9.8" font-weight="700">${t.value}</text>`;
    svg = svg.replace('<g id="decor">', `${text}<g id="decor">`);
  }
  return scopeIds(innerOf(svg));
}

/** Pet placed at (x, y) with a given pixel size; `cls` on a wrapper for CSS animation. */
function pet(s, x, y, size, opts = {}) {
  const inner = petInner(s, opts);
  const box = opts.viewBox ?? "0 0 120 120";
  const [, , vw, vh] = box.split(" ").map(Number);
  const h = (size * vh) / vw;
  const svg = `<svg x="${x}" y="${y}" width="${size}" height="${h.toFixed(1)}" viewBox="${box}" overflow="${opts.viewBox ? "hidden" : "visible"}"><g filter="url(#ds)">${inner}</g></svg>`;
  return opts.cls ? `<g class="${opts.cls}"${opts.style ? ` style="${opts.style}"` : ""}>${svg}</g>` : svg;
}

/** Pet standing on a ground line at y=groundY (x = left edge). */
function standing(s, x, groundY, size, opts = {}) {
  const g = GROUND[s.form] ?? 103;
  return pet(s, x, groundY - (g * size) / 120, size, opts);
}

const V1_PET = `
<defs><clipPath id="v1screen"><rect x="28" y="34" width="64" height="40" rx="8"/></clipPath></defs>
<rect x="18" y="22" width="84" height="76" rx="18" fill="#2a2f3a" stroke="#3d4555" stroke-width="3"/>
<rect x="28" y="34" width="64" height="40" rx="8" fill="#0b0d12"/>
<g clip-path="url(#v1screen)">
  <ellipse class="eye" cx="46" cy="54" rx="8" ry="10" fill="${C.orange}"/>
  <ellipse class="eye" cx="74" cy="54" rx="8" ry="10" fill="${C.orange}"/>
</g>`;

// ───────────────────────── shared chrome ─────────────────────────

const COMMON_STYLE = `
  text { font-family: ${FONT}; }
  .eye { transform-box: fill-box; transform-origin: 50% 50%; animation: blink 5.2s ease-in-out infinite; }
  .led { animation: led 3.2s ease-in-out infinite; }
  @keyframes blink { 0%, 90%, 100% { transform: scale(1, 1); } 93%, 95% { transform: scale(1, 0.15); } }
  @keyframes led { 0%, 100% { opacity: 0.35; } 50% { opacity: 1; } }
`;

function doc(w, h, body, { style = "", defs = "" } = {}) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">
<style>${COMMON_STYLE}${style}</style>
<defs>
  <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${C.bg0}"/><stop offset="1" stop-color="${C.bg1}"/></linearGradient>
  <filter id="ds" x="-25%" y="-25%" width="150%" height="160%"><feDropShadow dx="0" dy="3" stdDeviation="3" flood-color="#000" flood-opacity="0.45"/></filter>
  <filter id="glow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="18"/></filter>
  ${defs}
</defs>
<rect x="1" y="1" width="${w - 2}" height="${h - 2}" rx="22" fill="url(#bg)" stroke="${C.line}" stroke-width="2"/>
${body}
</svg>
`;
}

function pill(x, y, label, { fill = "#222a39", color = C.soft, w } = {}) {
  const width = w ?? label.length * 8.4 + 26;
  return `<g><rect x="${x}" y="${y}" width="${width}" height="30" rx="15" fill="${fill}"/><text x="${x + width / 2}" y="${y + 20}" text-anchor="middle" font-size="15" font-weight="600" fill="${color}">${label}</text></g>`;
}

function taskbar(x, y, w, h = 36) {
  let icons = "";
  const n = Math.floor((w - 40) / 46);
  const start = x + w / 2 - (n * 46) / 2;
  for (let i = 0; i < n; i++) {
    const ix = start + i * 46 + 9;
    icons += `<rect x="${ix}" y="${y + 8}" width="${h - 16}" height="${h - 16}" rx="5" fill="${i === 0 ? "#2d6fb8" : "#2a3242"}" opacity="${i === 0 ? 0.9 : 0.8}"/>`;
  }
  return `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="10" fill="#1c2331" stroke="#2a3446"/>${icons}`;
}

function bubble(x, y, text, { tailX } = {}) {
  const w = text.length * 8.6 + 32;
  const tx = tailX ?? x + 34;
  return `<g filter="url(#ds)">
    <rect x="${x}" y="${y}" width="${w}" height="38" rx="14" fill="#fbfcfe"/>
    <path d="M${tx - 9} ${y + 37} L${tx} ${y + 50} L${tx + 9} ${y + 37} Z" fill="#fbfcfe"/>
  </g>
  <text x="${x + w / 2}" y="${y + 25}" text-anchor="middle" font-size="16" font-weight="600" fill="#1b2130">${text}</text>`;
}

function write(name, svg) {
  writeFileSync(path.join(outDir, name), svg);
  console.log("wrote docs/images/" + name);
}

// ───────────────────────── 1. hero ─────────────────────────

{
  const ground = 338;
  const pets =
    standing(M.tower, 652, ground, 150, { cls: "bob", style: "animation-delay:0s", face: "happy", blush: 0.4 }) +
    standing(M.laptop, 800, ground, 150, { cls: "bob", style: "animation-delay:.5s", gaze: [-2.5, 1] }) +
    standing(M.desktop, 945, ground, 150, { cls: "bob", style: "animation-delay:1s", hat: "party_hat", face: "love", blush: 0.75 }) +
    standing(M.handheld, 1088, ground, 150, { cls: "bob", style: "animation-delay:1.5s", gaze: [-3, 0] });
  const body = `
  <circle cx="930" cy="250" r="190" fill="${C.orange}" opacity="0.10" filter="url(#glow)"/>
  <text x="72" y="168" font-size="118" font-weight="800" fill="${C.text}" letter-spacing="6">HUE</text>
  <circle cx="350" cy="157" r="10" fill="${C.orange}"/>
  <text x="74" y="220" font-size="27" font-weight="600" fill="${C.soft}">Le compagnon de bureau qui incarne ton PC</text>
  <text x="74" y="256" font-size="17.5" fill="${C.mute}">Une coque, une voix et une personnalité uniques, tirées de ton matériel.</text>
  ${pill(74, 288, "Rust")}${pill(146, 288, "Tauri 2")}${pill(240, 288, "TypeScript")}${pill(370, 288, "Windows")}
  ${pill(476, 288, "v2.0", { fill: C.orange, color: "#1a1206" })}
  ${taskbar(630, ground, 610)}
  ${pets}
  ${bubble(560, 108, "Salut ! Moi, c'est ton PC.", { tailX: 720 })}
  `;
  write(
    "hero.svg",
    doc(1280, 400, body, {
      style: `.bob { animation: bob 3.2s ease-in-out infinite; }
      @keyframes bob { 0%, 100% { transform: translateY(0); } 50% { transform: translateY(-5px); } }`,
    }),
  );
}

// ───────────────────────── 2. V1 → V2 ─────────────────────────

{
  const grid = [
    [M.tower, "Tour · GPU NVIDIA"],
    [M.laptop, "Portable · Intel"],
    [M.desktop, "PC fixe · GPU AMD"],
    [M.aio, "Tout-en-un"],
    [M.mini, "Mini-PC"],
    [M.handheld, "Console portable"],
  ];
  let cells = "";
  grid.forEach(([s, label], i) => {
    const col = i % 3;
    const row = Math.floor(i / 3);
    const cx = 655 + col * 200;
    const gy = 222 + row * 178;
    cells += standing(s, cx - 60, gy, 120, { cls: "bob", style: `animation-delay:${(i * 0.4).toFixed(1)}s` });
    cells += `<text x="${cx}" y="${gy + 26}" text-anchor="middle" font-size="15" font-weight="600" fill="${C.soft}">${label}</text>`;
  });
  const body = `
  <rect x="32" y="32" width="390" height="476" rx="18" fill="${C.card}" stroke="${C.line}"/>
  ${pill(52, 52, "V1", { fill: "#2a3242", color: C.soft })}
  <svg x="107" y="120" width="240" height="240" viewBox="0 0 120 120"><g filter="url(#ds)">${V1_PET}</g></svg>
  <text x="227" y="414" text-anchor="middle" font-size="20" font-weight="700" fill="${C.text}">Un seul look pour tous les PC</text>
  <text x="227" y="444" text-anchor="middle" font-size="15.5" fill="${C.mute}">Coque grise, écran noir, yeux orange.</text>
  <text x="227" y="468" text-anchor="middle" font-size="15.5" fill="${C.mute}">Seule la personnalité changeait.</text>

  <g fill="none" stroke="${C.orange}" stroke-width="4" stroke-linecap="round" stroke-linejoin="round">
    <path d="M438 270 H492"/><path d="M478 256 L494 270 L478 284"/>
  </g>

  <rect x="510" y="32" width="738" height="476" rx="18" fill="${C.card}" stroke="${C.line}"/>
  ${pill(530, 52, "V2", { fill: C.orange, color: "#1a1206" })}
  <text x="590" y="73" font-size="17" font-weight="600" fill="${C.soft}">Une coque générée depuis le matériel de chaque PC</text>
  ${cells}
  `;
  write(
    "v1-vs-v2.svg",
    doc(1280, 540, body, {
      style: `.bob { animation: bob 3.4s ease-in-out infinite; }
      @keyframes bob { 0%, 100% { transform: translateY(0); } 50% { transform: translateY(-4px); } }`,
    }),
  );
}

// ───────────────────────── 3. anatomy ─────────────────────────

{
  const s = M.tower;
  const size = 300;
  const px = 490;
  const py = 110;
  const k = size / 120;
  const P = (u, v) => [px + u * k, py + v * k];
  const w = 74 * s.width_scale;
  const x0 = 60 - w / 2;
  const x1 = 60 + w / 2;
  const callout = (u, v, lx, ly, title, sub, side) => {
    const [ax, ay] = P(u, v);
    const tx = side === "left" ? lx - 14 : lx + 14;
    const anchor = side === "left" ? "end" : "start";
    return `<path d="M${ax.toFixed(1)} ${ay.toFixed(1)} L${lx} ${ly}" stroke="${C.mute}" stroke-width="1.5" fill="none" stroke-dasharray="4 4"/>
    <circle cx="${ax.toFixed(1)}" cy="${ay.toFixed(1)}" r="5" fill="${C.orange}" stroke="#0d1118" stroke-width="2"/>
    <circle cx="${lx}" cy="${ly}" r="3" fill="${C.mute}"/>
    <text x="${tx}" y="${ly - 4}" text-anchor="${anchor}" font-size="17" font-weight="700" fill="${C.text}">${title}</text>
    <text x="${tx}" y="${ly + 18}" text-anchor="${anchor}" font-size="14.5" fill="${C.mute}">${sub}</text>`;
  };
  const body = `
  <text x="640" y="62" text-anchor="middle" font-size="26" font-weight="700" fill="${C.text}">Comment ton matériel dessine sa coque</text>
  <text x="640" y="92" text-anchor="middle" font-size="16" fill="${C.mute}">Exemple : une tour avec un GPU NVIDIA de 16 Go, 3 écrans et un SSD NVMe</text>
  ${pet(s, px, py, size)}
  ${callout(x0 - 1, 37, 420, 150, "Ailettes latérales", "carte graphique avec 8 Go de VRAM ou plus", "left")}
  ${callout(x0 + 6.3, 86, 420, 270, "Couleur d'accent", "marque du GPU : vert NVIDIA, rouge AMD, bleu Intel", "left")}
  ${callout(40, 106, 420, 390, "Pieds et palette", "choisis par la « graine » unique du PC", "left")}
  ${callout(x0 + 18, 20, 860, 135, "LED", "1 par écran (max 3) + 1 si SSD NVMe", "right")}
  ${callout(74, 54, 860, 230, "Écran noir + yeux orange", "identiques sur tous les PC : c'est toujours HUE", "right")}
  ${callout(x1 - 7, 84, 860, 325, "Motif du panneau", "grille, rayures, points, circuit… (GPU gaming = plus chargé)", "right")}
  ${callout(60, 92, 860, 420, "Aérations", "une par paire de cœurs CPU", "right")}
  <rect x="200" y="470" width="880" height="44" rx="12" fill="${C.card}" stroke="${C.line}"/>
  <text x="640" y="498" text-anchor="middle" font-size="15.5" fill="${C.soft}"><tspan font-weight="700" fill="${C.text}">Forme</tspan> ← type de machine   ·   <tspan font-weight="700" fill="${C.text}">Largeur</tspan> ← quantité de RAM   ·   <tspan font-weight="700" fill="${C.text}">Coins</tspan> ← graine du PC</text>
  `;
  write("anatomy.svg", doc(1280, 540, body));
}

// ───────────────────────── 4. faces ─────────────────────────

{
  const faces = [
    [null, "Normal", 0],
    ["happy", "Content", 0.45],
    ["love", "Câlin", 0.75],
    ["sleepy", "Somnolent", 0],
    ["sleep", "Endormi", 0],
    ["sad", "Triste", 0],
    ["strain", "Concentré", 0],
    ["wide", "Surpris", 0],
    ["dead", "K.O.", 0],
  ];
  let tiles = "";
  faces.forEach(([face, label, blush], i) => {
    const x = 34 + i * 136;
    tiles += `<rect x="${x}" y="86" width="126" height="128" rx="14" fill="${C.card}" stroke="${C.line}"/>`;
    tiles += pet(M.plain, x + 7, 98, 112, { face, blush, viewBox: "22 29 76 50" });
    tiles += `<text x="${x + 63}" y="194" text-anchor="middle" font-size="15.5" font-weight="600" fill="${C.soft}">${label}</text>`;
  });
  const body = `
  <text x="640" y="54" text-anchor="middle" font-size="24" font-weight="700" fill="${C.text}">Ses expressions (toujours dans l'écran, sans bouche)</text>
  ${tiles}
  `;
  write("faces.svg", doc(1280, 240, body));
}

// ───────────────────────── 5. loading ─────────────────────────

{
  const ghost = scopeIds(innerOf(buildGhostSvg()));
  const body = `
  <text x="550" y="54" text-anchor="middle" font-size="24" font-weight="700" fill="${C.text}">Au démarrage : un hologramme, puis sa vraie coque</text>
  <svg x="150" y="80" width="200" height="200" viewBox="0 0 120 120" overflow="visible"><g class="ghost">${ghost}</g></svg>
  <text x="250" y="304" text-anchor="middle" font-size="15.5" fill="${C.mute}">Pendant l'analyse du matériel</text>
  <g fill="none" stroke="${C.blue}" stroke-width="4" stroke-linecap="round" stroke-linejoin="round" opacity="0.9">
    <path d="M430 180 H640"/><path d="M624 166 L642 180 L624 194"/>
  </g>
  <text x="535" y="164" text-anchor="middle" font-size="15" font-weight="600" fill="${C.blue}">matérialisation</text>
  <circle cx="850" cy="180" r="96" fill="${C.blue}" opacity="0.16" filter="url(#glow)"/>
  ${pet(M.tower, 750, 80, 200, { face: "happy", blush: 0.35 })}
  <text x="850" y="304" text-anchor="middle" font-size="15.5" fill="${C.mute}">Sa coque unique apparaît</text>
  `;
  write(
    "loading.svg",
    doc(1100, 330, body, {
      style: `
      .ghost { filter: drop-shadow(0 0 6px rgba(94, 200, 255, 0.55)); animation: gfloat 2.6s ease-in-out infinite; }
      .ghost-outline { animation: gdash 1.8s linear infinite; }
      .ghost-scan { animation: gscan 2.4s linear infinite; }
      .ghost-spinner { transform-box: view-box; transform-origin: 60px 46px; animation: gspin 0.9s linear infinite; }
      .ghost-text { animation: gtext 1.6s ease-in-out infinite; font-family: ${FONT}; }
      .ghost-bar { animation: gbar 1.1s cubic-bezier(0.45, 0, 0.55, 1) infinite alternate; }
      @keyframes gfloat { 0%, 100% { opacity: 0.8; transform: translateY(0); } 50% { opacity: 0.96; transform: translateY(-2.5px); } }
      @keyframes gdash { to { stroke-dashoffset: -15; } }
      @keyframes gscan { from { transform: translateY(0); } to { transform: translateY(96px); } }
      @keyframes gspin { to { transform: rotate(360deg); } }
      @keyframes gtext { 0%, 100% { opacity: 0.6; } 50% { opacity: 1; } }
      @keyframes gbar { from { transform: translateX(0); } to { transform: translateX(26px); } }`,
    }),
  );
}

// ───────────────────────── 6. peek ─────────────────────────

{
  const frames = [
    [{ big: "14:32" }, "Heure"],
    [{ label: "CPU", value: "23%" }, "Processeur"],
    [{ glyph: "(•ᴗ•)" }, "Humeur du matin"],
    [{ label: "TEMP", value: "54°" }, "Température"],
    [{ glyph: "(★ᴗ★)" }, "GPU à fond"],
    [{ glyph: "(˘ω˘) zz" }, "La nuit"],
  ];
  const bar = 232;
  let pets = "";
  let labels = "";
  frames.forEach(([screen, label], i) => {
    const x = 60 + i * 196;
    // Tucked: only the top ~73 % shows above the taskbar.
    pets += pet(M.plain, x, bar - 112, 150, { screen });
    labels += `<text x="${x + 75}" y="${bar + 62}" text-anchor="middle" font-size="15" font-weight="600" fill="${C.soft}">${label}</text>`;
  });
  const body = `
  <text x="640" y="52" text-anchor="middle" font-size="24" font-weight="700" fill="${C.text}">Rangé dans le coin, son écran fait défiler l'heure, les stats et son humeur</text>
  ${pets}
  ${taskbar(30, bar, 1220, 40)}
  ${labels}
  `;
  write("peek.svg", doc(1280, 320, body));
}

// ───────────────────────── 7. bounce (V1 vs V2) ─────────────────────────

{
  // V2: squash target from the backend (impact 0.28, relaxes ×0.72 per 1/30 s) chased by the UI spring.
  const K = 520;
  const D = 13;
  const v2 = [];
  {
    let x = 1, vx = 0, y = 1, vy = 0, tx = 1, ty = 1;
    for (let f = 0; f <= 60; f++) {
      if (f === 0) { tx = 0.72; ty = 1.21; } else if (f % 2 === 0) { tx = 1 + (tx - 1) * 0.72; ty = 1 + (ty - 1) * 0.72; }
      vx += (K * (tx - x) - D * vx) / 60; x += vx / 60;
      vy += (K * (ty - y) - D * vy) / 60; y += vy / 60;
      v2.push([x, y]);
    }
  }
  // V1: fixed 0.52 × 1.42 on every hit, decayed ×0.92 and drawn raw at 30 Hz.
  const v1 = [];
  {
    let x = 0.52;
    for (let f = 0; f <= 30; f++) { v1.push(x); x = 1 + (x - 1) * 0.92; }
  }

  // Filmstrip of the V2 hit against a wall.
  const picks = [[0, "impact"], [4, "écrasé"], [9, "il se détend"], [14, "léger rebond"], [22, "effet gelée"], [40, "stable"]];
  let strip = "";
  picks.forEach(([f, label], i) => {
    const [sx, sy] = f === 0 ? [1, 1] : v2[f];
    const x = 70 + i * 196;
    const ox = x + 60;
    const oy = 190;
    strip += `<rect x="${x - 16}" y="84" width="8" height="120" rx="3" fill="#2e3850"/>`;
    strip += `<g transform="translate(${ox} ${oy}) scale(${sx.toFixed(3)} ${sy.toFixed(3)}) translate(${-ox} ${-oy})">${standing(M.plain, x, 204, 120)}</g>`;
    strip += `<text x="${x + 60}" y="232" text-anchor="middle" font-size="15" font-weight="600" fill="${C.soft}">${label}</text>`;
  });

  // Chart of squash X over one second.
  const cx0 = 150, cx1 = 1180, cy0 = 300, cy1 = 520;
  const X = (t) => cx0 + (t / 1) * (cx1 - cx0);
  const Y = (v) => cy1 - ((v - 0.45) / (1.1 - 0.45)) * (cy1 - cy0);
  let v1path = `M${X(0)} ${Y(v1[0])}`;
  v1.forEach((v, i) => { if (i > 0) v1path += ` H${X(i / 30).toFixed(1)} V${Y(v).toFixed(1)}`; });
  v1path += ` H${X(1)}`;
  const v2path = "M" + [[0, 1]].concat(v2.map(([x], i) => [(i + 1) / 60, x])).map(([t, v]) => `${X(Math.min(1, t)).toFixed(1)} ${Y(v).toFixed(1)}`).join(" L");
  let grid = "";
  for (const v of [0.5, 0.7, 0.9, 1.0]) {
    grid += `<line x1="${cx0}" y1="${Y(v)}" x2="${cx1}" y2="${Y(v)}" stroke="${C.line}" stroke-width="1" ${v === 1 ? "" : 'stroke-dasharray="3 5"'}/>`;
    grid += `<text x="${cx0 - 12}" y="${Y(v) + 5}" text-anchor="end" font-size="13" fill="${C.mute}">${v.toFixed(1).replace(".", ",")}</text>`;
  }
  for (const t of [0, 0.25, 0.5, 0.75, 1]) {
    grid += `<text x="${X(t)}" y="${cy1 + 24}" text-anchor="middle" font-size="13" fill="${C.mute}">${t.toString().replace(".", ",")} s</text>`;
  }
  const body = `
  <text x="640" y="52" text-anchor="middle" font-size="24" font-weight="700" fill="${C.text}">Rebonds V2 : écrasement selon la force du choc + ressort fluide</text>
  ${strip}
  ${grid}
  <text x="${cx0 - 70}" y="${(cy0 + cy1) / 2}" font-size="13" fill="${C.mute}" transform="rotate(-90 ${cx0 - 70} ${(cy0 + cy1) / 2})" text-anchor="middle">largeur (× normale)</text>
  <path d="${v1path}" fill="none" stroke="#8a95a8" stroke-width="2.5" stroke-linejoin="round"/>
  <path d="${v2path}" fill="none" stroke="${C.orange}" stroke-width="3.5" stroke-linejoin="round" stroke-linecap="round"/>
  <g font-size="14.5">
    <rect x="${X(0.56)}" y="${Y(0.76) - 5}" width="16" height="4" rx="2" fill="#8a95a8"/>
    <text x="${X(0.56) + 24}" y="${Y(0.76)}" fill="${C.soft}">V1 : toujours 0,52 × 1,42, puis retour saccadé (30 Hz)</text>
    <rect x="${X(0.56)}" y="${Y(0.68) - 5}" width="16" height="4" rx="2" fill="${C.orange}"/>
    <text x="${X(0.56) + 24}" y="${Y(0.68)}" fill="${C.soft}">V2 : proportionnel au choc, ressort amorti (60 FPS)</text>
  </g>
  `;
  write("bounce.svg", doc(1280, 570, body));
}

// ───────────────────────── 8. architecture ─────────────────────────

{
  const box = (x, y, w, h, title, lines, accent = C.orange) =>
    `<g><rect x="${x}" y="${y}" width="${w}" height="${h}" rx="14" fill="${C.card}" stroke="${C.line}"/>
    <rect x="${x}" y="${y + 14}" width="4" height="${h - 28}" rx="2" fill="${accent}"/>
    <text x="${x + 20}" y="${y + 30}" font-size="16.5" font-weight="700" fill="${C.text}">${title}</text>
    ${lines.map((l, i) => `<text x="${x + 20}" y="${y + 54 + i * 21}" font-size="14" fill="${C.mute}">${l}</text>`).join("")}</g>`;
  const body = `
  <text x="640" y="52" text-anchor="middle" font-size="24" font-weight="700" fill="${C.text}">Architecture de la V2</text>

  <text x="50" y="96" font-size="14" font-weight="700" fill="${C.blue}" letter-spacing="1.5">INTERFACE · TYPESCRIPT + VITE (WEBVIEW2)</text>
  ${box(40, 110, 360, 128, "Fenêtre HUE", ["Coque SVG, yeux, expressions, particules", "Ressort squash &amp; stretch (60 FPS)", "Discussion, micro, glisser-déposer"], C.blue)}
  ${box(40, 254, 360, 86, "Bulle de dialogue", ["Fenêtre séparée, collée au-dessus de lui"], C.blue)}
  ${box(40, 356, 360, 86, "Paramètres", ["Général · Toi · IA · Voix · Succès · Identité"], C.blue)}
  ${box(40, 458, 360, 86, "Voix Animalese", ["Web Audio : hauteur et timbre selon le PC"], C.blue)}

  <rect x="440" y="110" width="120" height="434" rx="14" fill="#1a2232" stroke="${C.line}"/>
  <text x="500" y="138" text-anchor="middle" font-size="15" font-weight="700" fill="${C.text}">Tauri 2</text>
  <text x="500" y="158" text-anchor="middle" font-size="13" fill="${C.mute}">IPC</text>
  <g stroke="${C.soft}" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
    <path d="M412 230 H588"/><path d="M576 222 L588 230 L576 238"/>
    <path d="M588 330 H412"/><path d="M424 322 L412 330 L424 338"/>
    <path d="M412 430 H588"/><path d="M576 422 L588 430 L576 438"/>
  </g>
  <text x="500" y="214" text-anchor="middle" font-size="12" fill="${C.mute}">souris, messages</text>
  <text x="500" y="314" text-anchor="middle" font-size="12" fill="${C.mute}">hue-tick 30/s</text>
  <text x="500" y="356" text-anchor="middle" font-size="12" fill="${C.mute}">répliques, chat</text>
  <text x="500" y="414" text-anchor="middle" font-size="12" fill="${C.mute}">fichiers, voix</text>

  <text x="610" y="96" font-size="14" font-weight="700" fill="${C.orange}" letter-spacing="1.5">CŒUR · RUST</text>
  ${box(600, 110, 310, 108, "Physique · 60 Hz", ["Marche, lancers, rebonds, regard", "Caresses au survol, tournis"])}
  ${box(926, 110, 314, 108, "Battement · 250 ms", ["Détecte freezes et lags", "Sortie de veille"])}
  ${box(600, 230, 310, 108, "Cerveau · 1 Hz", ["Humeur, répliques, sieste", "Succès, fêtes, anniversaires"])}
  ${box(926, 230, 314, 108, "IA", ["LLM local (llama.cpp) ou API", "Outils + RAG sur tes documents"])}
  ${box(600, 350, 310, 108, "Capteurs", ["Matériel, stats en direct", "Événements Windows, mode discret"])}
  ${box(926, 350, 314, 108, "Dictée Whisper", ["whisper.cpp en local (base / small)", "ou API distante"])}
  ${box(600, 470, 640, 96, "Mémoire persistante", ["memory.json · documents.json · settings.json (dans %APPDATA%\\hue)", "Clé API rangée dans le Gestionnaire d'identification Windows"], "#9b7bff")}
  `;
  write("architecture.svg", doc(1280, 600, body));
}
