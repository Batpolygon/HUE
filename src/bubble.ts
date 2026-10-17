import { emit, listen } from "@tauri-apps/api/event";

const el = () => document.querySelector("#bubble") as HTMLElement;

function sleep(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}

async function typewriter(text: string, msPerChar = 22) {
  const node = el();
  node.textContent = "";
  for (let i = 0; i < text.length; i++) {
    node.textContent += text[i];
    await sleep(msPerChar);
  }
}

window.addEventListener("DOMContentLoaded", () => {
  void listen<{ text: string }>("hue-bubble", async (ev) => {
    await typewriter(ev.payload.text);
    // Signal main window: text finished writing → stop eye chirps / close timing
    await emit("hue-bubble-typed", { text: ev.payload.text });
  });

  void listen("hue-bubble-clear", () => {
    el().textContent = "";
  });
});
