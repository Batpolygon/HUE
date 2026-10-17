import { invoke } from "@tauri-apps/api/core";

export type PeekDisplayMode = "all" | "spec_only" | "clock_only" | "eyes_only";

export interface LiveStats {
  cpu_pct: number;
  ram_pct: number;
  ram_used_gb: number;
  ram_total_gb: number;
  cpu_freq_mhz: number;
  disk_pct: number;
  battery_pct: number | null;
  on_ac: boolean | null;
  gpu_pct: number;
  hour: number;
  minute: number;
  second: number;
}

type Frame =
  | { kind: "eyes" }
  | { kind: "clock" }
  | { kind: "cpu" }
  | { kind: "ram" }
  | { kind: "disk" }
  | { kind: "gpu" }
  | { kind: "batt" }
  | { kind: "mood" };

const FRAME_MS = 3200;

let mode: PeekDisplayMode = "all";
let peeking = false;
let speaking = false;
let frameIdx = 0;
let timer: number | null = null;
let statsCache: LiveStats | null = null;
let lastStatsAt = 0;

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
    let glyph = hour >= 22 || hour < 6 ? "zzz" : hour < 11 ? ":)" : hour < 18 ? "•ᴗ•" : "-_-";
    if (s) {
      if (s.cpu_pct >= 85 || s.ram_pct >= 90) glyph = ">_<";
      else if (s.disk_pct >= 92) glyph = ";;";
      else if (s.battery_pct != null && s.battery_pct < 20 && s.on_ac !== true) glyph = "zZ";
      else if (s.gpu_pct >= 80) glyph = "★_★";
      else if (s.cpu_pct < 25 && s.ram_pct < 50) glyph = hour < 11 ? ":D" : "•ᴗ•";
    }
    face.innerHTML = `<span class="face-clock" style="font-size:13px">${glyph}</span>`;
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

function pad(n: number) {
  return String(n).padStart(2, "0");
}

function fmt(n: number) {
  return n >= 10 ? String(Math.round(n)) : n.toFixed(1);
}

