use std::path::Path;

const SENTINEL: &[u8] = b"PODIUM_APK_PLACEHOLDER";

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let apks_dir = Path::new(&manifest).join("apks");

    for name in ["runner.apk", "sampleapp.apk"] {
        let path = apks_dir.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        if !path.exists() {
            std::fs::create_dir_all(&apks_dir).unwrap();
            std::fs::write(&path, SENTINEL).unwrap();
        }
    }
}
