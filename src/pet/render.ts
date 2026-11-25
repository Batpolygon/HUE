import type { Face, ShellSpec, TickPayload } from "../types";
import { setSleeping } from "./particles";
import { buildGhostSvg, buildShellSvg, setDecor } from "./shell";

export type { TickPayload } from "../types";

const STATE_CLASSES = [
  "state-idle",
  "state-walking",
  "state-dragging",
  "state-tossed",
  "state-interacting",
  "state-peeking",
  "state-heavy",
  "state-tired",
  "state-panic",
  "state-joy",
  "state-dizzy",
];
const MICRO_CLASSES = ["micro-yawn", "micro-wiggle", "micro-look", "micro-stretch", "micro-dance", "micro-hop"];

let currentDecor: string | null = null;
let lastMicro: string | null = null;
/** Face forced by the UI (listening, file hover) — wins over the backend's. */
let uiFace: Face | null = null;

let materializeTimer = 0;

/** Swap in the real shell. `materialize` plays the ghost → solid transition. */
export function mountShell(pet: HTMLElement, spec: ShellSpec | null, materialize = false) {
  const face = pet.querySelector("#face-display");
  pet.querySelector("svg")?.remove();
  pet.insertAdjacentHTML("afterbegin", buildShellSvg(spec));
  if (face) pet.appendChild(face);
  pet.dataset.form = spec?.form ?? "desktop";
  pet.classList.remove("ghost");
  setDecor(pet.querySelector("svg"), currentDecor);
  if (materialize) {
    pet.classList.remove("materialize");
    void pet.offsetWidth;
    pet.classList.add("materialize");
    window.clearTimeout(materializeTimer);
    materializeTimer = window.setTimeout(() => pet.classList.remove("materialize"), 1000);
  }
}

/** Loading hologram shown until the identity (and so the real shell) is known. */
export function mountGhost(pet: HTMLElement) {
  const face = pet.querySelector("#face-display");
  pet.querySelector("svg")?.remove();
  pet.insertAdjacentHTML("afterbegin", buildGhostSvg());
  if (face) pet.appendChild(face);
  pet.classList.add("ghost");
}

export function isGhost(pet: HTMLElement) {
  return pet.classList.contains("ghost");
}

export function applyDecor(pet: HTMLElement, decor: string | null) {
  currentDecor = decor;
  setDecor(pet.querySelector("svg"), decor);
}

export function setUiFace(face: Face | null) {
  uiFace = face;
}

function faceFor(tick: TickPayload, dizzy: boolean): string {
  if (uiFace) return uiFace;
  if (tick.sleeping) return "sleep";
  if (dizzy) return "";
  if (tick.expression) return tick.expression;
  if (tick.micro === "yawn") return "sleepy";
  switch (tick.state) {
    case "joy":
      return "happy";
    case "heavy":
      return "strain";
    case "panic":
      return "wide";
    case "tired":
      return "sleepy";
    default:
      break;
  }
  // Sustained moods leak into the eyes.
  const v = tick.valence ?? 0.3;
  const e = tick.energy ?? 0.6;
  if (v < -0.45) return "sad";
  if (e < 0.18) return "sleepy";
  return "";
}

// ── Squash & stretch spring ──
// The backend sends a squash that snaps on impact and decays at 30 Hz. Rendering it raw looks
// stiff and steppy, so it becomes the *target* of an underdamped spring integrated every frame:
// the body overshoots a little and settles like jelly.
const SPRING_K = 520;
const SPRING_C = 13;
const spring = { x: 1, y: 1, vx: 0, vy: 0, tx: 1, ty: 1 };
let springEl: HTMLElement | null = null;
let springRaf = 0;
let springLast = 0;

function writeSquash() {
  if (springEl) springEl.style.transform = `scale(${spring.x.toFixed(4)}, ${spring.y.toFixed(4)})`;
}

function springFrame(now: number) {
  const dt = Math.min(0.05, Math.max(0.001, (now - springLast) / 1000));
  springLast = now;
  spring.vx += (SPRING_K * (spring.tx - spring.x) - SPRING_C * spring.vx) * dt;
  spring.vy += (SPRING_K * (spring.ty - spring.y) - SPRING_C * spring.vy) * dt;
  spring.x += spring.vx * dt;
  spring.y += spring.vy * dt;
  const settled =
    Math.abs(spring.tx - spring.x) < 0.0008 &&
    Math.abs(spring.ty - spring.y) < 0.0008 &&
    Math.abs(spring.vx) < 0.01 &&
    Math.abs(spring.vy) < 0.01;
  if (settled) {
    spring.x = spring.tx;
    spring.y = spring.ty;
    spring.vx = spring.vy = 0;
    writeSquash();
    springRaf = 0;
    return;
  }
  writeSquash();
  springRaf = requestAnimationFrame(springFrame);
}

function setSquashTarget(pet: HTMLElement, tx: number, ty: number) {
  springEl = pet;
  spring.tx = tx;
  spring.ty = ty;
  if (!springRaf) {
    springLast = performance.now();
    springRaf = requestAnimationFrame(springFrame);
  }
}

export function applyTick(pet: HTMLElement, tick: TickPayload) {
  const dizzy = Boolean(tick.dizzy) || tick.state === "dizzy";
  setSquashTarget(pet, tick.squash_x, tick.squash_y);
  for (const c of STATE_CLASSES) pet.classList.remove(c);
  pet.classList.add(`state-${tick.state}`);
  pet.classList.toggle("dizzy", dizzy);
  pet.classList.toggle("sleeping", Boolean(tick.sleeping));
  pet.classList.toggle("facing-user", tick.facing_user && !dizzy);
  document.body.classList.toggle("peeking", tick.peeking);
  document.body.classList.toggle("dragging", tick.state === "dragging");

  const face = faceFor(tick, dizzy);
  if (pet.dataset.face !== face) pet.dataset.face = face;
  pet.classList.toggle("has-face", face !== "");

  const micro = tick.micro ?? null;
  if (micro !== lastMicro) {
    for (const c of MICRO_CLASSES) pet.classList.remove(c);
    if (micro) pet.classList.add(`micro-${micro}`);
    lastMicro = micro;
  }

  // Mood → eye openness and blush.
  const v = tick.valence ?? 0.3;
  const e = tick.energy ?? 0.6;
  const open = Math.max(0.62, Math.min(1.08, 0.7 + e * 0.38));
  pet.style.setProperty("--eye-open", open.toFixed(3));
  const blush =
    face === "love" ? 0.75 : face === "happy" ? 0.45 : v > 0.6 ? (v - 0.6) * 1.2 : 0;
  const blushEl = pet.querySelector("#blush");
  if (blushEl) blushEl.setAttribute("opacity", blush.toFixed(2));

  setSleeping(Boolean(tick.sleeping) && !tick.peeking);

  // Only the eyes move — the frame never flips. They follow the cursor, and while strolling
  // (not looking at you) they glance toward where he's heading.
  const eyes = pet.querySelector("#eyes") as SVGGElement | null;
  if (eyes) {
    const strolling = tick.state === "walking" && !tick.facing_user && !dizzy;
    const lead = strolling ? (tick.walk_dir ?? 1) * 0.55 : 0;
    const gx = tick.sleeping ? 0 : Math.max(-1, Math.min(1, (tick.gaze_x ?? 0) + lead)) * 3.4;
    const gy = tick.sleeping ? 1 : (tick.gaze_y ?? 0) * 2.6;
    eyes.style.transform = `translate(${gx.toFixed(2)}px, ${gy.toFixed(2)}px)`;
  }

  const fx = pet.querySelector("#dizzy-fx") as SVGGElement | null;
  if (fx) fx.setAttribute("opacity", dizzy ? "1" : "0");
}

/** Logical "busy speaking" (pauses peek face cycle) — does NOT animate eyes. */
export function setSpeaking(pet: HTMLElement, speaking: boolean) {
  pet.classList.toggle("speaking", speaking);
  if (!speaking) pet.classList.remove("eye-chirp");
}

/** One eye pulse synced to an animalese blip. */
export function chirpEyes(pet: HTMLElement) {
  pet.classList.remove("eye-chirp");
  void pet.offsetWidth;
  pet.classList.add("eye-chirp");
  window.setTimeout(() => pet.classList.remove("eye-chirp"), 120);
}
