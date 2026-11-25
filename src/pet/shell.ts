/**
 * Procedural shell. The black screen (28,34 64×40) and the orange eyes never change;
 * everything around them is generated from the PC's hardware (ShellSpec).
 */
import type { ShellSpec } from "../types";

const EYE = "#ff8a1f";

export const DEFAULT_SHELL: ShellSpec = {
  form: "desktop",
  body: "#2a2f3a",
  body_dark: "#1d212a",
  trim: "#3d4555",
  accent: "#ff8a1f",
  corner: 16,
  width_scale: 1,
  vents: 4,
  pattern: "none",
  feet: "pads",
  side_fins: false,
  leds: 1,
  ears: false,
};

interface Frame {
  x0: number;
  x1: number;
  top: number;
  bottom: number;
  rx: number;
  /** SVG for the main body outline (also used as the pattern clip). */
  outline: string;
  /** Drawn before the body (stands, fins…). */
  behind: string;
  /** Drawn over the body, around the screen. */
  details: string;
  /** Where hats sit (body top centre). */
  hatX: number;
  hatY: number;
  hatScale: number;
  feet: boolean;
}

function clamp(v: number, lo: number, hi: number) {
  return Math.max(lo, Math.min(hi, v));
}

function rect(x: number, y: number, w: number, h: number, rx: number) {
  return `<rect x="${x.toFixed(1)}" y="${y.toFixed(1)}" width="${w.toFixed(1)}" height="${h.toFixed(1)}" rx="${rx.toFixed(1)}"/>`;
}

function vents(n: number, x: number, y: number, w: number, color: string, vertical = false) {
  const out: string[] = [];
  const count = clamp(Math.round(n), 1, 8);
  for (let i = 0; i < count; i++) {
    if (vertical) {
      const step = w / count;
      const vx = x + step * i + step / 2;
      out.push(`<line x1="${vx.toFixed(1)}" y1="${y}" x2="${vx.toFixed(1)}" y2="${y + 12}" stroke="${color}" stroke-width="1.6" stroke-linecap="round"/>`);
    } else {
      const vy = y + i * 3.2;
      out.push(`<line x1="${x}" y1="${vy.toFixed(1)}" x2="${x + w}" y2="${vy.toFixed(1)}" stroke="${color}" stroke-width="1.4" stroke-linecap="round"/>`);
    }
  }
  return out.join("");
}

function leds(n: number, x: number, y: number, accent: string) {
  const out: string[] = [];
  for (let i = 0; i < clamp(n, 0, 4); i++) {
    out.push(`<circle class="led" style="animation-delay:${(i * 0.7).toFixed(1)}s" cx="${(x + i * 5).toFixed(1)}" cy="${y}" r="1.4" fill="${accent}"/>`);
  }
  return out.join("");
}

function ears(f: Frame, s: ShellSpec) {
  if (!s.ears) return "";
  const l = f.x0 + 14;
  const r = f.x1 - 14;
  const t = f.top;
  const ear = (cx: number, dir: number) =>
    `<path d="M${cx - 8} ${t + 2} Q${cx - 2 * dir} ${t - 11} ${cx + 8} ${t + 2} Z" fill="${s.body}" stroke="${s.trim}" stroke-width="2" stroke-linejoin="round"/>`;
  return ear(l, 1) + ear(r, -1);
}

function feet(f: Frame, s: ShellSpec) {
  if (!f.feet) return "";
  const b = f.bottom;
  switch (s.feet) {
    case "pads":
      return `<g fill="${s.body_dark}">${rect(f.x0 + 9, b - 1, 14, 5, 2)}${rect(f.x1 - 23, b - 1, 14, 5, 2)}</g>`;
    case "block":
      return `<g fill="${s.body_dark}">${rect(f.x0 + 6, b - 1, f.x1 - f.x0 - 12, 5, 2.5)}</g>`;
    case "round":
      return `<g fill="${s.body_dark}"><circle cx="${f.x0 + 15}" cy="${b + 1}" r="3.6"/><circle cx="${f.x1 - 15}" cy="${b + 1}" r="3.6"/></g>`;
    default:
      return "";
  }
}

function pattern(kind: string, color: string) {
  const lines: string[] = [];
  switch (kind) {
    case "stripes":
      for (let i = -120; i < 140; i += 9) {
        lines.push(`<line x1="${i}" y1="0" x2="${i + 120}" y2="120" stroke="${color}" stroke-width="2.2"/>`);
      }
      break;
    case "dots":
      for (let y = 6; y < 120; y += 8) {
        for (let x = 6 + ((y / 8) % 2) * 4; x < 120; x += 8) {
          lines.push(`<circle cx="${x}" cy="${y}" r="1.1" fill="${color}"/>`);
        }
      }
      break;
    case "grid":
      for (let i = 0; i < 120; i += 10) {
        lines.push(`<line x1="${i}" y1="0" x2="${i}" y2="120" stroke="${color}" stroke-width="0.8"/>`);
        lines.push(`<line x1="0" y1="${i}" x2="120" y2="${i}" stroke="${color}" stroke-width="0.8"/>`);
      }
      break;
    case "circuit":
      lines.push(
        `<g fill="none" stroke="${color}" stroke-width="1.3" stroke-linejoin="round">` +
          `<polyline points="20,86 32,86 38,80 50,80"/><polyline points="100,88 88,88 84,94 70,94"/>` +
          `<polyline points="22,28 34,28 38,22 56,22"/><polyline points="98,26 86,26 82,20"/>` +
          `<polyline points="24,60 24,72 30,78"/><polyline points="96,58 96,70 90,76"/></g>` +
          `<g fill="${color}"><circle cx="50" cy="80" r="1.8"/><circle cx="70" cy="94" r="1.8"/><circle cx="56" cy="22" r="1.8"/><circle cx="82" cy="20" r="1.8"/><circle cx="30" cy="78" r="1.8"/><circle cx="90" cy="76" r="1.8"/></g>`,
      );
      break;
    default:
      return "";
  }
  return lines.join("");
}

function frameFor(s: ShellSpec): Frame {
  const ws = clamp(s.width_scale || 1, 0.85, 1.12);
  const corner = clamp(s.corner || 14, 4, 24);
  const dark = s.body_dark;
  const accent = s.accent;
  switch (s.form) {
    case "laptop": {
      const w = 82 * ws;
      const x0 = 60 - w / 2;
      const x1 = 60 + w / 2;
      const rx = Math.min(corner, 10);
      const deck =
        `<path d="M${x0 - 6} 83 L${x1 + 6} 83 L${x1 + 13} 96 Q${x1 + 14} 99 ${x1 + 10} 99 L${x0 - 10} 99 Q${x0 - 14} 99 ${x0 - 13} 96 Z" fill="${s.body}" stroke="${s.trim}" stroke-width="2.2" stroke-linejoin="round"/>` +
        `<g stroke="${dark}" stroke-width="1.3" stroke-linecap="round" opacity="0.8">` +
        `<line x1="${x0 + 2}" y1="87.5" x2="${x1 - 2}" y2="87.5" stroke-dasharray="3 1.6"/>` +
        `<line x1="${x0}" y1="91" x2="${x1}" y2="91" stroke-dasharray="3 1.6"/></g>` +
        `<rect x="52" y="93.5" width="16" height="3.6" rx="1.4" fill="${dark}"/>` +
        leds(s.leds, x1 - 2, 95.5, accent);
      return {
        x0, x1, top: 24, bottom: 83, rx,
        outline: rect(x0, 24, w, 59, rx),
        behind: deck,
        details: `<rect x="${x0 + 2}" y="80" width="${w - 4}" height="3" fill="${dark}" opacity="0.7"/>`,
        hatX: 60 + w * 0.22, hatY: 26, hatScale: 1, feet: false,
      };
    }
    case "tower": {
      const w = 74 * ws;
      const x0 = 60 - w / 2;
      const x1 = 60 + w / 2;
      const rx = Math.min(corner, 12);
      let fins = "";
      if (s.side_fins) {
        for (let y = 34; y < 96; y += 9) {
          fins += `<rect x="${x0 - 4}" y="${y}" width="6" height="4.5" rx="1.5" fill="${dark}"/>`;
          fins += `<rect x="${x1 - 2}" y="${y}" width="6" height="4.5" rx="1.5" fill="${dark}"/>`;
        }
      }
      const details =
        `<rect x="${x0 + 5}" y="30" width="2.6" height="66" rx="1.3" fill="${accent}" class="accent-strip"/>` +
        `<circle cx="${x1 - 11}" cy="20" r="3.4" fill="none" stroke="${accent}" stroke-width="1.6"/>` +
        `<line x1="${x1 - 11}" y1="16.8" x2="${x1 - 11}" y2="19.6" stroke="${accent}" stroke-width="1.6" stroke-linecap="round"/>` +
        vents(Math.max(3, s.vents), 36, 82, 48, dark, true) +
        leds(s.leds, x0 + 13, 20, accent);
      return {
        x0, x1, top: 10, bottom: 104, rx,
        outline: rect(x0, 10, w, 94, rx),
        behind: fins,
        details,
        hatX: 60 - w * 0.1, hatY: 21, hatScale: 0.7, feet: true,
      };
    }
    case "aio": {
      const w = clamp(94 * ws, 88, 104);
      const x0 = 60 - w / 2;
      const x1 = 60 + w / 2;
      const rx = Math.min(corner, 9);
      const stand =
        `<rect x="53" y="83" width="14" height="13" fill="${dark}"/>` +
        `<rect x="38" y="95" width="44" height="5" rx="2.5" fill="${s.body}" stroke="${s.trim}" stroke-width="1.6"/>`;
      const details =
        `<rect x="${x0 + 1.5}" y="76" width="${w - 3}" height="8" rx="${Math.max(1, rx - 2)}" fill="${dark}" opacity="0.55"/>` +
        `<circle cx="60" cy="80" r="1.8" fill="${accent}"/>` +
        leds(s.leds, x1 - 12, 80, accent);
      return {
        x0, x1, top: 24, bottom: 84, rx,
        outline: rect(x0, 24, w, 60, rx),
        behind: stand,
        details,
        hatX: 60 + w * 0.24, hatY: 26, hatScale: 1, feet: false,
      };
    }
    case "mini": {
      const w = 72 * ws;
      const x0 = 60 - w / 2;
      const x1 = 60 + w / 2;
      const rx = clamp(corner + 6, 12, 26);
      const details =
        `<circle cx="60" cy="85" r="3.2" fill="none" stroke="${accent}" stroke-width="1.6" class="accent-ring"/>` +
        vents(Math.min(3, s.vents), x0 + 10, 81, 12, dark) +
        leds(s.leds, x1 - 22, 85, accent);
      return {
        x0, x1, top: 26, bottom: 94, rx,
        outline: rect(x0, 26, w, 68, rx),
        behind: "",
        details,
        hatX: 60 + w * 0.2, hatY: 28, hatScale: 0.95, feet: true,
      };
    }
    case "handheld": {
      const x0 = 4;
      const x1 = 116;
      const behind =
        `<rect x="10" y="26" width="22" height="8" rx="4" fill="${dark}"/>` +
        `<rect x="88" y="26" width="22" height="8" rx="4" fill="${dark}"/>`;
      const details =
        // D-pad
        `<g fill="${dark}"><rect x="11" y="52" width="12" height="4.5" rx="1.2"/><rect x="14.75" y="48.25" width="4.5" height="12" rx="1.2"/></g>` +
        // Face buttons
        `<g><circle cx="104" cy="49" r="2.6" fill="${accent}"/><circle cx="110" cy="55" r="2.6" fill="${dark}"/><circle cx="98" cy="55" r="2.6" fill="${dark}"/><circle cx="104" cy="61" r="2.6" fill="${dark}"/></g>` +
        // Sticks
        `<circle cx="20" cy="72" r="4.2" fill="${dark}" stroke="${s.trim}" stroke-width="1"/>` +
        `<circle cx="100" cy="72" r="4.2" fill="${dark}" stroke="${s.trim}" stroke-width="1"/>` +
        leds(s.leds, 56, 80, accent);
      return {
        x0, x1, top: 30, bottom: 84, rx: 24,
        outline: rect(x0, 30, x1 - x0, 54, 24),
        behind,
        details,
        hatX: 60 + 20, hatY: 32, hatScale: 0.95, feet: false,
      };
    }
    default: {
      const w = 84 * ws;
      const x0 = 60 - w / 2;
      const x1 = 60 + w / 2;
      const rx = corner;
      const details =
        vents(s.vents, x0 + 12, 81, 16, dark) +
        `<rect x="${x1 - 24}" y="84" width="12" height="2.6" rx="1.3" fill="${accent}" class="accent-strip"/>` +
        leds(s.leds, x1 - 22, 91, accent);
      return {
        x0, x1, top: 22, bottom: 98, rx,
        outline: rect(x0, 22, w, 76, rx),
        behind: s.side_fins
          ? `<rect x="${x0 - 3}" y="40" width="5" height="34" rx="2" fill="${dark}"/><rect x="${x1 - 2}" y="40" width="5" height="34" rx="2" fill="${dark}"/>`
          : "",
        details,
        hatX: 60 + w * 0.2, hatY: 24, hatScale: 1, feet: true,
      };
    }
  }
}

let hatAnchor = { x: 78, y: 24, scale: 1 };

const GHOST = "#5ec8ff";

/**
 * Boot placeholder: a translucent blue hologram of a generic frame with a boot screen
 * (spinner + message + indeterminate bar), shown until the real shell is known.
 * Everything sits inside the screen so it stays readable even when tucked in the corner.
 */
export function buildGhostSvg(): string {
  return `
<svg viewBox="0 0 120 120" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
  <defs>
    <linearGradient id="ghostGrad" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="${GHOST}" stop-opacity="0.30"/>
      <stop offset="1" stop-color="${GHOST}" stop-opacity="0.08"/>
    </linearGradient>
    <clipPath id="ghostClip"><rect x="18" y="22" width="84" height="76" rx="16"/></clipPath>
    <clipPath id="ghostScreen"><rect x="28" y="34" width="64" height="40" rx="8"/></clipPath>
  </defs>
  <g id="rig">
    <rect x="18" y="22" width="84" height="76" rx="16" fill="url(#ghostGrad)" stroke="${GHOST}" stroke-width="1.8" stroke-opacity="0.8" stroke-dasharray="5 2.5" class="ghost-outline"/>
    <g clip-path="url(#ghostClip)"><rect class="ghost-scan" x="18" y="12" width="84" height="9" fill="${GHOST}" opacity="0.25"/></g>
    <g fill="${GHOST}" opacity="0.35">${rect(27, 97, 14, 5, 2)}${rect(79, 97, 14, 5, 2)}</g>
    <rect x="28" y="34" width="64" height="40" rx="8" fill="#04101a" fill-opacity="0.78" stroke="${GHOST}" stroke-opacity="0.55" stroke-width="1"/>
    <g clip-path="url(#ghostScreen)">
      <g class="ghost-spinner">
        <circle cx="60" cy="46" r="6" fill="none" stroke="${GHOST}" stroke-opacity="0.22" stroke-width="2.2"/>
        <path d="M60 40 A6 6 0 0 1 66 46" fill="none" stroke="${GHOST}" stroke-width="2.2" stroke-linecap="round"/>
      </g>
      <text x="60" y="62" text-anchor="middle" font-size="6.6" font-weight="700" fill="${GHOST}" class="ghost-text">HUE se charge…</text>
      <rect x="40" y="65.5" width="40" height="2.4" rx="1.2" fill="${GHOST}" opacity="0.2"/>
      <rect class="ghost-bar" x="40" y="65.5" width="14" height="2.4" rx="1.2" fill="${GHOST}"/>
    </g>
  </g>
</svg>`;
}

export function buildShellSvg(spec: ShellSpec | null | undefined): string {
  const s = { ...DEFAULT_SHELL, ...(spec ?? {}) };
  const f = frameFor(s);
  hatAnchor = { x: f.hatX, y: f.hatY, scale: f.hatScale };
  const outlineFilled = f.outline.replace(
    "/>",
    ` fill="url(#bodyGrad)" stroke="${s.trim}" stroke-width="2.6"/>`,
  );
  return `
<svg viewBox="0 0 120 120" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
  <defs>
    <linearGradient id="bodyGrad" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="${s.body}"/>
      <stop offset="1" stop-color="${s.body_dark}"/>
    </linearGradient>
    <clipPath id="bodyClip">${f.outline}</clipPath>
    <clipPath id="screenClip"><rect x="28" y="34" width="64" height="40" rx="8"/></clipPath>
  </defs>
  <g id="rig">
  <g id="shell">
    ${f.behind}
    ${ears(f, s)}
    ${feet(f, s)}
    ${outlineFilled}
    <g clip-path="url(#bodyClip)" opacity="0.2">${pattern(s.pattern, s.trim)}</g>
    ${f.details}
    <rect x="26" y="32" width="68" height="44" rx="9.5" fill="${s.body_dark}"/>
  </g>
  <rect x="28" y="34" width="64" height="40" rx="8" fill="#0b0d12"/>
  <g id="eyes" clip-path="url(#screenClip)">
    <g id="eyes-normal">
      <ellipse id="eye-l" class="eye" cx="46" cy="54" rx="8" ry="10" fill="${EYE}"/>
      <ellipse id="eye-r" class="eye" cx="74" cy="54" rx="8" ry="10" fill="${EYE}"/>
    </g>
    <g class="face face-happy" fill="none" stroke="${EYE}" stroke-width="4.2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M38 58 Q46 45 54 58"/><path d="M66 58 Q74 45 82 58"/>
    </g>
    <g class="face face-strain" fill="none" stroke="${EYE}" stroke-width="4" stroke-linecap="round" stroke-linejoin="round">
      <path d="M39 47 L52 54 L39 61"/><path d="M81 47 L68 54 L81 61"/>
    </g>
    <g class="face face-love" fill="${EYE}">
      <path d="M46 63 C36 56 37 46 42.5 46 C45 46 46 48.5 46 50 C46 48.5 47 46 49.5 46 C55 46 56 56 46 63 Z"/>
      <path d="M74 63 C64 56 65 46 70.5 46 C73 46 74 48.5 74 50 C74 48.5 75 46 77.5 46 C83 46 84 56 74 63 Z"/>
    </g>
    <g class="face face-dead" stroke="${EYE}" stroke-width="4" stroke-linecap="round">
      <path d="M40 48 L52 60 M52 48 L40 60"/><path d="M68 48 L80 60 M80 48 L68 60"/>
    </g>
    <g class="face face-sleepy" fill="none" stroke="${EYE}" stroke-width="4" stroke-linecap="round">
      <path d="M38 55 Q46 61 54 55"/><path d="M66 55 Q74 61 82 55"/>
    </g>
    <g class="face face-sleep" fill="none" stroke="${EYE}" stroke-width="3.6" stroke-linecap="round" opacity="0.8">
      <path d="M39 57 L53 57"/><path d="M67 57 L81 57"/>
    </g>
    <g class="face face-sad" fill="${EYE}">
      <path d="M38 52 L54 48 L54 56 Q54 64 46 64 Q38 64 38 56 Z"/>
      <path d="M82 52 L66 48 L66 56 Q66 64 74 64 Q82 64 82 56 Z"/>
    </g>
    <g class="face face-wide">
      <circle cx="46" cy="54" r="11" fill="${EYE}"/><circle cx="74" cy="54" r="11" fill="${EYE}"/>
      <circle cx="46" cy="54" r="3.4" fill="#0b0d12"/><circle cx="74" cy="54" r="3.4" fill="#0b0d12"/>
    </g>
    <g id="blush" opacity="0">
      <ellipse cx="36" cy="67" rx="6" ry="2.6" fill="#ff6fa1"/>
      <ellipse cx="84" cy="67" rx="6" ry="2.6" fill="#ff6fa1"/>
    </g>
  </g>
  <rect x="30" y="35.5" width="22" height="3" rx="1.5" fill="#ffffff" opacity="0.06" pointer-events="none"/>
  <g id="dizzy-fx" class="dizzy-fx" clip-path="url(#screenClip)" opacity="0" aria-hidden="true">
    <circle class="dizzy-star" cx="38" cy="40" r="2.2" fill="#ffd27a"/>
    <circle class="dizzy-star" cx="82" cy="42" r="1.8" fill="#ffb347"/>
    <circle class="dizzy-star" cx="60" cy="38" r="1.6" fill="#ffe6a8"/>
    <path class="dizzy-swirl" d="M52 48c6-4 14-2 16 4" fill="none" stroke="${EYE}" stroke-width="1.6" stroke-linecap="round" opacity="0.85"/>
    <path class="dizzy-swirl" d="M68 60c-5 4-13 3-16-3" fill="none" stroke="#ffb347" stroke-width="1.4" stroke-linecap="round" opacity="0.7"/>
  </g>
  <g id="decor"></g>
  </g>
</svg>`;
}

const HATS: Record<string, string> = {
  santa_hat:
    `<path d="M-15 0 Q-5 -24 13 -15 Q5 -9 15 0 Z" fill="#d62d2d"/>` +
    `<rect x="-17" y="-3.5" width="34" height="7" rx="3.5" fill="#f4f4f4"/>` +
    `<circle cx="13.5" cy="-15" r="3.6" fill="#f4f4f4"/>`,
  witch_hat:
    `<ellipse cx="0" cy="0" rx="21" ry="4.2" fill="#2b1d3a"/>` +
    `<path d="M-10 -1 L3 -27 Q5 -20 10 -1 Z" fill="#3a2650"/>` +
    `<rect x="-10" y="-6" width="20" height="3.4" fill="#ff8a1f"/>`,
  party_hat:
    `<path d="M-10 0 L0 -24 L10 0 Z" fill="#4fc3f7"/>` +
    `<path d="M-6.5 -8 L6.5 -8 M-3.6 -15.5 L3.6 -15.5" stroke="#fff176" stroke-width="2.6"/>` +
    `<circle cx="0" cy="-25" r="3" fill="#ff8a65"/>`,
  crown:
    `<path d="M-15 0 L-15 -13 L-7.5 -5.5 L0 -16 L7.5 -5.5 L15 -13 L15 0 Z" fill="#ffca28" stroke="#c79100" stroke-width="1.1" stroke-linejoin="round"/>` +
    `<circle cx="0" cy="-16" r="2" fill="#e53935"/><circle cx="-15" cy="-13" r="1.6" fill="#42a5f5"/><circle cx="15" cy="-13" r="1.6" fill="#42a5f5"/>`,
  flower:
    `<g transform="translate(0 -4)">` +
    [0, 72, 144, 216, 288]
      .map((a) => `<ellipse cx="0" cy="-5.5" rx="3.6" ry="5.5" fill="#f48fb1" transform="rotate(${a})"/>`)
      .join("") +
    `<circle r="3.4" fill="#ffd54f"/></g>`,
};

/** Seasonal hat (or nothing). Call after every shell rebuild. */
export function setDecor(svgRoot: Element | null, decor: string | null) {
  const g = svgRoot?.querySelector("#decor");
  if (!g) return;
  const hat = decor ? HATS[decor] : undefined;
  if (!hat) {
    g.innerHTML = "";
    return;
  }
  const { x, y, scale } = hatAnchor;
  g.innerHTML = `<g class="hat" transform="translate(${x.toFixed(1)} ${y.toFixed(1)}) rotate(12) scale(${scale})">${hat}</g>`;
}
