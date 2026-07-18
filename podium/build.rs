use std::path::PathBuf;

// Podium version — APKs are published as assets on the matching GitHub release.
const PODIUM_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAESTRO_VERSION: &str = "2.6.1";

const APKS: &[(&str, &str)] = &[
    ("maestro-app", "maestro-app"),
    ("maestro-server", "maestro-server"),
];

fn main() {
    // ── proto codegen ────────────────────────────────────────────────────────
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc not found");
    std::env::set_var("PROTOC", protoc);
    tonic_build::configure()
        .build_server(false)
        .compile_protos(&["proto/maestro_android.proto"], &["proto"])
        .expect("tonic_build failed");

    // ── APK download ─────────────────────────────────────────────────────────
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    for (file_stem, asset_stem) in APKS {
        let dest = out_dir.join(format!("{file_stem}-{MAESTRO_VERSION}.apk"));
        if !dest.exists() {
            let url = format!(
                "https://github.com/mohitk05/podium/releases/download/v{PODIUM_VERSION}/{asset_stem}-{MAESTRO_VERSION}.apk"
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
}
