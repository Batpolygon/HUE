fn main() {
    // Force re-embed when the custom HUE icon changes (avoid stale Tauri default).
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/icon.png");
    println!("cargo:rerun-if-changed=icons/hue-tray.svg");
    tauri_build::build()
}
