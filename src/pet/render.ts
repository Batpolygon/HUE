export type HueState =
  | "idle"
  | "walking"
  | "dragging"
  | "tossed"
  | "interacting"
  | "peeking"
  | "heavy"
  | "tired"
  | "panic"
  | "joy"
  | "dizzy";

export interface TickPayload {
  x: number;
  y: number;
  vx: number;
  vy: number;
  squash_x: number;
  squash_y: number;
  state: HueState;
  facing_user: boolean;
  peeking: boolean;
  speaking: boolean;
  hud_should_close: boolean;
  gaze_x?: number;
  gaze_y?: number;
  dizzy?: boolean;
  walk_dir?: number;
}

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

export function applyTick(pet: HTMLElement, tick: TickPayload) {
  const dizzy = Boolean(tick.dizzy) || tick.state === "dizzy";
  const facingLeft = (tick.walk_dir ?? 1) < 0 && !tick.facing_user && !dizzy;
  pet.style.transform = `scale(${tick.squash_x.toFixed(3)}, ${tick.squash_y.toFixed(3)})`;
  for (const c of STATE_CLASSES) pet.classList.remove(c);
  pet.classList.add(`state-${tick.state}`);
  pet.classList.toggle("dizzy", dizzy);
  pet.classList.toggle("facing-user", tick.facing_user && !dizzy);
  pet.classList.toggle("facing-left", facingLeft);
  document.body.classList.toggle("peeking", tick.peeking);
  document.body.classList.toggle("dragging", tick.state === "dragging");

  // Soft pupil/eye group follow toward the cursor (spinning while dizzy).
  // Facing-left mirrors the SVG — invert X so gaze still tracks the real cursor.
  const eyes = pet.querySelector("#eyes") as SVGGElement | null;
  if (eyes) {
    const mirrored = facingLeft ? -1 : 1;
    const gx = (tick.gaze_x ?? 0) * 3.4 * mirrored;
    const gy = (tick.gaze_y ?? 0) * 2.6;
    eyes.setAttribute("transform", `translate(${gx.toFixed(2)} ${gy.toFixed(2)})`);
  }

  const fx = pet.querySelector("#dizzy-fx") as SVGGElement | null;
  if (fx) {
    fx.setAttribute("opacity", dizzy ? "1" : "0");
  }
}

/** Logical "busy speaking" (pauses peek face cycle) — does NOT animate eyes. */
export function setSpeaking(pet: HTMLElement, speaking: boolean) {
  pet.classList.toggle("speaking", speaking);
  if (!speaking) {
    pet.classList.remove("eye-chirp");
  }
}

/** One eye blink/pulse synced to an animalese blip. */
export function chirpEyes(pet: HTMLElement) {
  pet.classList.remove("eye-chirp");
  void pet.offsetWidth;
  pet.classList.add("eye-chirp");
  window.setTimeout(() => pet.classList.remove("eye-chirp"), 120);
}
