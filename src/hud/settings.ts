import { invoke } from "@tauri-apps/api/core";
import {
  setAnimaleseEnabled,
  setAnimaleseParams,
} from "../audio/animalese";
import {
  setPeekDisplayMode,
  type PeekDisplayMode,
} from "../pet/faceDisplay";

export type TalkFrequency =
  | "mute"
  | "discrete"
  | "normal"
  | "chatty"
  | "hyperactive";

export type MovementMode = "fullscreen" | "taskbar" | "static";

export interface HueSettings {
  talk_frequency: TalkFrequency;
  stealth_auto: boolean;
  movement_mode: MovementMode;
  peek_display: PeekDisplayMode;
  llm_backend: "local" | "api";
  gguf_path: string;
  api_endpoint: string;
  api_key: string;
  api_model: string;
  animalese_pitch: number;
  animalese_speed: number;
  animalese_enabled: boolean;
  launch_at_startup: boolean;
}

const FREQ_LABELS = ["Muet", "Discret", "Normal", "Bavard", "Hyperactif"];
const FREQ_VALUES: TalkFrequency[] = [
  "mute",
  "discrete",
  "normal",
  "chatty",
  "hyperactive",
];

export function syncLlmFields(backend?: string) {
  const select = document.querySelector("#llm-backend") as HTMLSelectElement;
  const value = backend ?? select.value;
  const api = document.querySelector("#llm-api-fields") as HTMLElement;
  const local = document.querySelector("#llm-local-fields") as HTMLElement;
  const isApi = value === "api";
  api.classList.toggle("hidden", !isApi);
  local.classList.toggle("hidden", isApi);
}

export async function loadSettingsIntoUi() {
  const s = await invoke<HueSettings>("get_settings");
  (document.querySelector("#talk-freq") as HTMLInputElement).value = String(
    FREQ_VALUES.indexOf(s.talk_frequency),
  );
  (document.querySelector("#talk-freq-label") as HTMLElement).textContent =
    FREQ_LABELS[FREQ_VALUES.indexOf(s.talk_frequency)] ?? "Normal";
  (document.querySelector("#stealth-auto") as HTMLInputElement).checked =
    s.stealth_auto;
  (document.querySelector("#movement-mode") as HTMLSelectElement).value =
    s.movement_mode ?? "taskbar";
  (document.querySelector("#peek-display") as HTMLSelectElement).value =
    s.peek_display ?? "all";
  setPeekDisplayMode((s.peek_display ?? "all") as PeekDisplayMode);
  (document.querySelector("#llm-backend") as HTMLSelectElement).value =
    s.llm_backend;
  (document.querySelector("#gguf-path") as HTMLInputElement).value = s.gguf_path;
  (document.querySelector("#api-endpoint") as HTMLInputElement).value =
    s.api_endpoint;
  (document.querySelector("#api-key") as HTMLInputElement).value = s.api_key;
  (document.querySelector("#api-model") as HTMLInputElement).value = s.api_model;
  (document.querySelector("#anim-pitch") as HTMLInputElement).value = String(
    s.animalese_pitch,
  );
  (document.querySelector("#anim-speed") as HTMLInputElement).value = String(
    s.animalese_speed,
  );
  (document.querySelector("#anim-enabled") as HTMLInputElement).checked =
    s.animalese_enabled !== false;
  (document.querySelector("#launch-at-startup") as HTMLInputElement).checked =
    Boolean(s.launch_at_startup);
  setAnimaleseParams(s.animalese_pitch, s.animalese_speed);
  setAnimaleseEnabled(s.animalese_enabled !== false);
  syncLlmFields(s.llm_backend);
  return s;
}

export function wireSettingsControls() {
  const freq = document.querySelector("#talk-freq") as HTMLInputElement;
  const label = document.querySelector("#talk-freq-label") as HTMLElement;
  freq.addEventListener("input", () => {
    label.textContent = FREQ_LABELS[Number(freq.value)] ?? "Normal";
  });
  document.querySelector("#llm-backend")?.addEventListener("change", () => {
    syncLlmFields();
  });
  document.querySelector("#peek-display")?.addEventListener("change", (e) => {
    const v = (e.target as HTMLSelectElement).value as PeekDisplayMode;
    setPeekDisplayMode(v);
  });
  document.querySelector("#anim-enabled")?.addEventListener("change", (e) => {
    setAnimaleseEnabled((e.target as HTMLInputElement).checked);
  });
}

export async function saveSettingsFromUi(): Promise<HueSettings> {
  const freqIdx = Number(
    (document.querySelector("#talk-freq") as HTMLInputElement).value,
  );
  const settings: HueSettings = {
    talk_frequency: FREQ_VALUES[freqIdx] ?? "normal",
    stealth_auto: (document.querySelector("#stealth-auto") as HTMLInputElement)
      .checked,
    movement_mode: (document.querySelector("#movement-mode") as HTMLSelectElement)
      .value as MovementMode,
    peek_display: (document.querySelector("#peek-display") as HTMLSelectElement)
      .value as PeekDisplayMode,
    llm_backend: (document.querySelector("#llm-backend") as HTMLSelectElement)
      .value as "local" | "api",
    gguf_path: (document.querySelector("#gguf-path") as HTMLInputElement).value,
    api_endpoint: (document.querySelector("#api-endpoint") as HTMLInputElement)
      .value,
    api_key: (document.querySelector("#api-key") as HTMLInputElement).value,
    api_model: (document.querySelector("#api-model") as HTMLInputElement).value,
    animalese_pitch: Number(
      (document.querySelector("#anim-pitch") as HTMLInputElement).value,
    ),
    animalese_speed: Number(
      (document.querySelector("#anim-speed") as HTMLInputElement).value,
    ),
    animalese_enabled: (document.querySelector("#anim-enabled") as HTMLInputElement)
      .checked,
    launch_at_startup: (
      document.querySelector("#launch-at-startup") as HTMLInputElement
    ).checked,
  };
  const saved = await invoke<HueSettings>("update_settings", { settings });
  setAnimaleseParams(saved.animalese_pitch, saved.animalese_speed);
  setAnimaleseEnabled(saved.animalese_enabled !== false);
  setPeekDisplayMode(saved.peek_display ?? "all");
  syncLlmFields(saved.llm_backend);
  return saved;
}

export async function renderDna() {
  const id = await invoke<Record<string, unknown>>("get_identity");
  const view = document.querySelector("#dna-view") as HTMLElement;
  view.textContent = JSON.stringify(id, null, 2);
}
