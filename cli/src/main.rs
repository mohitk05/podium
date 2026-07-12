use clap::{Parser, Subcommand};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

// APKs embedded at compile time by build.rs.
// When building without the Android artifacts (e.g. `cargo build` on a fresh
// clone), build.rs writes a sentinel placeholder so the binary still compiles.
const BUNDLED_RUNNER_APK: &[u8] = include_bytes!("../apks/runner.apk");
const BUNDLED_SAMPLEAPP_APK: &[u8] = include_bytes!("../apks/sampleapp.apk");
const APK_SENTINEL: &[u8] = b"PODIUM_APK_PLACEHOLDER";

/// Returns a path to a usable APK file, extracting the embedded bytes if needed.
/// Returns None when `explicit` is None and the bundled bytes are the sentinel.
fn ensure_apk(
    explicit: Option<&Path>,
    bundled: &'static [u8],
    name: &str,
) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    if bundled == APK_SENTINEL {
        return None;
    }
    // Hash the first 256 bytes as a cheap version key
    let mut hasher = Sha256::new();
    hasher.update(&bundled[..bundled.len().min(256)]);
    let hash = format!("{:.8x}", u64::from_be_bytes(hasher.finalize()[..8].try_into().unwrap()));
    let dir = std::env::temp_dir().join("podium-apks").join(&hash);
    let path = dir.join(name);
    if !path.exists() {
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::write(&path, bundled).ok()?;
    }
    Some(path)
}

#[derive(Parser)]
#[command(name = "podium", about = "On-device Maestro-flow runner")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run flows on a connected device or emulator
    Test {
        /// Flow file or directory of .yaml flow files
        flows: PathBuf,
        /// Path to the app APK to install
        #[arg(long)]
        app: Option<PathBuf>,
        /// Path to the runner (test) APK
        #[arg(long)]
        runner: Option<PathBuf>,
        /// ADB device serial (omit to use the only connected device)
        #[arg(long)]
        serial: Option<String>,
        /// Environment variables passed to flows as KEY=VALUE
        #[arg(long = "env", value_name = "KEY=VALUE")]
        env: Vec<String>,
        /// Output directory for results and screenshots
        #[arg(long, default_value = "podium-out")]
        out: PathBuf,
    },
    /// Validate flow files without a device
    Validate {
        /// Flow file or directory of .yaml flow files
        flows: PathBuf,
        /// Environment variables as KEY=VALUE
        #[arg(long = "env", value_name = "KEY=VALUE")]
        env: Vec<String>,
    },
    /// Pretty-print results from a previous run
    Report {
        /// Directory containing result JSON files
        dir: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Validate { flows, env } => cmd_validate(&flows, &parse_env(&env)),
        Cmd::Test {
            flows,
            app,
            runner,
            serial,
            env,
            out,
        } => cmd_test(
            &flows,
            app.as_deref(),
            runner.as_deref(),
            serial.as_deref(),
            &parse_env(&env),
            &out,
        ),
        Cmd::Report { dir } => cmd_report(&dir),
    }
}

fn parse_env(pairs: &[String]) -> HashMap<String, String> {
    pairs
        .iter()
        .filter_map(|s| {
            let mut parts = s.splitn(2, '=');
            let k = parts.next()?.to_string();
            let v = parts.next().unwrap_or("").to_string();
            Some((k, v))
        })
        .collect()
}

// ── validate ────────────────────────────────────────────────────────────────

fn cmd_validate(path: &Path, env: &HashMap<String, String>) -> ExitCode {
    let files = collect_yaml_files(path);
    if files.is_empty() {
        eprintln!("No .yaml files found at {}", path.display());
        return ExitCode::FAILURE;
    }

    let mut all_ok = true;
    for f in &files {
        let yaml = match std::fs::read_to_string(f) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error reading {}: {e}", f.display());
                all_ok = false;
                continue;
            }
        };
        match podium_core::parse_flow(yaml, env.clone()) {
            Ok(flow) => println!("✓  {} (appId: {})", f.display(), flow.app_id),
            Err(e) => {
                eprintln!("✗  {}: {e}", f.display());
                all_ok = false;
            }
        }
    }
    if all_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

// ── test ─────────────────────────────────────────────────────────────────────

const DEVICE_FLOWS_DIR: &str = "/data/local/tmp/podium/flows";

fn cmd_test(
    flows_path: &Path,
    app_apk: Option<&Path>,
    runner_apk: Option<&Path>,
    serial: Option<&str>,
    env: &HashMap<String, String>,
    out_dir: &Path,
) -> ExitCode {
    // 1. Validate flows locally first
    println!("Validating flows…");
    let files = collect_yaml_files(flows_path);
    if files.is_empty() {
        eprintln!("No .yaml files found at {}", flows_path.display());
        return ExitCode::FAILURE;
    }
    let mut ok = true;
    for f in &files {
        let yaml = match std::fs::read_to_string(f) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error reading {}: {e}", f.display());
                return ExitCode::FAILURE;
            }
        };
        match podium_core::parse_flow(yaml, env.clone()) {
            Ok(_) => println!("  ✓ {}", f.display()),
            Err(e) => {
                eprintln!("  ✗ {}: {e}", f.display());
                ok = false;
            }
        }
    }
    if !ok {
        eprintln!("Flow validation failed — aborting (no device touched).");
        return ExitCode::FAILURE;
    }

    // 2. Install APKs
    if let Some(path) = ensure_apk(app_apk, BUNDLED_SAMPLEAPP_APK, "sampleapp.apk") {
        println!("Installing app APK: {}", path.display());
        if !adb(serial, &["install", "-r", &path.to_string_lossy()]).success() {
            eprintln!("Failed to install app APK");
            return ExitCode::FAILURE;
        }
    }

    let runner_path = match ensure_apk(runner_apk, BUNDLED_RUNNER_APK, "runner.apk") {
        Some(p) => p,
        None => {
            eprintln!(
                "No runner APK available. Either pass --runner <path> or use a release build \
                 that has the APKs embedded."
            );
            return ExitCode::FAILURE;
        }
    };
    println!("Installing runner APK: {}", runner_path.display());
    if !adb(serial, &["install", "-r", &runner_path.to_string_lossy()]).success() {
        eprintln!("Failed to install runner APK");
        return ExitCode::FAILURE;
    }

    // 3. Push flows to device (clean dir first so stale flows don't run)
    println!("Pushing flows to device…");
    adb(serial, &["shell", "rm", "-rf", DEVICE_FLOWS_DIR]);
    if !adb(serial, &["shell", "mkdir", "-p", DEVICE_FLOWS_DIR]).success() {
        eprintln!("Failed to create device flows dir");
        return ExitCode::FAILURE;
    }
    for f in &files {
        let dest = format!(
            "{}/{}",
            DEVICE_FLOWS_DIR,
            f.file_name().unwrap().to_string_lossy()
        );
        if !adb(serial, &["push", &f.to_string_lossy(), &dest]).success() {
            eprintln!("Failed to push {}", f.display());
            return ExitCode::FAILURE;
        }
    }

    // 4. Build instrumentation args
    let mut instrument_args: Vec<String> = vec![
        "shell".into(),
        "am".into(),
        "instrument".into(),
        "-w".into(),
        "-r".into(),
        "-e".into(),
        "flowsDir".into(),
        DEVICE_FLOWS_DIR.into(),
    ];
    for (k, v) in env {
        instrument_args.push("-e".into());
        instrument_args.push(format!("env.{k}"));
        instrument_args.push(v.clone());
    }
    instrument_args.push("dev.podium.runner.test/androidx.test.runner.AndroidJUnitRunner".into());

    // 5. Run and stream output
    println!("\nRunning flows…\n");
    std::fs::create_dir_all(out_dir).ok();
    let passed = stream_instrument(serial, &instrument_args);

    // 6. Pull results (the test APK package is dev.podium.runner.test)
    println!("\nPulling results…");
    let results_candidates = [
        "/sdcard/Android/data/dev.podium.runner.test/files/podium/results",
        "/sdcard/Android/data/dev.podium.runner/files/podium/results",
    ];
    let mut pulled = false;
    for device_results in &results_candidates {
        if adb(
            serial,
            &["pull", device_results, &out_dir.to_string_lossy()],
        )
        .success()
        {
            pulled = true;
            break;
        }
    }
    if !pulled {
        eprintln!("  (no result files found on device)");
    }

    if passed {
        println!("\nAll flows passed. Results in {}", out_dir.display());
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "\nOne or more flows failed. Results in {}",
            out_dir.display()
        );
        ExitCode::FAILURE
    }
}

fn stream_instrument(serial: Option<&str>, args: &[String]) -> bool {
    // Clear logcat buffer before run so we don't pick up old PODIUM lines
    let mut clear_args = vec![];
    if let Some(s) = serial {
        clear_args.extend(["-s", s]);
    }
    Command::new("adb")
        .args(&clear_args)
        .arg("logcat")
        .arg("-c")
        .output()
        .ok();

    // Spawn am instrument
    let mut inst_args: Vec<&str> = Vec::new();
    if let Some(s) = serial {
        inst_args.extend(["-s", s]);
    }
    inst_args.extend(args.iter().map(|s| s.as_str()));
    let mut child = match Command::new("adb")
        .args(&inst_args)
        .stdout(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to spawn adb: {e}");
            return false;
        }
    };

    // Spawn logcat to stream PODIUM lines
    let mut logcat_args = vec![];
    if let Some(s) = serial {
        logcat_args.extend(["-s", s]);
    }
    logcat_args.extend(["logcat", "-v", "raw", "-s", "System.out:I"]);
    let mut logcat = Command::new("adb")
        .args(&logcat_args)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .ok();

    // Stream logcat PODIUM lines in a background thread
    let logcat_stdout = logcat.as_mut().and_then(|c| c.stdout.take());
    let logcat_thread = logcat_stdout.map(|stdout| {
        std::thread::spawn(move || {
            use std::io::BufRead;
            for line in std::io::BufReader::new(stdout).lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => break,
                };
                if let Some(rendered) = render_podium_line(&line) {
                    println!("{rendered}");
                }
            }
        })
    });

    // Wait for am instrument and check result
    use std::io::BufRead;
    let stdout = child.stdout.take().unwrap();
    let mut passed = true;
    for line in std::io::BufReader::new(stdout).lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.contains("INSTRUMENTATION_STATUS_CODE: -2") || line.contains("FAILURES!!!") {
            passed = false;
        }
    }
    let status = child.wait().unwrap_or_default();

    // Kill logcat and wait for thread
    if let Some(mut lc) = logcat {
        lc.kill().ok();
    }
    if let Some(t) = logcat_thread {
        t.join().ok();
    }

    passed && status.success()
}

/// Parse a `PODIUM|type|...` line and render it nicely.
pub fn render_podium_line(line: &str) -> Option<String> {
    let content = line.strip_prefix("PODIUM|")?;
    let parts: Vec<&str> = content.splitn(5, '|').collect();
    match parts.as_slice() {
        ["info", msg, ..] => Some(format!("ℹ  {msg}")),
        ["flow", name, "started", ..] => Some(format!("\n▶  flow: {name}")),
        ["flow", name, "passed", ..] => Some(format!("✅ flow {name} passed")),
        ["flow", name, "failed", ..] => Some(format!("❌ flow {name} FAILED")),
        ["flow", name, "error", msg, ..] => Some(format!("❌ flow {name} error: {msg}")),
        ["step", desc, "passed", ms, ..] => Some(format!("  ✓ {desc} ({ms})")),
        ["step", desc, "failed", ms, ..] => Some(format!("  ✗ {desc} ({ms})")),
        ["step", desc, "skipped", ..] => Some(format!("  ⊘ {desc} (skipped)")),
        ["warning", msg, ..] => Some(format!("⚠  {msg}")),
        _ => None,
    }
}

fn adb(serial: Option<&str>, args: &[&str]) -> std::process::ExitStatus {
    let mut cmd = Command::new("adb");
    if let Some(s) = serial {
        cmd.args(["-s", s]);
    }
    cmd.args(args);
    cmd.status().unwrap_or_else(|_| {
        // Return a fake failure status
        Command::new("false").status().unwrap()
    })
}

// ── report ────────────────────────────────────────────────────────────────

fn cmd_report(dir: &Path) -> ExitCode {
    let jsons: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Err(e) => {
            eprintln!("Cannot read {}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
        Ok(rd) => rd
            .filter_map(|e| {
                let p = e.ok()?.path();
                (p.extension()?.to_str()? == "json" && p.file_name()?.to_str()? != "junit.xml")
                    .then_some(p)
            })
            .collect(),
    };

    if jsons.is_empty() {
        eprintln!("No result JSON files found in {}", dir.display());
        return ExitCode::FAILURE;
    }

    println!("{:<30} {:>6} {:>8} {}", "Flow", "Steps", "Total", "Status");
    println!("{}", "─".repeat(60));

    let mut all_passed = true;
    for path in &jsons {
        let name = path.file_stem().unwrap_or_default().to_string_lossy();
        let text = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Cannot read {}: {e}", path.display());
                continue;
            }
        };
        let val: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Bad JSON in {}: {e}", path.display());
                continue;
            }
        };
        let passed = val["passed"].as_bool().unwrap_or(false);
        let steps = val["steps"].as_array().map(|a| a.len()).unwrap_or(0);
        let total_ms: u64 = val["steps"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|s| s["duration_ms"].as_u64().unwrap_or(0))
                    .sum()
            })
            .unwrap_or(0);
        let status = if passed { "PASSED" } else { "FAILED" };
        println!("{:<30} {:>6} {:>7}ms {}", name, steps, total_ms, status);
        if !passed {
            all_passed = false;
            // Print failing step details
            if let Some(steps_arr) = val["steps"].as_array() {
                for s in steps_arr {
                    if s["status"].as_str() == Some("FAILED") {
                        let cmd = s["command"].as_str().unwrap_or("?");
                        let msg = s["failure_message"].as_str().unwrap_or("unknown");
                        println!("    ✗ {cmd}: {msg}");
                    }
                }
            }
        }
    }

    if all_passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

// ── helpers ───────────────────────────────────────────────────────────────

fn collect_yaml_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    match std::fs::read_dir(path) {
        Err(_) => vec![],
        Ok(rd) => {
            let mut files: Vec<PathBuf> = rd
                .filter_map(|e| {
                    let p = e.ok()?.path();
                    let ext = p.extension()?.to_str()?;
                    (ext == "yaml" || ext == "yml").then_some(p)
                })
                .collect();
            files.sort();
            files
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_env_basic() {
        let pairs = vec!["FOO=bar".to_string(), "BAZ=qux".to_string()];
        let env = parse_env(&pairs);
        assert_eq!(env["FOO"], "bar");
        assert_eq!(env["BAZ"], "qux");
    }

    #[test]
    fn test_parse_env_empty_value() {
        let pairs = vec!["KEY=".to_string()];
        let env = parse_env(&pairs);
        assert_eq!(env["KEY"], "");
    }

    #[test]
    fn test_parse_env_value_with_equals() {
        let pairs = vec!["URL=http://example.com?a=1".to_string()];
        let env = parse_env(&pairs);
        assert_eq!(env["URL"], "http://example.com?a=1");
    }

    #[test]
    fn test_parse_env_empty() {
        let env = parse_env(&[]);
        assert!(env.is_empty());
    }

    #[test]
    fn test_render_podium_line_step_passed() {
        let line = "PODIUM|step|tapOn(login_button)|passed|412ms";
        let rendered = render_podium_line(line).unwrap();
        assert!(rendered.contains("tapOn(login_button)"));
        assert!(rendered.contains("412ms"));
        assert!(rendered.contains('✓'));
    }

    #[test]
    fn test_render_podium_line_step_failed() {
        let line = "PODIUM|step|assertVisible(Welcome)|failed|10001ms";
        let rendered = render_podium_line(line).unwrap();
        assert!(rendered.contains("assertVisible(Welcome)"));
        assert!(rendered.contains('✗'));
    }

    #[test]
    fn test_render_podium_line_flow_passed() {
        let line = "PODIUM|flow|login|passed";
        let rendered = render_podium_line(line).unwrap();
        assert!(rendered.contains("login"));
        assert!(rendered.contains('✅'));
    }

    #[test]
    fn test_render_podium_line_flow_failed() {
        let line = "PODIUM|flow|smoke|failed";
        let rendered = render_podium_line(line).unwrap();
        assert!(rendered.contains("smoke"));
        assert!(rendered.contains('❌'));
    }

    #[test]
    fn test_render_podium_line_unknown() {
        let line = "some random log line";
        assert!(render_podium_line(line).is_none());
    }

    #[test]
    fn test_collect_yaml_files_single_file() {
        // This just tests with a path that doesn't exist — returns empty
        let files = collect_yaml_files(Path::new("/nonexistent/dir"));
        assert!(files.is_empty());
    }
}
