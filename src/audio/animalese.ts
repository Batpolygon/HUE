let ctx: AudioContext | null = null;
let pitch = 1;
let speed = 1;
let enabled = true;

export function setAnimaleseParams(p: number, s: number) {
  pitch = p;
  speed = s;
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

  const waitMs = Math.max(28, (45 + token.length * 5) / speed);
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

  const dur = Math.min(0.11, 0.035 + token.length * 0.01) / speed;
  const t0 = audio.currentTime;
  const osc = audio.createOscillator();
  const gain = audio.createGain();
  const filter = audio.createBiquadFilter();

  const hash = [...token].reduce((a, c) => a + c.charCodeAt(0), 0);
  const freq = (280 + (hash % 220)) * pitch;

  osc.type = "square";
  osc.frequency.value = freq;
  filter.type = "bandpass";
  filter.frequency.value = freq * 1.4;
  filter.Q.value = 4;

  const peak = 0.2;
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
