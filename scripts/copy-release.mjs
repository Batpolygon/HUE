import { copyFileSync, existsSync, mkdirSync } from "node:fs";
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

copyFileSync(path.join(srcDir, "hue.exe"), path.join(releaseDir, "HUE.exe"));
const setup = path.join(srcDir, "bundle", "nsis", "HUE_1.0.0_x64-setup.exe");
if (existsSync(setup)) {
  copyFileSync(setup, path.join(releaseDir, "HUE_1.0.0_x64-setup.exe"));
}
console.log(`copied from ${srcDir} → release/`);
