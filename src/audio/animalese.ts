let ctx: AudioContext | null = null;
/** User sliders (settings). */
let pitch = 1;
let speed = 1;
let enabled = true;
/** Machine voice (from hardware): multiplies the user's sliders. */
let voicePitch = 1;
let voiceSpeed = 1;
let wave: OscillatorType = "square";
/** Mood modulation: happy = a bit higher, tired = slower and lower. */
let moodPitch = 1;
let moodSpeed = 1;

export function setAnimaleseParams(p: number, s: number) {
  pitch = p;
  speed = s;
}

export function setVoice(v: { pitch: number; speed: number; wave: string } | null | undefined) {
  if (!v) return;
  voicePitch = v.pitch || 1;
  voiceSpeed = v.speed || 1;
  wave = (["square", "triangle", "sawtooth", "sine"].includes(v.wave) ? v.wave : "square") as OscillatorType;
}

export function setVoiceMood(valence: number, energy: number) {
  moodPitch = 1 + Math.max(-0.08, Math.min(0.08, valence * 0.07)) + (energy - 0.5) * 0.06;
  moodSpeed = 0.88 + Math.max(0, Math.min(1, energy)) * 0.24;
}

function effPitch() {
  return pitch * voicePitch * moodPitch;
}

function effSpeed() {
  return speed * voiceSpeed * moodSpeed;
}

export function setAnimaleseEnabled(on: boolean) {
  enabled = on;
}

export function isAnimaleseEnabled() {
  return enabled;
}

function ensureCtx(): AudioContext {
  if (!ctx) {
    ctx = new AudioContext();
  }
  return ctx;
}

/** Call from a user gesture so WebView2 allows audio (autoplay policy). */
export async function unlockAnimalese() {
  if (!enabled) return;
  const audio = ensureCtx();
  if (audio.state === "suspended") {
    try {
      await audio.resume();
    } catch {
      /* ignore */
    }
  }
}

function sleep(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}

/** One short blip — always runs timing + onPulse (eyes); audio only if enabled. */
export async function speakAnimaleseBlip(token: string, onPulse?: () => void) {
  if (!token.trim()) return;

  const sp = effSpeed();
  const waitMs = Math.max(28, (45 + token.length * 5) / sp);
  onPulse?.();

  if (!enabled) {
    await sleep(waitMs);
    return;
  }

  const audio = ensureCtx();
  if (audio.state === "suspended") {
    try {
      await audio.resume();
    } catch {
      await sleep(waitMs);
      return;
    }
  }
  if (audio.state !== "running") {
    await sleep(waitMs);
    return;
  }

  const dur = Math.min(0.11, 0.035 + token.length * 0.01) / sp;
  const t0 = audio.currentTime;
  const osc = audio.createOscillator();
  const gain = audio.createGain();
  const filter = audio.createBiquadFilter();

  const hash = [...token].reduce((a, c) => a + c.charCodeAt(0), 0);
  // Questions rise at the end, exclamations punch.
  const lift = /\?$/.test(token) ? 1.18 : /!$/.test(token) ? 1.08 : 1;
  const freq = (280 + (hash % 220)) * effPitch() * lift;

  osc.type = wave;
  osc.frequency.value = freq;
  filter.type = "bandpass";
  filter.frequency.value = freq * 1.4;
  filter.Q.value = 4;

  // Softer waves need a bit more gain to sound as loud.
  const peak = wave === "sine" ? 0.3 : wave === "triangle" ? 0.26 : wave === "sawtooth" ? 0.14 : 0.18;
  gain.gain.setValueAtTime(0.0001, t0);
  gain.gain.exponentialRampToValueAtTime(peak, t0 + 0.01);
  gain.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);

  osc.connect(filter);
  filter.connect(gain);
  gain.connect(audio.destination);

  osc.start(t0);
  osc.stop(t0 + dur + 0.02);

  await sleep(waitMs);
}

/** Procedural animalese for a full phrase (spontaneous / HUD welcome). */
export async function speakAnimalese(
  text: string,
  onPulse?: () => void,
  shouldStop?: () => boolean,
) {
  const tokens = text.split(/\s+/).filter(Boolean);
  const chunks =
    tokens.length > 0 ? tokens : (text.match(/.{1,4}/g) ?? []);
  for (const token of chunks) {
    if (shouldStop?.()) break;
    await speakAnimaleseBlip(token, onPulse);
  }
}
