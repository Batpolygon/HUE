import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const rcedit = path.join(root, "node_modules", "rcedit", "bin", "rcedit.exe");
const ico = path.join(root, "src-tauri", "icons", "hue-tray.ico");
// Only patch the app binary — NSIS installers break if rcedit rewrites them.
const targets = [path.join(root, "release", "HUE.exe")];

if (!existsSync(rcedit) || !existsSync(ico)) {
  console.warn("set-exe-icon: rcedit or icon.ico missing — skip");
  process.exit(0);
}

for (const exe of targets) {
  if (!existsSync(exe)) continue;
  const r = spawnSync(rcedit, [exe, "--set-icon", ico], { stdio: "inherit" });
  if (r.status !== 0) process.exit(r.status ?? 1);
  console.log(`icon set: ${path.relative(root, exe)}`);
}
