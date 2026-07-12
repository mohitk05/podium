# PLAN.md — "Podium" PoC: On-Device Maestro-Flow Interpreter (Rust core + UniFFI)

> Execution plan for Claude Code (Sonnet). Work through phases strictly in order.
> Each phase has acceptance criteria — do not start the next phase until the current
> one's criteria pass. Commit at the end of every phase with the message given.

---

## 0. Context & Goal (read first, do not skip)

**Problem.** Appium-based cloud testing is slow because the test interpreter runs on
the client machine and every command (find element, tap, assert) is a separate HTTP
round trip over the WAN to the device farm. Maestro is fast because its decision loop
runs adjacent to the device — but Maestro can't run natively on most cloud farms.

**Thesis to validate.** If the flow interpreter is embedded *inside* an Android
instrumentation test APK (the artifact type every cloud farm — BrowserStack, Sauce
Labs, Firebase Test Lab, AWS Device Farm — accepts via their native Espresso
endpoints), then:
1. The same artifact runs locally (via adb) and on any cloud farm (via upload), and
2. Execution is Maestro-speed or faster, because element matching and action dispatch
   happen in-process on the device with zero network hops.

**Architecture in one paragraph.** The interpreter core — YAML parsing, flow model,
command orchestration, retry/wait/timeout semantics — is a Rust crate. It is exposed
to Kotlin via **UniFFI**. The Rust core drives the loop but never touches Android
APIs directly: it calls out through a UniFFI-exported `Driver` trait, implemented on
the Kotlin side with UIAutomator. This inversion is deliberate — all timing and retry
*semantics* live in Rust, so a future iOS port only needs a Swift `Driver` over
XCUITest and behavior stays identical across platforms.

**PoC scope (hard boundaries — do NOT exceed):**
- Android only. No iOS (but never introduce Android types into the Rust core).
- 10 core commands only (listed in Phase 3). No JavaScript scripting, no `runFlow`
  composition, no env interpolation beyond `${VAR}` string substitution.
- One local CLI, written in Rust (reuses the core crate for offline flow validation).
- Cloud upload = documented curl scripts. Actual cloud runs are gated on credentials
  the human provides later (Phase 7 is optional).

**Non-goals for the PoC:** feature parity with Maestro, iOS support, parallel
execution, report dashboards, plugin systems, moving hierarchy matching into Rust.
If you find yourself building any of these, stop and re-read this section.

---

## 1. Environment Prerequisites (verify before Phase 0)

Run these checks. If any fail, report to the human and stop — do not attempt large
installs (Android SDK/NDK) without the human's approval.

```bash
java -version                      # need JDK 17+
echo $ANDROID_HOME                 # must be set; adb available
ls $ANDROID_HOME/ndk 2>/dev/null   # NDK required for cargo-ndk (26+ preferred)
adb devices                        # device/emulator connected, or AVD available
rustc --version                    # 1.75+
cargo ndk --version                # if missing: cargo install cargo-ndk
rustup target list --installed     # need: aarch64-linux-android, x86_64-linux-android
                                   # if missing: rustup target add <target>
```

x86_64-linux-android matters: local emulators are x86_64, real/cloud devices are
arm64 — the runner APK must bundle both.

If no device is connected but an AVD exists:
```bash
emulator -list-avds
emulator -avd <name> -no-window -no-audio &
adb wait-for-device
```

---

## 2. Repository Layout (create in Phase 0)

```
podium/
├── PLAN.md
├── README.md
├── Cargo.toml                     # workspace: core, cli
├── core/                          # Rust crate "podium-core" (cdylib + rlib)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                 # uniffi setup + exports
│       ├── model.rs               # Flow, Command, Selector
│       ├── parser.rs              # YAML → Flow
│       ├── engine.rs              # interpreter loop, retry/wait semantics
│       └── driver.rs              # #[uniffi::export] Driver trait + StepResult
├── cli/                           # Rust crate "podium" (binary)
│   └── src/main.rs
├── android/                       # Android Gradle project
│   ├── settings.gradle.kts
│   ├── runner/                    # com.android.test module → instrumentation APK
│   │   └── src/main/
│   │       ├── kotlin/dev/podium/runner/   # FlowRunner test + UiAutomatorDriver
│   │       ├── jniLibs/           # populated by cargo-ndk (gitignored)
│   │       └── assets/flows/      # flows baked in for the cloud path
│   └── sampleapp/                 # tiny app under test (dev.podium.sample)
├── flows/                         # sample YAML flows for e2e verification
│   ├── smoke.yaml
│   ├── login.yaml
│   └── bench.yaml                 # Phase 5
└── scripts/
    ├── build-runner.sh            # cargo-ndk → bindgen → gradle assemble
    ├── run-local.sh               # full local demo
    └── upload-browserstack.sh     # Phase 7
```

**Key architectural rules (enforce throughout):**
- `core` has **zero Android/JNI-specific code** beyond what UniFFI generates. Pure
  Rust, unit-testable with `cargo test` on the host.
- All UIAutomator code lives in `UiAutomatorDriver.kt`. Nothing else on the Kotlin
  side contains logic — Kotlin is a thin adapter.
- Timing/retry decisions live in Rust (`engine.rs`), never in the Kotlin driver.

---

## Phase 0 — Scaffold & FFI Walking Skeleton

Goal: prove the entire toolchain (Rust → cargo-ndk → UniFFI bindgen → Kotlin → APK →
device) with a trivial function BEFORE writing any real logic. This de-risks the
build system, which is the most fragile part of this stack.

Tasks:
1. Cargo workspace with `core` and `cli`. `core/Cargo.toml`:
   ```toml
   [lib]
   crate-type = ["cdylib", "rlib"]   # cdylib for Android, rlib for the CLI

   [dependencies]
   uniffi = { version = "0.29", features = ["cli"] }   # verify latest stable
   serde = { version = "1", features = ["derive"] }
   serde_yaml = "0.9"
   regex = "1"
   thiserror = "2"
   ```
   Use **UniFFI proc-macros** (`#[uniffi::export]`, `uniffi::setup_scaffolding!()`),
   not UDL files — less to keep in sync. Add a `uniffi-bindgen` binary target per
   the UniFFI docs so bindings generate via
   `cargo run --bin uniffi-bindgen generate --library <so> --language kotlin`.
2. Export a walking-skeleton function: `#[uniffi::export] fn core_version() -> String`.
3. `scripts/build-runner.sh`:
   ```bash
   cargo ndk -t arm64-v8a -t x86_64 -o android/runner/src/main/jniLibs build --release -p podium-core
   cargo run -p podium-core --bin uniffi-bindgen -- generate \
     --library target/aarch64-linux-android/release/libpodium_core.so \
     --language kotlin --out-dir android/runner/src/main/kotlin
   (cd android && ./gradlew :runner:assembleAndroidTest :sampleapp:assembleDebug)
   ```
4. Android side: `runner` module with `androidx.test:runner`,
   `androidx.test.uiautomator:uiautomator:2.3.+`, and JNA as
   `net.java.dev.jna:jna:<latest>@aar` (the `@aar` classifier is REQUIRED — UniFFI's
   Kotlin bindings load the native lib through JNA, and the plain jar has no Android
   natives; this is the classic first-run crash).
5. `sampleapp`: minimal app, package `dev.podium.sample`. Two screens:
   - Login: username field (resource-id `username`), password field (`password`),
     button text "Log in", inline error text on bad credentials.
   - Home: TextView "Welcome" + scrollable list of 50 items ("Item 1"…"Item 50"),
     tapping the last item shows text "Reached end".
6. Skeleton instrumentation test that calls `core_version()` and logs it.

Acceptance criteria:
- `cargo test -p podium-core` and `cargo build -p podium` pass on host.
- `scripts/build-runner.sh` succeeds; `adb shell am instrument -w ...` on the
  skeleton test prints the version string from Rust on the device/emulator.
- Commit: `phase-0: workspace + uniffi walking skeleton through to device`

---

## Phase 1 — Flow Model & Parser (pure Rust, TDD this phase)

Implement in `core/src/model.rs` + `parser.rs`:

1. Data model (uniffi-annotated where it crosses the FFI boundary):
   ```rust
   pub struct Flow { pub app_id: String, pub commands: Vec<Command> }

   pub enum Command {
     LaunchApp { app_id: Option<String>, clear_state: bool },
     TapOn { selector: Selector },
     InputText { text: String },
     AssertVisible { selector: Selector, timeout_ms: u64 },
     AssertNotVisible { selector: Selector, timeout_ms: u64 },
     ScrollUntilVisible { selector: Selector, max_swipes: u32 },
     Back,
     WaitForAnimationToEnd { timeout_ms: u64 },
     Swipe { direction: Direction },
     TakeScreenshot { name: String },
   }

   pub struct Selector {
     pub text: Option<String>,   // exact, or regex when written /.../ in YAML
     pub id: Option<String>,     // resource-id suffix match
     pub index: u32,
   }
   ```
   Note: UniFFI supports enums with named fields and Option/Vec — keep types within
   what UniFFI can lower (no lifetimes, no generics across the boundary).
2. Parser: accept the Maestro YAML dialect for these commands, shorthand and map
   forms. The Maestro file format is two YAML documents separated by `---` (config
   header, then command list):
   ```yaml
   appId: dev.podium.sample
   ---
   - launchApp:
       clearState: true
   - tapOn: "Log in"            # shorthand: string → Selector { text }
   - tapOn:
       id: "username"
   - inputText: "podium"
   - assertVisible: "Welcome"
   - scrollUntilVisible:
       element: "Item 50"
   ```
   `${VAR}` substitution from a `HashMap<String, String>` env passed to the parser.
3. Errors via `thiserror`: unknown command → error naming the command and document
   position. Missing `appId` → error. Export a
   `#[uniffi::export] fn parse_flow(yaml: String, env: HashMap<String,String>) -> Result<Flow, PodiumError>`.

Acceptance criteria:
- ≥ 15 `cargo test` unit tests: shorthand vs map forms, regex text selectors
  (`/Item \d+/`), env substitution, unknown-command error, missing appId, two-document
  split, timeout defaults. All green.
- Commit: `phase-1: flow model + parser with rust unit tests`

---

## Phase 2 — Driver Trait & Engine (Rust orchestrates, Kotlin executes)

Implement in `core/src/driver.rs` + `engine.rs`:

1. The FFI seam — a foreign-implemented trait:
   ```rust
   #[uniffi::export(with_foreign)]
   pub trait Driver: Send + Sync {
     fn launch_app(&self, app_id: String, clear_state: bool) -> Result<(), DriverError>;
     fn is_visible(&self, selector: Selector) -> Result<bool, DriverError>;
     fn tap(&self, selector: Selector) -> Result<(), DriverError>;
     fn input_text(&self, text: String) -> Result<(), DriverError>;
     fn swipe(&self, direction: Direction) -> Result<(), DriverError>;
     fn back(&self) -> Result<(), DriverError>;
     fn wait_for_idle(&self, timeout_ms: u64) -> Result<(), DriverError>;
     fn take_screenshot(&self, name: String) -> Result<(), DriverError>;
     fn now_ms(&self) -> u64;      // clock via driver → engine stays testable
     fn sleep_ms(&self, ms: u64);  // ditto
   }
   ```
   Verify the exact UniFFI foreign-trait syntax against the version you pin —
   this API has evolved across releases; check the docs, don't trust memory.
2. `engine.rs`: `run_flow(flow: Flow, driver: Arc<dyn Driver>) -> FlowResult`.
   ALL Maestro-style semantics live here:
   - Implicit wait: every selector-based command polls `is_visible` (poll interval
     ~200 ms) up to `timeout_ms` (default 10 000, overridable via header
     `commandTimeout`). No sleeps in the driver.
   - `scroll_until_visible`: check → swipe → check loop, max_swipes cap.
   - Per-step timing captured via `driver.now_ms()`; produce
     `FlowResult { steps: Vec<StepResult>, passed: bool }` with
     `StepResult { command_desc, status, duration_ms, failure_message }`.
   - First failure aborts the flow (record remaining steps as skipped).
3. Engine unit tests with a `MockDriver` (plain Rust struct, scripted visibility
   timelines + a fake clock): verify retry-until-timeout behavior, scroll loop
   bounds, timing capture, abort-on-failure. This is where the correctness of the
   whole tool is pinned down — be thorough.
4. Export `#[uniffi::export] fn run_flow(...)` returning `FlowResult`.

Acceptance criteria:
- ≥ 12 engine tests against MockDriver, all green with `cargo test`.
- Bindings regenerate cleanly; runner module compiles against generated Kotlin.
- Commit: `phase-2: driver trait + engine with mock-driver tests`

---

## Phase 3 — Kotlin Driver & On-Device Runner

Implement in `android/runner`:

1. `UiAutomatorDriver.kt` implementing the generated `Driver` interface over
   `UiDevice`:
   - Selector → `BySelector`: `By.text` exact; regex when text is `/…/` (compile via
     `Pattern`); `By.res` with suffix match for ids; `instance`/index handling.
   - `is_visible` = `device.hasObject(selector)` — NO waiting here (engine owns it).
   - `launch_app`: launch intent + `FLAG_ACTIVITY_CLEAR_TASK`; `clear_state = true`
     → `device.executeShellCommand("pm clear <pkg>")` then relaunch.
   - `take_screenshot` → PNG under `context.getExternalFilesDir(null)/podium/screenshots/`.
   - `wait_for_idle` → `device.waitForIdle(timeout)`.
2. `FlowRunner.kt` — the single instrumentation entry point:
   ```kotlin
   @RunWith(AndroidJUnit4::class)
   class FlowRunner {
     @Test fun runFlows() { /* load flows → parse_flow → run_flow → report */ }
   }
   ```
   Flow delivery, support BOTH:
   - **Assets**: `assets/flows/*.yaml` baked into the APK (the cloud-farm path).
   - **Instrumentation args**: `-e flowsDir <device-path>` (and `-e flow <base64>`)
     read via `InstrumentationRegistry.getArguments()` — the fast local path, no
     rebuild needed when flows change. Args take precedence over assets.
3. Reporting, written on-device:
   - `podium/results/<flow>.json` — full `FlowResult` (serialize in Rust to JSON
     string via an exported helper, so the schema is owned by one side).
   - `podium/results/junit.xml` — one `<testcase>` per flow; failures embed the
     failing step + a hierarchy dump (`device.dumpWindowHierarchy`).
   - Emit machine-readable progress lines to logcat/instrumentation status prefixed
     `PODIUM|step|<n>|<status>|<duration_ms>|<desc>` for the CLI to stream.
   - Throw on any flow failure so the instrumentation itself reports red — cloud
     farm dashboards key off this.

Acceptance criteria (on emulator/device):
- `flows/login.yaml` (launch+clearState → fill username/password → tap "Log in" →
  assertVisible "Welcome") passes end-to-end via `am instrument`.
- `flows/smoke.yaml` including `scrollUntilVisible: "Item 50"` passes.
- A deliberately broken flow (assert on nonexistent text, 3 s timeout) fails in
  ~3 s with a useful message; junit.xml marks it failed; hierarchy dump present.
- Commit: `phase-3: uiautomator driver + on-device runner`

---

## Phase 4 — CLI (`podium`, Rust binary in `cli/`)

Developer experience target: `podium test flows/ --app sampleapp.apk`

Subcommands (use `clap` with derive; no other heavy deps):
1. `podium test <flow-file-or-dir>` — flags: `--app <apk>`, `--runner <apk>`
   (default: known build output path), `--serial <adb-serial>`, `--env KEY=VALUE`
   (repeatable), `--out <dir>` (default `./podium-out`).
   Pipeline: **validate flows locally by calling `podium_core::parse_flow` directly**
   (instant feedback on YAML errors, no device needed — this is the payoff of the
   shared Rust core) → `adb install -r` both APKs → push flows to
   `/data/local/tmp/podium/flows` → `adb shell am instrument -w -e flowsDir ...` →
   stream stdout live, rendering `PODIUM|` lines as `✅ tapOn "Log in" (412ms)` →
   `adb pull` results + screenshots → nonzero exit on any failure.
2. `podium validate <flow-file-or-dir>` — parse-only, no device. Free feature via
   the shared crate; useful as a pre-commit hook story.
3. `podium report <dir>` — pretty-print results JSON as a summary table.

Implementation notes:
- Shell out to `adb` (`std::process::Command`); do not implement the adb protocol.
- Unit-test the `PODIUM|` line parser and the env-flag parsing (5+ tests).

Acceptance criteria:
- `podium validate flows/` catches a seeded YAML error with file+reason.
- `podium test flows/ --app <path> --runner <path>` runs both sample flows, prints
  live per-step progress, pulls junit.xml + screenshots into `./podium-out`,
  exits 0. Broken flow → exits 1. `cargo clippy --workspace` clean.
- Commit: `phase-4: rust cli with local validation and live streaming`

---

## Phase 5 — Benchmark Harness (the thesis test)

1. `flows/bench.yaml`: a 50-step flow against the sample app (realistic
   tap/assert/scroll cycles, not no-ops).
2. `scripts/bench-local.sh`: run it 5× via `podium test`, extract per-step timings
   from results JSON, print mean/median/p95 total wall time and per-command means.
3. `BENCHMARK.md`: table of measured results on the local emulator, plus a
   "Comparison methodology" section describing how the human can run the same flow
   through Maestro CLI and maestro-runner (Appium driver) for apples-to-apples
   numbers. Record ONLY what was actually measured in this environment — never
   fabricate or extrapolate comparison numbers.

Acceptance criteria: `scripts/bench-local.sh` produces the stats table from real
runs. Commit: `phase-5: benchmark flow, harness, BENCHMARK.md`

---

## Phase 6 — Docs & Polish

1. README.md: problem statement (3 paragraphs max), ASCII architecture diagram
   showing the Rust-core/Kotlin-driver split, quickstart (prereqs → build → run),
   reference for the 10 commands with YAML examples, known limitations (list the
   non-goals from section 0 explicitly), and a short "porting to iOS" note
   (Swift `Driver` over XCUITest, same engine).
2. `scripts/run-local.sh` = one-command demo on a fresh emulator.
3. Sweep: `cargo fmt` + `clippy` clean, ktlint on Kotlin, no dead code, stray
   ideas moved to `docs/future.md`.

Acceptance criteria: a newcomer goes clone → green demo following only the README.
Commit: `phase-6: docs and polish`

---

## Phase 7 — Cloud Upload Adapters (OPTIONAL — gated on credentials)

Do not start unless the human provides `BROWSERSTACK_USERNAME`/`ACCESS_KEY` or Sauce
credentials. Until then, deliver only:

1. `scripts/upload-browserstack.sh`: documented curl calls against BrowserStack App
   Automate **Espresso** endpoints (app upload, espresso test-suite upload, build
   trigger with `devices` + `app` + `testSuite`). The runner APK with flows baked
   into assets IS the test suite — that is the whole point. Verify current endpoint
   paths against BrowserStack's live docs at implementation time; do not trust
   memorized URLs. Note in comments: the uploaded runner must contain the arm64 .so.
2. `docs/cloud.md`: equivalent notes for Sauce Labs (`saucectl` espresso config).
3. If credentials ARE provided: run `flows/bench.yaml` on one BrowserStack device,
   pull timing JSON from session artifacts, append real numbers to BENCHMARK.md.

Commit: `phase-7: cloud upload scripts and docs`

---

## Working Agreements for the Agent

- **Verify, don't assume — especially UniFFI.** UniFFI's proc-macro and foreign-trait
  APIs have changed across versions. Pin one version in Phase 0, read its actual docs
  (docs.rs / mozilla.github.io/uniffi-rs), and validate with the walking skeleton
  before building on it. Same for cargo-ndk flags and the JNA `@aar` dependency.
- **The build script is a first-class deliverable.** If `scripts/build-runner.sh`
  is flaky or order-dependent, fix that before adding features.
- **Small commits per phase**, messages as specified. Never commit failing tests.
- **When blocked** (NDK missing, linker errors, emulator won't boot, UniFFI version
  mismatch): describe the blocker + what you tried + 2 options, then ask the human.
  Do not silently substitute a different architecture (e.g. do not fall back to
  writing the interpreter in Kotlin).
- **Scope discipline.** Any "wouldn't it be nice" goes into `docs/future.md`, not
  the codebase. The PoC's value is the benchmark number and the single-artifact
  local/cloud story — nothing else.
- **FFI hygiene.** Anything crossing the boundary stays UniFFI-lowerable: owned
  types, `Option`/`Vec`/`HashMap`, no lifetimes or generics. Chatty calls are fine
  (`is_visible` polling every 200 ms is ~5 FFI calls/sec — negligible), but never
  pass large blobs (like hierarchy XML) across per poll.

## Definition of Done (whole PoC)

1. One artifact (runner APK: Rust core .so + Kotlin driver + baked flows) runs
   identically via local adb and — per documented scripts — via any farm's Espresso
   endpoint.
2. `podium test` gives Maestro-like local DX with live step output;
   `podium validate` catches flow errors with no device attached.
3. Engine semantics are pinned by Rust unit tests against a MockDriver, proving the
   core is portable to an iOS driver later.
4. BENCHMARK.md contains real measured per-step and total timings for a 50-step flow.
5. README lets a stranger reproduce everything.
