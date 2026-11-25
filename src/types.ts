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

export type Face = "happy" | "strain" | "love" | "dead" | "sleepy" | "sad" | "wide";
export type Micro = "yawn" | "wiggle" | "look" | "stretch" | "dance" | "hop";

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
  sleeping?: boolean;
  expression?: Face | null;
  micro?: Micro | null;
  valence?: number;
  energy?: number;
}

export interface ShellSpec {
  form: "laptop" | "tower" | "desktop" | "aio" | "mini" | "handheld" | string;
  body: string;
  body_dark: string;
  trim: string;
  accent: string;
  corner: number;
  width_scale: number;
  vents: number;
  pattern: "none" | "stripes" | "dots" | "grid" | "circuit" | string;
  feet: "none" | "pads" | "block" | "round" | string;
  side_fins: boolean;
  leds: number;
  ears: boolean;
}

export interface VoiceSpec {
  pitch: number;
  speed: number;
  wave: OscillatorType;
}

export interface Axes {
  energy: number;
  curiosity: number;
  sociability: number;
  pride: number;
  anxiety: number;
  whimsy: number;
}

export interface HardwareSnapshot {
  hostname: string;
  cpu_name: string;
  cpu_cores: number;
  cpu_freq_mhz: number;
  ram_total_gb: number;
  gpu_name: string;
  vram_gb: number;
  disk_total_gb: number;
  disk_free_gb: number;
  disk_label: string;
  gpu_brand: string;
  form_factor: string;
  disk_kind: string;
  monitors: number;
  has_battery: boolean;
  os_name: string;
}

export interface IdentityCard {
  hostname: string;
  display_name: string;
  full_title: string;
  user_name: string;
  hardware: HardwareSnapshot;
  personality: {
    seed: string;
    axes: Axes;
    bucket: string;
    archetype: string;
    traits: string[];
    traits_summary: string;
  };
  shell: ShellSpec;
  voice: VoiceSpec;
  pc_birthday: string | null;
  pc_age_years: number | null;
}

export interface LiveStats {
  cpu_pct: number;
  ram_pct: number;
  ram_used_gb: number;
  ram_total_gb: number;
  cpu_freq_mhz: number;
  disk_pct: number;
  disk_free_gb: number;
  battery_pct: number | null;
  on_ac: boolean | null;
  gpu_pct: number;
  cpu_temp: number | null;
  gpu_temp: number | null;
  uptime_secs: number;
  hour: number;
  minute: number;
  second: number;
}

export interface Vitals {
  stats: LiveStats;
  mood: { valence: number; energy: number; label: string };
  network_up: boolean;
}

export interface TodayInfo {
  events: { id: string; name: string }[];
  decor: string | null;
  particles: string | null;
}

export type TalkFrequency = "mute" | "discrete" | "normal" | "chatty" | "hyperactive";
export type MovementMode = "fullscreen" | "taskbar" | "static";
export type PeekDisplayMode = "all" | "spec_only" | "clock_only" | "eyes_only";

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
  user_birthday: string;
  title_prefix: string;
  title_suffix: string;
  stt_engine: "local" | "api";
  stt_model: "base" | "small";
  stt_language: string;
  llm_thoughts: boolean;
  share_window_title: boolean;
  sleep_after_min: number;
  seasonal_decor: boolean;
  particles: boolean;
}

export interface AchievementView {
  id: string;
  name: string;
  description: string;
  title: string;
  kind: "prefix" | "suffix";
  unlocked: boolean;
  unlocked_at: number | null;
  secret: boolean;
}

export interface SttStatus {
  available: boolean;
  engine: string;
  ready: boolean;
  recording: boolean;
  message: string;
}
