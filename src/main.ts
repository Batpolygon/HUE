import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  speakAnimalese,
  speakAnimaleseBlip,
  unlockAnimalese,
} from "./audio/animalese";
import {
  loadSettingsIntoUi,
  renderDna,
  saveSettingsFromUi,
  wireSettingsControls,
} from "./hud/settings";
import { setFaceContext } from "./pet/faceDisplay";
import {
  applyTick,
  chirpEyes,
  setSpeaking,
  type TickPayload,
} from "./pet/render";

const pet = () => document.querySelector("#pet") as HTMLElement;
const bubble = () => document.querySelector("#bubble") as HTMLElement;
const promptForm = () => document.querySelector("#prompt-form") as HTMLFormElement;
const promptInput = () => document.querySelector("#prompt-input") as HTMLInputElement;
const gear = () => document.querySelector("#settings-btn") as HTMLButtonElement;
const modal = () => document.querySelector("#settings-modal") as HTMLElement;
const stage = () => document.querySelector("#pet-stage") as HTMLElement;

let hudOpen = false;
let chatBuffer = "";
let speaking = false;
let winX = 0;
let winY = 0;

/** Queue so LLM tokens chirp as they arrive, without overlapping chaos. */
const streamSpeakQueue: string[] = [];
let streamSpeakBusy = false;

async function pumpStreamSpeech() {
  if (streamSpeakBusy) return;
  streamSpeakBusy = true;
  speaking = true;
  setSpeaking(pet(), true);
  const pulse = () => chirpEyes(pet());
  try {
    await unlockAnimalese();
    while (streamSpeakQueue.length > 0) {
      const chunk = streamSpeakQueue.shift()!;
      // Prefer whole words; otherwise chirp short fragments
      const parts = chunk.split(/(\s+)/).filter((p) => p.trim().length > 0);
      if (parts.length === 0) {
        const frag = chunk.replace(/\s+/g, "");
        if (frag) await speakAnimaleseBlip(frag, pulse);
      } else {
        for (const p of parts) {
          await speakAnimaleseBlip(p, pulse);
        }
      }
    }
  } finally {
    streamSpeakBusy = false;
    if (streamSpeakQueue.length > 0) {
      void pumpStreamSpeech();
    } else {
      speaking = false;
      setSpeaking(pet(), false);
    }
  }
}

function enqueueStreamSpeech(delta: string) {
  if (!delta) return;
  streamSpeakQueue.push(delta);
  void pumpStreamSpeech();
}

function tuckHud() {
  hudOpen = false;
  document.body.classList.remove("hud-open", "settings-open", "has-bubble");
  gear().classList.add("hidden");
  promptForm().classList.add("hidden");
  bubble().classList.add("hidden");
  stage().classList.remove("hud");
  modal().classList.add("hidden");
}

function showHud(open: boolean) {
  if (!open) {
    tuckHud();
    void invoke("set_hud_open", { open: false });
    void invoke("set_settings_open", { open: false });
    return;
  }
  hudOpen = true;
  document.body.classList.add("hud-open");
  gear().classList.remove("hidden");
  promptForm().classList.remove("hidden");
  stage().classList.add("hud");
  void invoke("set_hud_open", { open: true });
  setTimeout(() => promptInput().focus(), 50);
}

/** Interactive HUD bubble (requires HUD open) — stays inside the HUD window. */
function showBubble(text: string, speak = true) {
  if (!hudOpen) return;
  const el = bubble();
  el.textContent = text;
  el.classList.toggle("hidden", !text);
  document.body.classList.toggle("has-bubble", Boolean(text));
  if (speak && text) {
    void playSpeech(text);
  }
}

function sleep(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}

/**
 * Spontaneous speech: separate bubble window above the pet.
 * Pet window size/position is never changed.
 */
function waitBubbleTyped(timeoutMs: number): Promise<void> {
  return new Promise((resolve) => {
    let settled = false;
    let unlisten: (() => void) | null = null;
    const finish = () => {
      if (settled) return;
      settled = true;
      unlisten?.();
      resolve();
    };
    const t = window.setTimeout(finish, timeoutMs);
    void listen("hue-bubble-typed", () => {
      window.clearTimeout(t);
      finish();
    }).then((u) => {
      unlisten = u;
      if (settled) u();
    });
  });
}

async function showSpontaneousBubble(text: string) {
  speaking = true;
  setSpeaking(pet(), true);
  let stopChirps = false;
  const pulse = () => {
    if (!stopChirps) chirpEyes(pet());
  };
  try {
    await unlockAnimalese();
    const typed = waitBubbleTyped(Math.min(8000, 1200 + text.length * 40)).then(
      () => {
        stopChirps = true;
      },
    );
    await invoke("show_speech_bubble", { text });
    // Eyes + sound with each blip; stop chirps as soon as typewriter finishes
    await Promise.all([
      speakAnimalese(text, pulse, () => stopChirps).catch(() => undefined),
      typed,
    ]);
    // Leave the bubble up long enough to read after typing finishes
    await sleep(Math.min(9000, 2800 + text.length * 50));
  } finally {
    stopChirps = true;
    speaking = false;
    setSpeaking(pet(), false);
    await invoke("hide_speech_bubble");
  }
}

async function playSpeech(text: string) {
  speaking = true;
  setSpeaking(pet(), true);
  const pulse = () => chirpEyes(pet());
  try {
    await unlockAnimalese();
    await speakAnimalese(text, pulse);
  } finally {
    speaking = false;
    setSpeaking(pet(), false);
  }
}

function sendPointer(phase: "down" | "move" | "up", clientX: number, clientY: number) {
  void invoke("pointer_event", {
    input: {
      phase,
      screen_x: winX + clientX,
      screen_y: winY + clientY,
    },
  });
}

function wirePointer() {
  const el = pet();
  let down = false;
  let lastMove = 0;

  el.addEventListener("pointerenter", () => {
    void invoke("pet_hover");
  });

  el.addEventListener("pointerdown", (e) => {
    down = true;
    el.setPointerCapture(e.pointerId);
    void unlockAnimalese();
    sendPointer("down", e.clientX, e.clientY);
  });

  el.addEventListener("pointermove", (e) => {
    if (!down) return;
    const now = performance.now();
    if (now - lastMove < 16) return;
    lastMove = now;
    sendPointer("move", e.clientX, e.clientY);
  });

  const end = (e: PointerEvent) => {
    if (!down) return;
    down = false;
    sendPointer("up", e.clientX, e.clientY);
  };

  el.addEventListener("pointerup", end);
  el.addEventListener("pointercancel", end);
}

function wireHud() {
  promptForm().addEventListener("submit", async (e) => {
    e.preventDefault();
    const msg = promptInput().value.trim();
    if (!msg || speaking) return;
    promptInput().value = "";
    chatBuffer = "";
    streamSpeakQueue.length = 0;
    showBubble("…", false);
    try {
      await invoke("submit_prompt", { message: msg });
    } catch (err) {
      showBubble(String(err), false);
    }
  });

  gear().addEventListener("click", async (e) => {
    e.stopPropagation();
    await invoke("set_settings_open", { open: true });
    await loadSettingsIntoUi();
    await renderDna();
    modal().classList.remove("hidden");
    document.body.classList.add("settings-open");
  });

  const closeSettings = async () => {
    try {
      await saveSettingsFromUi();
    } catch (err) {
      console.error("auto-save settings failed", err);
    }
    modal().classList.add("hidden");
    document.body.classList.remove("settings-open");
    await invoke("set_settings_open", { open: false });
  };

  document.querySelector("#settings-close")?.addEventListener("click", () => {
    void closeSettings();
  });

  // Click on backdrop (modal overlay, not the card) also auto-saves & closes
  modal().addEventListener("click", (e) => {
    if (e.target === modal()) {
      void closeSettings();
    }
  });

  document.querySelector("#stt-btn")?.addEventListener("click", async () => {
    const st = await invoke<{ available: boolean; message: string }>("stt_status");
    showBubble(st.message, false);
  });

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      void closeSettings();
      showHud(false);
    }
  });

  wireSettingsControls();
}

async function boot() {
  wirePointer();
  wireHud();
  await loadSettingsIntoUi();

  await listen<{ open: boolean }>("hue-hud", (ev) => {
    if (ev.payload.open) {
      showHud(true);
      const welcome = (window as unknown as { __hueWelcome?: string }).__hueWelcome;
      if (welcome) {
        showBubble(welcome, true);
        delete (window as unknown as { __hueWelcome?: string }).__hueWelcome;
      }
    } else {
      tuckHud();
    }
  });

  await listen<TickPayload>("hue-tick", (ev) => {
    winX = ev.payload.x;
    winY = ev.payload.y;
    applyTick(pet(), ev.payload);
    setFaceContext(ev.payload.peeking, speaking || ev.payload.speaking);
    if (ev.payload.hud_should_close && hudOpen) {
      tuckHud();
    }
    if (ev.payload.state === "interacting" && !hudOpen && !ev.payload.peeking) {
      showHud(true);
      const welcome = (window as unknown as { __hueWelcome?: string }).__hueWelcome;
      if (welcome) {
        showBubble(welcome, true);
        delete (window as unknown as { __hueWelcome?: string }).__hueWelcome;
      }
    }
  });

  await listen<{ kind: string; message: string }>("hue-system", (ev) => {
    if (hudOpen) {
      showBubble(ev.payload.message);
    } else {
      void showSpontaneousBubble(ev.payload.message);
    }
  });

  await listen<{ message: string; peeking: boolean }>("hue-chatter", (ev) => {
    void showSpontaneousBubble(ev.payload.message);
  });

  await listen<{ delta?: string; done?: boolean }>("hue-chat", (ev) => {
    if (ev.payload.delta) {
      chatBuffer += ev.payload.delta;
      const el = bubble();
      el.textContent = chatBuffer;
      el.classList.remove("hidden");
      document.body.classList.add("has-bubble");
      // Chirp in sync with tokens as they land in the bubble
      enqueueStreamSpeech(ev.payload.delta);
      // Keep latest text visible while streaming
      el.scrollTop = el.scrollHeight;
    }
    if (ev.payload.done) {
      // Speech already streamed with deltas — just ensure queue drains
      void pumpStreamSpeech();
    }
  });

  await listen<{
    display_name: string;
    hostname: string;
    user_name?: string;
  }>("hue-identity", (ev) => {
    const petName = ev.payload.display_name || ev.payload.hostname;
    const user = ev.payload.user_name?.trim();
    (window as unknown as { __hueWelcome?: string }).__hueWelcome = user
      ? `Salut ${user} ! C'est moi, ${petName}. Prêt pour une nouvelle session !`
      : `Salut ! C'est moi, ${petName}. Prêt pour une nouvelle session !`;
  });

  await listen<{ message: string }>("hue-status", (ev) => {
    const msg = ev.payload.message;
    if (!msg) return;
    if (hudOpen) {
      showBubble(msg, false);
    } else {
      void showSpontaneousBubble(msg);
    }
  });
}

window.addEventListener("DOMContentLoaded", () => {
  void boot();
});
