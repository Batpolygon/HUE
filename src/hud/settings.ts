import { invoke } from "@tauri-apps/api/core";
import {
  setAnimaleseEnabled,
  setAnimaleseParams,
  speakAnimalese,
  unlockAnimalese,
} from "../audio/animalese";
import { setPeekDisplayMode } from "../pet/faceDisplay";
import { setParticlesEnabled } from "../pet/particles";
import { buildShellSvg } from "../pet/shell";
import type {
  AchievementView,
  HueSettings,
  IdentityCard,
  MovementMode,
  PeekDisplayMode,
  SttStatus,
  TalkFrequency,
} from "../types";

export type { HueSettings } from "../types";

const FREQ_LABELS = ["Muet", "Discret", "Normal", "Bavard", "Hyperactif"];
const FREQ_VALUES: TalkFrequency[] = ["mute", "discrete", "normal", "chatty", "hyperactive"];

const $ = <T extends HTMLElement>(sel: string) => document.querySelector(sel) as T;
const input = (sel: string) => $<HTMLInputElement>(sel);
const select = (sel: string) => $<HTMLSelectElement>(sel);

function esc(s: string) {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

/** Last settings seen (keeps fields the UI doesn't edit). */
let current: HueSettings | null = null;

export function syncLlmFields(backend?: string) {
  const value = backend ?? select("#llm-backend").value;
  $("#llm-api-fields").classList.toggle("hidden", value !== "api");
  $("#llm-local-fields").classList.toggle("hidden", value === "api");
}

/** Apply settings that change the live pet (sound, particles, peek face). */
export function applyRuntimeSettings(s: HueSettings) {
  setAnimaleseParams(s.animalese_pitch, s.animalese_speed);
  setAnimaleseEnabled(s.animalese_enabled !== false);
  setPeekDisplayMode((s.peek_display ?? "all") as PeekDisplayMode);
  setParticlesEnabled(s.particles !== false);
}

function sliderLabels() {
  $("#talk-freq-label").textContent = FREQ_LABELS[Number(input("#talk-freq").value)] ?? "Normal";
  $("#anim-pitch-label").textContent = `×${Number(input("#anim-pitch").value).toFixed(2)}`;
  $("#anim-speed-label").textContent = `×${Number(input("#anim-speed").value).toFixed(2)}`;
}

/** "MM-DD" / "YYYY-MM-DD" (storage) → "JJ/MM[/AAAA]" (display). */
function birthdayToUi(v: string) {
  const m = /^(?:(\d{4})-)?(\d{2})-(\d{2})$/.exec(v.trim());
  if (!m) return v;
  return m[1] ? `${m[3]}/${m[2]}/${m[1]}` : `${m[3]}/${m[2]}`;
}

/** "JJ/MM[/AAAA]" → "MM-DD" / "YYYY-MM-DD". */
function birthdayFromUi(v: string) {
  const t = v.trim();
  if (!t) return "";
  const m = /^(\d{1,2})[/.-](\d{1,2})(?:[/.-](\d{4}))?$/.exec(t);
  if (!m) return t;
  const d = m[1].padStart(2, "0");
  const mo = m[2].padStart(2, "0");
  return m[3] ? `${m[3]}-${mo}-${d}` : `${mo}-${d}`;
}

export async function loadSettingsIntoUi() {
  const s = await invoke<HueSettings>("get_settings");
  current = s;
  input("#talk-freq").value = String(Math.max(0, FREQ_VALUES.indexOf(s.talk_frequency)));
  input("#llm-thoughts").checked = s.llm_thoughts !== false;
  select("#movement-mode").value = s.movement_mode ?? "fullscreen";
  select("#peek-display").value = s.peek_display ?? "all";
  input("#sleep-after").value = String(s.sleep_after_min ?? 5);
  input("#stealth-auto").checked = s.stealth_auto;
  input("#seasonal-decor").checked = s.seasonal_decor !== false;
  input("#particles").checked = s.particles !== false;
  input("#launch-at-startup").checked = Boolean(s.launch_at_startup);

  input("#user-birthday").value = birthdayToUi(s.user_birthday ?? "");
  input("#share-window-title").checked = Boolean(s.share_window_title);

  select("#llm-backend").value = s.llm_backend;
  input("#gguf-path").value = s.gguf_path;
  input("#api-endpoint").value = s.api_endpoint;
  input("#api-key").value = s.api_key;
  input("#api-model").value = s.api_model;
  select("#stt-engine").value = s.stt_engine ?? "local";
  select("#stt-model").value = s.stt_model ?? "base";
  select("#stt-language").value = s.stt_language || "fr";

  input("#anim-enabled").checked = s.animalese_enabled !== false;
  input("#anim-pitch").value = String(s.animalese_pitch);
  input("#anim-speed").value = String(s.animalese_speed);

  sliderLabels();
  syncLlmFields(s.llm_backend);
  applyRuntimeSettings(s);
  return s;
}

function collect(): HueSettings {
  const base = current ?? ({} as HueSettings);
  const sleep = Math.round(Number(input("#sleep-after").value));
  return {
    ...base,
    talk_frequency: FREQ_VALUES[Number(input("#talk-freq").value)] ?? "normal",
    llm_thoughts: input("#llm-thoughts").checked,
    movement_mode: select("#movement-mode").value as MovementMode,
    peek_display: select("#peek-display").value as PeekDisplayMode,
    sleep_after_min: Number.isFinite(sleep) ? Math.max(0, Math.min(120, sleep)) : 5,
    stealth_auto: input("#stealth-auto").checked,
    seasonal_decor: input("#seasonal-decor").checked,
    particles: input("#particles").checked,
    launch_at_startup: input("#launch-at-startup").checked,
    user_birthday: birthdayFromUi(input("#user-birthday").value),
    share_window_title: input("#share-window-title").checked,
    llm_backend: select("#llm-backend").value as "local" | "api",
    gguf_path: input("#gguf-path").value.trim(),
    api_endpoint: input("#api-endpoint").value.trim(),
    api_key: input("#api-key").value.trim(),
    api_model: input("#api-model").value.trim(),
    stt_engine: select("#stt-engine").value as "local" | "api",
    stt_model: select("#stt-model").value as "base" | "small",
    stt_language: select("#stt-language").value,
    animalese_enabled: input("#anim-enabled").checked,
    animalese_pitch: Number(input("#anim-pitch").value),
    animalese_speed: Number(input("#anim-speed").value),
    title_prefix: select("#title-prefix").value ?? base.title_prefix ?? "",
    title_suffix: select("#title-suffix").value ?? base.title_suffix ?? "",
  };
}

export async function saveSettingsFromUi(): Promise<HueSettings> {
  const saved = await invoke<HueSettings>("update_settings", { settings: collect() });
  current = saved;
  applyRuntimeSettings(saved);
  syncLlmFields(saved.llm_backend);
  return saved;
}

// ───────────────────────────── tabs ─────────────────────────────

export function showTab(name: string) {
  document.querySelectorAll<HTMLElement>("#settings-tabs button").forEach((b) => {
    b.classList.toggle("active", b.dataset.tab === name);
  });
  document.querySelectorAll<HTMLElement>(".modal-card .tab").forEach((t) => {
    t.classList.toggle("hidden", t.dataset.tab !== name);
  });
  void refreshTab(name);
}

async function refreshTab(name: string) {
  try {
    if (name === "you") await renderMemory();
    if (name === "ai") await Promise.all([renderDocs(), renderSttStatus()]);
    if (name === "titles") await renderAchievements();
    if (name === "identity") await renderIdentity();
    if (name === "voice") await renderVoiceHint();
  } catch (err) {
    console.error(err);
  }
}

export async function refreshOpenTab() {
  const active = document.querySelector<HTMLElement>("#settings-tabs button.active");
  await refreshTab(active?.dataset.tab ?? "general");
}

// ───────────────────────────── memory ─────────────────────────────

interface MemoryView {
  facts: { text: string; added: number }[];
  stats: Record<string, number | string[]>;
  first_met: number | null;
  turns: number;
}

const STAT_LABELS: [string, string][] = [
  ["days_seen", "Jours ensemble"],
  ["best_streak", "Meilleure série"],
  ["pets", "Caresses"],
  ["chats", "Discussions"],
  ["files_read", "Fichiers lus"],
  ["voice_uses", "Dictées"],
  ["naps", "Siestes"],
  ["lags", "Lags survécus"],
  ["dizzies", "Tournis"],
  ["timers_done", "Minuteurs"],
];

async function renderMemory() {
  const m = await invoke<MemoryView>("get_memory");
  const list = $("#facts-list");
  if (!m.facts.length) {
    list.innerHTML = `<li class="empty">Rien pour l'instant. Dis-lui « retiens que… ».</li>`;
  } else {
    list.innerHTML = m.facts
      .map((f, i) => `<li><span>${esc(f.text)}</span><button type="button" data-forget="${i}" title="Oublier">✕</button></li>`)
      .join("");
  }
  const since = m.first_met ? new Date(m.first_met * 1000).toLocaleDateString("fr-FR") : "aujourd'hui";
  $("#memory-stats").innerHTML =
    `<div class="stat wide"><b>${esc(since)}</b><span>Première rencontre</span></div>` +
    STAT_LABELS.map(([k, label]) => `<div class="stat"><b>${Number(m.stats[k] ?? 0)}</b><span>${label}</span></div>`).join("");
}

// ───────────────────────────── documents / stt ─────────────────────────────

interface DocSummary {
  name: string;
  kind: string;
  chunks: number;
  summary: string;
  added: number;
}

async function renderDocs() {
  const docs = await invoke<DocSummary[]>("list_documents");
  const list = $("#docs-list");
  if (!docs.length) {
    list.innerHTML = `<li class="empty">Aucun document pour l'instant.</li>`;
    return;
  }
  list.innerHTML = docs
    .map(
      (d) =>
        `<li title="${esc(d.summary || d.name)}"><span><b>${esc(d.name)}</b> <small>${esc(d.kind)} · ${d.chunks} morceau${d.chunks > 1 ? "x" : ""}</small></span>` +
        `<button type="button" data-forget-doc="${esc(d.name)}" title="Oublier ce document">✕</button></li>`,
    )
    .join("");
}

async function renderSttStatus() {
  const st = await invoke<SttStatus>("stt_status");
  $("#stt-status-line").textContent = st.message;
}

// ───────────────────────────── achievements / titles ─────────────────────────────

async function renderAchievements() {
  const all = await invoke<AchievementView[]>("get_achievements");
  const s = current;
  const fill = (sel: HTMLSelectElement, kind: "prefix" | "suffix", value: string) => {
    const opts = all.filter((a) => a.unlocked && a.kind === kind);
    sel.innerHTML =
      `<option value="">— aucun —</option>` +
      opts.map((a) => `<option value="${esc(a.title)}">${esc(a.title)}</option>`).join("");
    sel.value = opts.some((a) => a.title === value) ? value : "";
  };
  fill(select("#title-prefix"), "prefix", s?.title_prefix ?? "");
  fill(select("#title-suffix"), "suffix", s?.title_suffix ?? "");
  updateTitlePreview();

  const unlocked = all.filter((a) => a.unlocked).length;
  $("#ach-count").textContent = `${unlocked} / ${all.length} succès débloqués — chaque succès offre un titre.`;
  $("#ach-list").innerHTML = all
    .map((a) => {
      const hidden = a.secret && !a.unlocked;
      const when = a.unlocked_at ? new Date(a.unlocked_at * 1000).toLocaleDateString("fr-FR") : "";
      return (
        `<li class="${a.unlocked ? "on" : "off"}">` +
        `<span class="ach-icon">${a.unlocked ? "🏆" : hidden ? "❔" : "🔒"}</span>` +
        `<span class="ach-body"><b>${hidden ? "Succès secret" : esc(a.name)}</b>` +
        `<small>${hidden ? "Continue à vivre avec lui…" : esc(a.description)}</small></span>` +
        `<span class="ach-title">${hidden ? "???" : esc(a.title)}<small>${a.kind === "prefix" ? "préfixe" : "suffixe"}${when ? " · " + when : ""}</small></span>` +
        `</li>`
      );
    })
    .join("");
}

let cachedIdentity: IdentityCard | null = null;

export function setCachedIdentity(id: IdentityCard) {
  cachedIdentity = id;
  updateTitlePreview();
}

function updateTitlePreview() {
  const name = cachedIdentity?.display_name ?? "HUE";
  const p = select("#title-prefix")?.value ?? "";
  const s = select("#title-suffix")?.value ?? "";
  const full = [p, name, s].filter(Boolean).join(" ");
  const el = $("#title-preview");
  if (el) el.textContent = full;
}

// ───────────────────────────── identity ─────────────────────────────

const AXIS_LABELS: [keyof IdentityCard["personality"]["axes"], string][] = [
  ["energy", "Énergie"],
  ["curiosity", "Curiosité"],
  ["sociability", "Sociabilité"],
  ["pride", "Fierté"],
  ["anxiety", "Anxiété"],
  ["whimsy", "Fantaisie"],
];

const FORM_LABELS: Record<string, string> = {
  laptop: "Portable",
  tower: "Tour",
  desktop: "Bureau",
  aio: "Tout-en-un",
  mini: "Mini PC",
  handheld: "Console portable",
};

const DISK_LABELS: Record<string, string> = { nvme: "SSD NVMe", ssd: "SSD", hdd: "Disque dur" };

async function renderIdentity() {
  const id = await invoke<IdentityCard>("get_identity");
  cachedIdentity = id;
  const mem = await invoke<MemoryView>("get_memory").catch(() => null);
  const hw = id.hardware;
  const p = id.personality;
  const axes = AXIS_LABELS.map(
    ([k, label]) =>
      `<div class="axis"><span>${label}</span><div class="bar"><i style="width:${Math.round(p.axes[k] * 100)}%"></i></div></div>`,
  ).join("");
  const birth = id.pc_birthday
    ? `${new Date(id.pc_birthday + "T12:00:00").toLocaleDateString("fr-FR", { day: "numeric", month: "long", year: "numeric" })}` +
      (id.pc_age_years != null ? ` (${id.pc_age_years} an${id.pc_age_years > 1 ? "s" : ""})` : "")
    : "inconnue";
  const days = Number(mem?.stats?.days_seen ?? 0);
  const specs: [string, string][] = [
    ["Forme", FORM_LABELS[hw.form_factor] ?? hw.form_factor],
    ["Processeur", `${hw.cpu_name} (${hw.cpu_cores} cœurs)`],
    ["Carte graphique", `${hw.gpu_name}${hw.vram_gb ? ` · ${hw.vram_gb.toFixed(0)} Go` : ""}`],
    ["Mémoire", `${hw.ram_total_gb.toFixed(0)} Go`],
    ["Stockage", `${DISK_LABELS[hw.disk_kind] ?? hw.disk_kind ?? ""} · ${hw.disk_total_gb.toFixed(0)} Go`],
    ["Écrans", String(hw.monitors || 1)],
    ["Système", hw.os_name || "Windows"],
    ["Né le", birth],
    ["Ensemble depuis", `${days} jour${days > 1 ? "s" : ""}`],
  ];
  $("#identity-card").innerHTML =
    `<div class="id-head"><div class="id-shell">${buildShellSvg(id.shell)}</div>` +
    `<div><h3>${esc(id.full_title || id.display_name)}</h3>` +
    `<p class="archetype">${esc(p.archetype)}</p>` +
    `<p class="hint">${esc(p.traits_summary)}</p>` +
    `<p class="voice-tag">Voix : ×${id.voice.pitch.toFixed(2)} · ${esc(id.voice.wave)}</p></div></div>` +
    `<div class="axes">${axes}</div>` +
    `<dl class="specs">${specs.map(([k, v]) => `<dt>${k}</dt><dd>${esc(v)}</dd>`).join("")}</dl>` +
    `<div class="traits">${p.traits.map((t) => `<span>${esc(t)}</span>`).join("")}</div>`;
}

async function renderVoiceHint() {
  const id = cachedIdentity ?? (await invoke<IdentityCard>("get_identity"));
  cachedIdentity = id;
  $("#voice-hint").textContent =
    `Sa voix de base vient de son matériel (hauteur ×${id.voice.pitch.toFixed(2)}, timbre ${id.voice.wave}) ` +
    `et varie avec son humeur. Les curseurs s'y ajoutent.`;
}

// ───────────────────────────── wiring ─────────────────────────────

export function wireSettingsControls() {
  for (const sel of ["#talk-freq", "#anim-pitch", "#anim-speed"]) {
    input(sel).addEventListener("input", () => {
      sliderLabels();
      if (sel !== "#talk-freq") {
        setAnimaleseParams(Number(input("#anim-pitch").value), Number(input("#anim-speed").value));
      }
    });
  }
  select("#llm-backend").addEventListener("change", () => syncLlmFields());
  select("#peek-display").addEventListener("change", (e) => {
    setPeekDisplayMode((e.target as HTMLSelectElement).value as PeekDisplayMode);
  });
  input("#anim-enabled").addEventListener("change", (e) => {
    setAnimaleseEnabled((e.target as HTMLInputElement).checked);
  });
  input("#particles").addEventListener("change", (e) => {
    setParticlesEnabled((e.target as HTMLInputElement).checked);
  });
  select("#title-prefix").addEventListener("change", updateTitlePreview);
  select("#title-suffix").addEventListener("change", updateTitlePreview);

  document.querySelectorAll<HTMLElement>("#settings-tabs button").forEach((b) => {
    b.addEventListener("click", () => showTab(b.dataset.tab ?? "general"));
  });

  $("#facts-list").addEventListener("click", async (e) => {
    const btn = (e.target as HTMLElement).closest<HTMLElement>("[data-forget]");
    if (!btn) return;
    await invoke("forget_fact_index", { index: Number(btn.dataset.forget) });
    await renderMemory();
  });
  $("#docs-list").addEventListener("click", async (e) => {
    const btn = (e.target as HTMLElement).closest<HTMLElement>("[data-forget-doc]");
    if (!btn) return;
    await invoke("forget_document", { name: btn.dataset.forgetDoc });
    await renderDocs();
  });
  $("#clear-conversation").addEventListener("click", async () => {
    await invoke("clear_conversation");
    $("#clear-conversation").textContent = "Conversation oubliée ✓";
    window.setTimeout(() => ($("#clear-conversation").textContent = "Oublier la conversation récente"), 2000);
  });
  $("#voice-test").addEventListener("click", async () => {
    await unlockAnimalese();
    const name = cachedIdentity?.display_name ?? "HUE";
    await speakAnimalese(`Coucou ! C'est moi, ${name}. Ça sonne bien ?`);
  });
}
