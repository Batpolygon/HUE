import { emit, listen } from "@tauri-apps/api/event";

const el = () => document.querySelector("#bubble") as HTMLElement;
const KINDS = ["achievement", "greeting", "event", "thought", "lag", "timer", "status"];

function sleep(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}

let seq = 0;

async function typewriter(text: string, msPerChar = 22) {
  const mine = ++seq;
  const node = el();
  node.textContent = "";
  for (let i = 0; i < text.length; i++) {
    if (mine !== seq) return false;
    node.textContent += text[i];
    await sleep(msPerChar);
  }
  return true;
}

window.addEventListener("DOMContentLoaded", () => {
  void listen<{ text: string; kind?: string }>("hue-bubble", async (ev) => {
    const node = el();
    for (const k of KINDS) node.classList.remove(`k-${k}`);
    if (ev.payload.kind && KINDS.includes(ev.payload.kind)) node.classList.add(`k-${ev.payload.kind}`);
    const done = await typewriter(ev.payload.text);
    // Main window: typing finished → stop eye chirps / start the reading timer.
    if (done) await emit("hue-bubble-typed", { text: ev.payload.text });
  });

  void listen("hue-bubble-clear", () => {
    seq++;
    el().textContent = "";
  });
});
