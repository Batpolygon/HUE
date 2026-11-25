import { copyFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cargoTarget =
  process.env.CARGO_TARGET_DIR ||
  path.join(
    process.env.LOCALAPPDATA || "",
    "Temp",
    "cursor-sandbox-cache",
    "0e90bc121fc29c74b0f17c5ac33385a1",
    "cargo-target",
  );

const candidates = [
  path.join(root, "src-tauri", "target", "release"),
  path.join(cargoTarget, "release"),
];

const releaseDir = path.join(root, "release");
mkdirSync(releaseDir, { recursive: true });

let srcDir = candidates.find((d) => existsSync(path.join(d, "hue.exe")));
if (!srcDir) {
  console.error("copy-release: hue.exe not found in known target dirs");
  process.exit(1);
}

const { version } = JSON.parse(readFileSync(path.join(root, "src-tauri", "tauri.conf.json"), "utf8"));
copyFileSync(path.join(srcDir, "hue.exe"), path.join(releaseDir, "HUE.exe"));
const setupName = `HUE_${version}_x64-setup.exe`;
const setup = path.join(srcDir, "bundle", "nsis", setupName);
if (existsSync(setup)) {
  copyFileSync(setup, path.join(releaseDir, setupName));
} else {
  console.warn(`copy-release: ${setupName} not found (installer skipped)`);
}
console.log(`copied from ${srcDir} → release/`);
