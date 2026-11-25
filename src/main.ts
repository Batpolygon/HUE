import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  setVoice,
  setVoiceMood,
  speakAnimalese,
  speakAnimaleseBlip,
  unlockAnimalese,
} from "./audio/animalese";
import {
  loadSettingsIntoUi,
  refreshOpenTab,
  saveSettingsFromUi,
  setCachedIdentity,
  showTab,
  wireSettingsControls,
} from "./hud/settings";
import { feedStats, setFaceContext } from "./pet/faceDisplay";
import { burst, setAmbient, setHot, setStraining } from "./pet/particles";
import {
  applyDecor,
  applyTick,
  chirpEyes,
  isGhost,
  mountGhost,
  mountShell,
  setSpeaking,
  setUiFace,
} from "./pet/render";
import type { IdentityCard, SttStatus, TickPayload, TodayInfo, Vitals } from "./types";

const $ = <T extends HTMLElement>(sel: string) => document.querySelector(sel) as T;
const pet = () => $<HTMLElement>("#pet");
const bubble = () => $<HTMLElement>("#bubble");
const hud = () => $<HTMLElement>("#hud");
const promptInput = () => $<HTMLInputElement>("#prompt-input");
const gear = () => $<HTMLButtonElement>("#settings-btn");
const modal = () => $<HTMLElement>("#settings-modal");
const stage = () => $<HTMLElement>("#pet-stage");
const statusEl = () => $<HTMLElement>("#chat-status");
const sttBtn = () => $<HTMLButtonElement>("#stt-btn");

let hudOpen = false;
let settingsOpen = false;
let chatBusy = false;
let chatBuffer = "";
let speaking = false;
let winX = 0;
let winY = 0;
let recording = false;
let transcribing = false;
let pendingConfirm: number | null = null;
let sleepingNow = false;

function sleep(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}

function esc(s: string) {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

/** Tiny, safe markdown: **bold**, bullets, line breaks. */
function renderMarkdown(text: string) {
  return esc(text)
    .replace(/\*\*(.+?)\*\*/g, "<b>$1</b>")
    .split(/\n/)
    .map((line) => {
      const m = /^\s*(?:[-*•]|\d+[.)])\s+(.*)$/.exec(line);
      return m ? `<span class="li">${m[1]}</span>` : line;
    })
    .join("<br>")
    .replace(/(<br>){3,}/g, "<br><br>");
}

// ───────────────────────────── chat speech (HUD) ─────────────────────────────

/** LLM tokens chirp as they arrive, without overlapping chaos. */
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
      const parts = chunk.split(/(\s+)/).filter((p) => p.trim().length > 0);
      if (parts.length === 0) {
        const frag = chunk.replace(/\s+/g, "");
        if (frag) await speakAnimaleseBlip(frag, pulse);
      } else {
        for (const p of parts) await speakAnimaleseBlip(p, pulse);
      }
    }
  } finally {
    streamSpeakBusy = false;
    if (streamSpeakQueue.length > 0) {
      void pumpStreamSpeech();
    } else {
      speaking = false;
      setSpeaking(pet(), false);
      void pumpLines();
    }
  }
}

function enqueueStreamSpeech(delta: string) {
  if (!delta) return;
  // Long replies: don't let the voice lag minutes behind the text.
  if (streamSpeakQueue.length > 40) streamSpeakQueue.splice(0, streamSpeakQueue.length - 20);
  streamSpeakQueue.push(delta);
  void pumpStreamSpeech();
}

async function playSpeech(text: string) {
  speaking = true;
  setSpeaking(pet(), true);
  try {
    await unlockAnimalese();
    await speakAnimalese(text, () => chirpEyes(pet()));
  } finally {
    speaking = false;
    setSpeaking(pet(), false);
  }
}

// ───────────────────────────── HUD ─────────────────────────────

function setStatus(text: string) {
  const el = statusEl();
  el.textContent = text;
  el.classList.toggle("hidden", !text);
}

function tuckHud() {
  hudOpen = false;
  settingsOpen = false;
  document.body.classList.remove("hud-open", "settings-open", "has-bubble");
  gear().classList.add("hidden");
  hud().classList.add("hidden");
  stage().classList.remove("hud");
  modal().classList.add("hidden");
  if (recording) void cancelMic();
}

function showHud(open: boolean) {
  if (!open) {
    tuckHud();
    void invoke("set_hud_open", { open: false });
    void invoke("set_settings_open", { open: false });
    return;
  }
  if (!hudOpen) {
    hudOpen = true;
    document.body.classList.add("hud-open");
    gear().classList.remove("hidden");
    hud().classList.remove("hidden");
    stage().classList.add("hud");
  }
  void invoke("set_hud_open", { open: true });
  setTimeout(() => promptInput().focus(), 60);
}

function showBubble(html: string, speak = false, plain?: string) {
  const el = bubble();
  el.innerHTML = html;
  el.classList.toggle("hidden", !html);
  document.body.classList.toggle("has-bubble", Boolean(html));
  if (speak && plain) void playSpeech(plain);
}

function showError(err: unknown) {
  showBubble(`<span class="err">${esc(String(err))}</span>`);
}

async function runChat(task: () => Promise<unknown>) {
  if (chatBusy) return;
  chatBusy = true;
  chatBuffer = "";
  streamSpeakQueue.length = 0;
  setStatus("");
  showBubble(`<span class="typing">…</span>`);
  try {
    await task();
  } catch (err) {
    showError(err);
  } finally {
    chatBusy = false;
    hideConfirm();
    window.setTimeout(() => {
      if (!chatBusy) setStatus("");
    }, 2500);
    void pumpLines();
  }
}

function sendPrompt(message: string) {
  void runChat(() => invoke("submit_prompt", { message }));
}

function analyzeFile(path: string) {
  showHud(true);
  void runChat(() => invoke("analyze_file", { path }));
}

// ───────────────────────────── confirmations (tools) ─────────────────────────────

function hideConfirm() {
  pendingConfirm = null;
  $("#confirm-bar").classList.add("hidden");
}

async function answerConfirm(yes: boolean) {
  if (pendingConfirm == null) return;
  const id = pendingConfirm;
  hideConfirm();
  await invoke("confirm_answer", { id, yes });
}

// ───────────────────────────── spontaneous speech queue ─────────────────────────────

interface QueuedLine {
  text: string;
  kind: string;
  at: number;
}

const IMPORTANT = new Set(["greeting", "achievement", "event", "timer", "pet", "wake"]);
const lineQueue: QueuedLine[] = [];
let pumping = false;

function enqueueLine(text: string, kind: string) {
  if (!text || lineQueue.some((l) => l.text === text)) return;
  lineQueue.push({ text, kind, at: Date.now() });
  while (lineQueue.length > 4) {
    const idx = lineQueue.findIndex((l) => !IMPORTANT.has(l.kind));
    lineQueue.splice(idx >= 0 ? idx : 0, 1);
  }
  void pumpLines();
}

/** One bubble at a time — fixes overlapping / prematurely hidden bubbles. */
async function pumpLines() {
  if (pumping) return;
  pumping = true;
  try {
    while (lineQueue.length > 0) {
      if (chatBusy || streamSpeakBusy || recording || transcribing || settingsOpen) {
        await sleep(500);
        continue;
      }
      const l = lineQueue.shift()!;
      const maxAge = IMPORTANT.has(l.kind) ? 180_000 : 30_000;
      if (Date.now() - l.at > maxAge) continue;
      await showSpontaneousBubble(l.text, l.kind);
      await sleep(350);
    }
  } finally {
    pumping = false;
  }
}

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

/** Separate bubble window above the pet. */
async function showSpontaneousBubble(text: string, kind: string) {
  speaking = true;
  setSpeaking(pet(), true);
  let stopChirps = false;
  const pulse = () => {
    if (!stopChirps) chirpEyes(pet());
  };
  try {
    await unlockAnimalese();
    const typed = waitBubbleTyped(Math.min(8000, 1200 + text.length * 40)).then(() => {
      stopChirps = true;
    });
    await invoke("show_speech_bubble", { text, kind });
    await Promise.all([speakAnimalese(text, pulse, () => stopChirps).catch(() => undefined), typed]);
    // Long enough to read once typing is done.
    await sleep(Math.min(9000, 2600 + text.length * 50));
  } catch (err) {
    console.error("bubble", err);
  } finally {
    stopChirps = true;
    speaking = false;
    setSpeaking(pet(), false);
    await invoke("hide_speech_bubble").catch(() => undefined);
  }
}

// ───────────────────────────── microphone (Whisper) ─────────────────────────────

async function toggleMic() {
  if (transcribing) return;
  if (recording) {
    await stopMic();
    return;
  }
  const st = await invoke<SttStatus>("stt_status");
  if (!st.available) {
    showBubble(esc(st.message));
    return;
  }
  try {
    await invoke("stt_start");
    recording = true;
    sttBtn().classList.add("rec");
    setUiFace("wide");
    setStatus("🎙 Je t'écoute… (reclique pour envoyer, Échap pour annuler)");
  } catch (err) {
    showError(err);
  }
}

async function stopMic() {
  if (!recording) return;
  recording = false;
  transcribing = true;
  sttBtn().classList.remove("rec");
  sttBtn().classList.add("busy");
  setUiFace(null);
  setStatus("✍️ Je transcris…");
  try {
    const text = (await invoke<string>("stt_stop")).trim();
    if (text) {
      setStatus("");
      promptInput().value = "";
      sendPrompt(text);
    } else {
      setStatus("Je n'ai rien entendu…");
    }
  } catch (err) {
    setStatus("");
    showError(err);
  } finally {
    transcribing = false;
    sttBtn().classList.remove("busy");
    sttBtn().style.setProperty("--level", "0");
  }
}

async function cancelMic() {
  recording = false;
  sttBtn().classList.remove("rec");
  sttBtn().style.setProperty("--level", "0");
  setUiFace(null);
  setStatus("");
  await invoke("stt_cancel").catch(() => undefined);
}

// ───────────────────────────── pointer ─────────────────────────────

function sendPointer(phase: "down" | "move" | "up", clientX: number, clientY: number) {
  void invoke("pointer_event", {
    input: { phase, screen_x: winX + clientX, screen_y: winY + clientY },
  });
}

function wirePointer() {
  const el = pet();
  let down = false;
  let lastMove = 0;

  el.addEventListener("pointerenter", () => {
    if (!sleepingNow) void invoke("pet_hover");
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

// ───────────────────────────── settings modal ─────────────────────────────

async function openSettings() {
  settingsOpen = true;
  await invoke("set_settings_open", { open: true });
  await loadSettingsIntoUi();
  showTab("general");
  modal().classList.remove("hidden");
  document.body.classList.add("settings-open");
}

async function closeSettings() {
  if (!settingsOpen) return;
  try {
    await saveSettingsFromUi();
  } catch (err) {
    // Keep the modal open so the user can fix the field.
    const title = $("#settings-title");
    title.textContent = String(err);
    title.classList.add("err");
    window.setTimeout(() => {
      title.textContent = "Paramètres";
      title.classList.remove("err");
    }, 4000);
    return;
  }
  settingsOpen = false;
  modal().classList.add("hidden");
  document.body.classList.remove("settings-open");
  await invoke("set_settings_open", { open: false });
  void pumpLines();
}

function wireHud() {
  $<HTMLFormElement>("#prompt-form").addEventListener("submit", (e) => {
    e.preventDefault();
    const msg = promptInput().value.trim();
    if (!msg || chatBusy) return;
    promptInput().value = "";
    sendPrompt(msg);
  });

  gear().addEventListener("click", (e) => {
    e.stopPropagation();
    void openSettings();
  });
  $("#settings-close").addEventListener("click", () => void closeSettings());
  modal().addEventListener("click", (e) => {
    if (e.target === modal()) void closeSettings();
  });

  sttBtn().addEventListener("click", () => void toggleMic());
  $("#confirm-yes").addEventListener("click", () => void answerConfirm(true));
  $("#confirm-no").addEventListener("click", () => void answerConfirm(false));

  document.addEventListener("keydown", (e) => {
    if (e.key !== "Escape") return;
    if (recording) {
      void cancelMic();
      return;
    }
    if (pendingConfirm != null) {
      void answerConfirm(false);
      return;
    }
    if (settingsOpen) {
      void closeSettings();
      return;
    }
    showHud(false);
  });

  wireSettingsControls();
}

// ───────────────────────────── identity / calendar ─────────────────────────────

/** Shortest time the loading hologram stays up, so it never just flickers. */
const GHOST_MIN_MS = 1400;
/** Give up waiting for the hardware probe after this and boot with the default shell. */
const GHOST_MAX_MS = 12000;

function applyIdentity(id: IdentityCard, materialize = false) {
  mountShell(pet(), id.shell, materialize || isGhost(pet()));
  setVoice(id.voice);
  setCachedIdentity(id);
  promptInput().placeholder = `Parle à ${id.display_name}… ou glisse-lui un fichier`;
}

function applyToday(t: TodayInfo) {
  applyDecor(pet(), t.decor ?? null);
  setAmbient(t.particles ?? null);
}

// ───────────────────────────── boot ─────────────────────────────

async function boot() {
  const bootAt = performance.now();
  mountGhost(pet());
  wirePointer();
  wireHud();
  await loadSettingsIntoUi().catch((e) => console.error(e));

  await listen<{ open: boolean }>("hue-hud", (ev) => {
    if (ev.payload.open) showHud(true);
    else tuckHud();
  });

  await listen<TickPayload>("hue-tick", (ev) => {
    const t = ev.payload;
    winX = t.x;
    winY = t.y;
    sleepingNow = Boolean(t.sleeping);
    applyTick(pet(), t);
    setFaceContext(t.peeking && !isGhost(pet()), speaking || t.speaking);
    if (t.hud_should_close && hudOpen && !chatBusy) tuckHud();
    if (t.state === "interacting" && !hudOpen && !t.peeking) showHud(true);
  });

  await listen<{ message: string; kind?: string }>("hue-chatter", (ev) => {
    enqueueLine(ev.payload.message, ev.payload.kind ?? "ambient");
  });

  await listen<{ delta?: string; tool?: string; done?: boolean }>("hue-chat", (ev) => {
    const p = ev.payload;
    if (p.tool) setStatus(p.tool);
    if (p.delta) {
      chatBuffer += p.delta;
      const el = bubble();
      showBubble(renderMarkdown(chatBuffer.trimStart()));
      enqueueStreamSpeech(p.delta);
      el.scrollTop = el.scrollHeight;
    }
    if (p.done) void pumpStreamSpeech();
  });

  // While the hologram is up, keep the freshest card for the reveal instead of popping early.
  let pendingIdentity: IdentityCard | null = null;
  await listen<IdentityCard>("hue-identity", (ev) => {
    if (isGhost(pet())) pendingIdentity = ev.payload;
    else applyIdentity(ev.payload);
  });

  await listen<{ message: string }>("hue-status", (ev) => {
    const msg = ev.payload.message;
    if (!msg) return;
    if (hudOpen) setStatus(msg);
    else enqueueLine(msg, "status");
  });

  await listen<TodayInfo>("hue-today", (ev) => applyToday(ev.payload));

  await listen<Vitals>("hue-vitals", (ev) => {
    const v = ev.payload;
    feedStats(v.stats);
    setVoiceMood(v.mood.valence, v.mood.energy);
    const temp = Math.max(v.stats.cpu_temp ?? 0, v.stats.gpu_temp ?? 0);
    setHot(temp >= 82 && !sleepingNow);
    setStraining(v.stats.cpu_pct >= 92 && !sleepingNow);
  });

  await listen<{ kind: string }>("hue-fx", (ev) => burst(ev.payload.kind));

  await listen("hue-achievement", () => {
    if (settingsOpen) void refreshOpenTab();
  });

  await listen("hue-settings-changed", async () => {
    await loadSettingsIntoUi().catch(() => undefined);
    if (settingsOpen) void refreshOpenTab();
  });

  await listen<{ id: number; text: string }>("hue-confirm", (ev) => {
    pendingConfirm = ev.payload.id;
    $("#confirm-text").textContent = ev.payload.text;
    $("#confirm-bar").classList.remove("hidden");
    if (!hudOpen) showHud(true);
    void playSpeech(ev.payload.text);
  });
  await listen<{ id: number }>("hue-confirm-clear", (ev) => {
    if (pendingConfirm === ev.payload.id) hideConfirm();
  });

  await listen<{ level: number }>("hue-mic-level", (ev) => {
    sttBtn().style.setProperty("--level", ev.payload.level.toFixed(2));
  });
  await listen("hue-mic-autostop", () => void stopMic());

  // Drop a file on HUE → summary + opinion (RAG-indexed for later questions).
  await getCurrentWebview().onDragDropEvent((ev) => {
    const p = ev.payload;
    const hint = $("#drop-hint");
    if (p.type === "enter" || p.type === "over") {
      hint.classList.remove("hidden");
      setUiFace("wide");
    } else if (p.type === "leave") {
      hint.classList.add("hidden");
      setUiFace(null);
    } else if (p.type === "drop") {
      hint.classList.add("hidden");
      setUiFace(null);
      const path = p.paths?.[0];
      if (path) analyzeFile(path);
    }
  });

  invoke<TodayInfo>("get_today").then(applyToday).catch(() => undefined);
  void unlockAnimalese();

  // Hologram until the real shell is known (hardware probe), then materialize.
  const identity = await Promise.race([
    invoke<IdentityCard>("get_identity").catch((e) => {
      console.error(e);
      return null;
    }),
    sleep(GHOST_MAX_MS).then(() => null),
  ]);
  const waited = performance.now() - bootAt;
  if (waited < GHOST_MIN_MS) await sleep(GHOST_MIN_MS - waited);
  const card = pendingIdentity ?? identity;
  if (card) applyIdentity(card, true);
  else {
    mountShell(pet(), null, true);
    // Late probe: swap to the real shell as soon as it lands.
    invoke<IdentityCard>("get_identity").then((id) => applyIdentity(id, true)).catch(() => undefined);
  }
  // Everything is listening and visible: the backend may greet now.
  await invoke("frontend_ready");
}

window.addEventListener("DOMContentLoaded", () => {
  void boot();
});
