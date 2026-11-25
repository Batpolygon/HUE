/**
 * Lightweight DOM particles around the pet: bursts (hearts, confetti, sweat, steam,
 * zzz, ring, notes) and ambient seasonal weather (snow, leaves, hearts, confetti, notes).
 */

let enabled = true;
let ambientKind: string | null = null;
let ambientTimer: number | null = null;
let zzzTimer: number | null = null;
let steamTimer: number | null = null;
let sweatTimer: number | null = null;
const MAX_LIVE = 40;

const layer = () => document.querySelector("#fx-layer") as HTMLElement | null;
const petBox = () => document.querySelector("#pet") as HTMLElement | null;

export function setParticlesEnabled(on: boolean) {
  enabled = on;
  if (!on) {
    layer()?.replaceChildren();
    setAmbient(null);
  }
}

function rand(a: number, b: number) {
  return a + Math.random() * (b - a);
}

/** Pet rect relative to the fx layer. */
function anchor() {
  const pet = petBox();
  const lay = layer();
  if (!pet || !lay) return null;
  const p = pet.getBoundingClientRect();
  const l = lay.getBoundingClientRect();
  return { x: p.left - l.left, y: p.top - l.top, w: p.width, h: p.height };
}

function spawn(
  cls: string,
  text: string,
  x: number,
  y: number,
  vars: Record<string, string | number>,
  lifeMs: number,
) {
  const lay = layer();
  if (!lay || !enabled || lay.childElementCount > MAX_LIVE) return;
  const el = document.createElement("span");
  el.className = `particle ${cls}`;
  el.textContent = text;
  el.style.left = `${x.toFixed(1)}px`;
  el.style.top = `${y.toFixed(1)}px`;
  for (const [k, v] of Object.entries(vars)) el.style.setProperty(k, String(v));
  el.style.animationDuration = `${lifeMs}ms`;
  lay.appendChild(el);
  window.setTimeout(() => el.remove(), lifeMs + 50);
}

const CONFETTI = ["#ff8a1f", "#4fc3f7", "#ffd54f", "#81c784", "#f06292", "#ba68c8"];

export function burst(kind: string) {
  if (!enabled) return;
  const a = anchor();
  if (!a) return;
  const cx = a.x + a.w / 2;
  const top = a.y + a.h * 0.18;
  switch (kind) {
    case "hearts":
      for (let i = 0; i < 5; i++) {
        spawn("p-heart", "♥", cx + rand(-30, 30), top + rand(0, 20), { "--dx": `${rand(-16, 16)}px`, "--dy": `${rand(-50, -34)}px`, "--s": rand(0.8, 1.4).toFixed(2) }, 1400 + i * 90);
      }
      break;
    case "confetti":
      for (let i = 0; i < 22; i++) {
        const c = CONFETTI[i % CONFETTI.length];
        spawn("p-confetti", "", cx + rand(-10, 10), top + 10, { "--dx": `${rand(-70, 70)}px`, "--dy": `${rand(-60, 40)}px`, "--rot": `${rand(-540, 540)}deg`, "--c": c }, 1500 + rand(0, 500));
      }
      break;
    case "sweat":
      for (let i = 0; i < 2; i++) {
        const side = i === 0 ? -1 : 1;
        spawn("p-sweat", "", cx + side * a.w * 0.36, top + rand(2, 14), { "--dx": `${side * rand(4, 10)}px`, "--dy": `${rand(18, 30)}px` }, 1100);
      }
      break;
    case "steam":
      for (let i = 0; i < 3; i++) {
        spawn("p-steam", "", cx + rand(-26, 26), a.y + a.h * 0.08, { "--dx": `${rand(-10, 10)}px`, "--dy": `${rand(-36, -24)}px`, "--s": rand(0.8, 1.5).toFixed(2) }, 1800 + i * 200);
      }
      break;
    case "zzz":
      spawn("p-zzz", "z", cx + a.w * 0.25, top + 6, { "--dx": `${rand(12, 22)}px`, "--dy": `${rand(-34, -26)}px`, "--s": rand(0.8, 1.2).toFixed(2) }, 2200);
      break;
    case "ring":
      for (let i = 0; i < 3; i++) {
        window.setTimeout(() => spawn("p-ring", "", cx, a.y + a.h * 0.42, {}, 900), i * 260);
      }
      break;
    case "notes":
      for (let i = 0; i < 3; i++) {
        spawn("p-note", i % 2 ? "♪" : "♫", cx + rand(-28, 28), top + rand(0, 20), { "--dx": `${rand(-14, 14)}px`, "--dy": `${rand(-46, -30)}px`, "--s": "1" }, 1600);
      }
      break;
    default:
      break;
  }
}

function ambientTick() {
  const lay = layer();
  if (!lay || !ambientKind || !enabled) return;
  const w = lay.clientWidth;
  switch (ambientKind) {
    case "snow":
      spawn("p-snow", "❄", rand(0, w), -6, { "--dx": `${rand(-14, 14)}px`, "--dy": `${lay.clientHeight + 10}px`, "--s": rand(0.5, 1).toFixed(2) }, 5200);
      break;
    case "leaves":
      spawn("p-leaf", "", rand(0, w), -6, { "--dx": `${rand(-30, 30)}px`, "--dy": `${lay.clientHeight + 10}px`, "--rot": `${rand(-360, 360)}deg`, "--c": ["#e57c23", "#c0392b", "#d4a017"][Math.floor(rand(0, 3))] }, 5600);
      break;
    case "hearts":
      spawn("p-heart", "♥", rand(10, w - 10), lay.clientHeight - 10, { "--dx": `${rand(-10, 10)}px`, "--dy": `${-lay.clientHeight * 0.6}px`, "--s": rand(0.5, 0.9).toFixed(2) }, 3600);
      break;
    case "confetti":
      spawn("p-confetti", "", rand(0, w), -6, { "--dx": `${rand(-20, 20)}px`, "--dy": `${lay.clientHeight + 10}px`, "--rot": `${rand(-540, 540)}deg`, "--c": CONFETTI[Math.floor(rand(0, CONFETTI.length))] }, 4200);
      break;
    case "notes":
      burst("notes");
      break;
  }
}

/** Seasonal weather (null = none). */
export function setAmbient(kind: string | null) {
  ambientKind = enabled ? kind : null;
  if (ambientTimer != null) {
    clearInterval(ambientTimer);
    ambientTimer = null;
  }
  if (ambientKind) {
    const every = ambientKind === "notes" ? 7000 : ambientKind === "hearts" ? 2600 : 1100;
    ambientTimer = window.setInterval(ambientTick, every);
  }
}

/** Continuous emitters driven by state (sleep, heat, effort). */
export function setSleeping(on: boolean) {
  if (on && zzzTimer == null) {
    burst("zzz");
    zzzTimer = window.setInterval(() => burst("zzz"), 1500);
  } else if (!on && zzzTimer != null) {
    clearInterval(zzzTimer);
    zzzTimer = null;
  }
}

export function setHot(on: boolean) {
  if (on && steamTimer == null) {
    burst("steam");
    steamTimer = window.setInterval(() => burst("steam"), 2600);
  } else if (!on && steamTimer != null) {
    clearInterval(steamTimer);
    steamTimer = null;
  }
}

export function setStraining(on: boolean) {
  if (on && sweatTimer == null) {
    burst("sweat");
    sweatTimer = window.setInterval(() => burst("sweat"), 3200);
  } else if (!on && sweatTimer != null) {
    clearInterval(sweatTimer);
    sweatTimer = null;
  }
}
