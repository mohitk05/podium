use std::path::PathBuf;

// Podium version — APKs are published as assets on the matching GitHub release.
const PODIUM_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAESTRO_VERSION: &str = "2.6.1";

const APKS: &[&str] = &["maestro-app", "maestro-server"];

fn main() {
    // ── proto codegen ────────────────────────────────────────────────────────
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc not found");
    std::env::set_var("PROTOC", protoc);
    tonic_build::configure()
        .build_server(false)
        .compile_protos(&["proto/maestro_android.proto"], &["proto"])
        .expect("tonic_build failed");

    // ── APK resolution ───────────────────────────────────────────────────────
    // Priority:
    //   1. vendor/<stem>-<maestro_version>.apk  (repo checkout, CI)
    //   2. GitHub release asset download         (crates.io consumers)
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let vendor_dir = manifest_dir.parent().unwrap().join("vendor");

    for stem in APKS {
        let filename = format!("{stem}-{MAESTRO_VERSION}.apk");
        let dest = out_dir.join(&filename);

        if dest.exists() {
            continue;
        }

        // 1. Check vendor/ in the workspace root
        let vendor = vendor_dir.join(&filename);
        if vendor.exists() {
            std::fs::copy(&vendor, &dest).unwrap_or_else(|e| {
                panic!(
                    "failed to copy {} -> {}: {e}",
                    vendor.display(),
                    dest.display()
                )
            });
            continue;
        }

        // 2. Download from GitHub release
        let url = format!(
            "https://github.com/mohitk05/podium/releases/download/v{PODIUM_VERSION}/{filename}"
        );
        eprintln!("podium build: downloading {url}");
        let bytes = reqwest::blocking::get(&url)
            .unwrap_or_else(|e| panic!("failed to fetch {url}: {e}"))
            .error_for_status()
            .unwrap_or_else(|e| panic!("HTTP error fetching {url}: {e}"))
            .bytes()
            .unwrap_or_else(|e| panic!("failed to read response for {url}: {e}"));
        std::fs::write(&dest, &bytes)
            .unwrap_or_else(|e| panic!("failed to write {}: {e}", dest.display()));
    }
}
