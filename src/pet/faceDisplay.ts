import { invoke } from "@tauri-apps/api/core";

import type { LiveStats, PeekDisplayMode } from "../types";

export type { PeekDisplayMode } from "../types";

type Frame =
  | { kind: "eyes" }
  | { kind: "clock" }
  | { kind: "cpu" }
  | { kind: "ram" }
  | { kind: "disk" }
  | { kind: "gpu" }
  | { kind: "batt" }
  | { kind: "temp" }
  | { kind: "mood" };

const FRAME_MS = 3200;

let mode: PeekDisplayMode = "all";
let peeking = false;
let speaking = false;
let frameIdx = 0;
let timer: number | null = null;
let statsCache: LiveStats | null = null;
let lastStatsAt = 0;

/** Fresh stats pushed by the backend (avoids polling). */
export function feedStats(s: LiveStats) {
  statsCache = s;
  lastStatsAt = Date.now();
}

export function setPeekDisplayMode(m: PeekDisplayMode) {
  mode = m;
  frameIdx = 0;
  render();
}

export function setFaceContext(nextPeeking: boolean, nextSpeaking: boolean) {
  const changed = peeking !== nextPeeking || speaking !== nextSpeaking;
  peeking = nextPeeking;
  speaking = nextSpeaking;
  if (peeking && !speaking) {
    ensureLoop();
    if (changed) render();
  } else {
    stopLoop();
    showEyesOnly();
  }
}

function framesForMode(): Frame[] {
  switch (mode) {
    case "eyes_only":
      return [{ kind: "eyes" }];
    case "clock_only":
      return [{ kind: "clock" }];
    case "spec_only":
      return [
        { kind: "cpu" },
        { kind: "ram" },
        { kind: "disk" },
        { kind: "gpu" },
        { kind: "temp" },
        { kind: "batt" },
      ];
    case "all":
    default:
      return [
        { kind: "eyes" },
        { kind: "clock" },
        { kind: "mood" },
        { kind: "cpu" },
        { kind: "ram" },
        { kind: "disk" },
        { kind: "gpu" },
        { kind: "temp" },
        { kind: "batt" },
      ];
  }
}

function ensureLoop() {
  if (timer != null) return;
  timer = window.setInterval(() => {
    frameIdx += 1;
    void render();
  }, FRAME_MS);
  void render();
}

function stopLoop() {
  if (timer != null) {
    clearInterval(timer);
    timer = null;
  }
}

function showEyesOnly() {
  const pet = document.querySelector("#pet");
  const face = document.querySelector("#face-display") as HTMLElement | null;
  pet?.classList.remove("face-overlay");
  if (face) {
    face.classList.add("hidden");
    face.textContent = "";
  }
}

async function refreshStats(): Promise<LiveStats | null> {
  const now = Date.now();
  if (statsCache && now - lastStatsAt < 2000) return statsCache;
  try {
    statsCache = await invoke<LiveStats>("get_live_stats");
    lastStatsAt = now;
    return statsCache;
  } catch {
    return statsCache;
  }
}

async function render() {
  if (!peeking || speaking) {
    showEyesOnly();
    return;
  }

  const frames = framesForMode();
  const frame = frames[frameIdx % frames.length] ?? { kind: "eyes" as const };
  const pet = document.querySelector("#pet");
  const face = document.querySelector("#face-display") as HTMLElement | null;
  if (!face || !pet) return;

  if (frame.kind === "eyes") {
    showEyesOnly();
    return;
  }

  pet.classList.add("face-overlay");
  face.classList.remove("hidden");

  face.style.opacity = "0";
  requestAnimationFrame(() => {
    face.style.transition = "opacity 0.25s ease";
    face.style.opacity = "1";
  });

  if (frame.kind === "clock") {
    const s = await refreshStats();
    if (!s) {
      const d = new Date();
      face.innerHTML = `<span class="face-clock">${pad(d.getHours())}:${pad(d.getMinutes())}</span>`;
    } else {
      face.innerHTML = `<span class="face-clock">${pad(s.hour)}:${pad(s.minute)}</span>`;
    }
    return;
  }

  if (frame.kind === "mood") {
    const s = await refreshStats();
    const hour = s?.hour ?? new Date().getHours();
    face.innerHTML = `<span class="face-clock face-glyph">${pick(MOODS[moodOf(s, hour)])}</span>`;
    return;
  }

  const s = await refreshStats();
  if (!s) {
    face.textContent = "…";
    return;
  }

  switch (frame.kind) {
    case "cpu":
      face.innerHTML = `<span class="face-label">CPU</span><span class="face-value">${fmt(s.cpu_pct)}%</span>`;
      break;
    case "ram":
      face.innerHTML = `<span class="face-label">RAM</span><span class="face-value">${fmt(s.ram_pct)}%</span>`;
      break;
    case "disk":
      face.innerHTML = `<span class="face-label">DISK</span><span class="face-value">${fmt(s.disk_pct)}%</span>`;
      break;
    case "gpu":
      face.innerHTML = `<span class="face-label">GPU</span><span class="face-value">${fmt(s.gpu_pct)}%</span>`;
      break;
    case "temp": {
      const t = Math.max(s.cpu_temp ?? 0, s.gpu_temp ?? 0);
      if (!t) {
        frameIdx += 1;
        void render();
        return;
      }
      face.innerHTML = `<span class="face-label">TEMP</span><span class="face-value">${Math.round(t)}°</span>`;
      break;
    }
    case "batt":
      if (s.battery_pct == null) {
        frameIdx += 1;
        void render();
        return;
      }
      {
        const plug = s.on_ac === true ? "⚡" : "🔋";
        face.innerHTML = `<span class="face-label">${plug}</span><span class="face-value">${Math.round(s.battery_pct)}%</span>`;
      }
      break;
  }
}

/** Kaomoji shown on the tucked-away screen, by mood — a fresh one each time the frame comes up. */
const MOODS = {
  night: ["(-_-) zz", "(˘ω˘) zz", "(=_=) zz", "(ᴗ_ᴗ) zz"],
  morning: ["(•ᴗ•)", "(^ᴗ^)", "\\(^o^)/", "(•ω•)"],
  day: ["(•ᴗ•)", "(^-^)", "(•ω•)", "(^ω^)"],
  evening: ["(˘ᴗ˘)", "(-ᴗ-)", "(•ᴗ•)~", "(ᴗ_ᴗ)"],
  chill: ["(^ᴗ^)", "(•ᴗ•)♪", "(˘ω˘)", "(^ω^)♪"],
  busy: ["(>_<)", "(@_@)", "(ò_ó)", "(°ロ°)"],
  full: ["(°□°)", "(⊙_⊙)", "(o_O)"],
  lowbatt: ["(=_=)", "(×_×)", "(-_-)…"],
  gaming: ["(★ᴗ★)", "(★_★)", "(òᴗó)"],
} as const;

type Mood = keyof typeof MOODS;

function moodOf(s: LiveStats | null, hour: number): Mood {
  if (s) {
    if (s.cpu_pct >= 85 || s.ram_pct >= 90) return "busy";
    if (s.disk_pct >= 92) return "full";
    if (s.battery_pct != null && s.battery_pct < 20 && s.on_ac !== true) return "lowbatt";
    if (s.gpu_pct >= 80) return "gaming";
  }
  if (hour >= 22 || hour < 6) return "night";
  if (s && s.cpu_pct < 25 && s.ram_pct < 50 && hour >= 11) return "chill";
  if (hour < 11) return "morning";
  return hour < 18 ? "day" : "evening";
}

function pick<T>(list: readonly T[]): T {
  return list[Math.floor(Math.random() * list.length)];
}

function pad(n: number) {
  return String(n).padStart(2, "0");
}

function fmt(n: number) {
  return n >= 10 ? String(Math.round(n)) : n.toFixed(1);
}

