# Podium Benchmark Results

## Summary

50-step flow (`flows/bench.yaml`) measured on a local Android emulator, 5 runs.

| Metric | Step time | Wall time |
|--------|-----------|-----------|
| Mean   | 33,865 ms | 34,892 ms |
| Median | 34,114 ms | 34,759 ms |
| p95    | 34,653 ms | 35,853 ms |

Step time = sum of `duration_ms` across all 50 steps from result JSON.  
Wall time = total elapsed time including APK install, push, instrumentation startup, and result pull.

## Environment

| Field | Value |
|-------|-------|
| Device | sdk_gphone64_arm64 (Android Emulator) |
| Android | 15 (API 35) |
| ABI | arm64-v8a |
| Run date | 2026-07-12 |
| Runs | 5 |

## Raw Run Data

| Run | Step time (ms) | Wall time (ms) |
|-----|---------------|----------------|
| 1   | 35,219        | 34,759         |
| 2   | 33,079        | 34,533         |
| 3   | 32,261        | 33,750         |
| 4   | 34,653        | 35,853         |
| 5   | 34,114        | 35,567         |

## Per-Command Breakdown (Run 1)

| # | Command | ms |
|---|---------|-----|
| 1 | launchApp(clearState: true) | 1,979 |
| 2 | assertVisible(username) | 12 |
| 3 | tapOn(username) | 144 |
| 4 | inputText("podium") | 1,320 |
| 5 | assertVisible(username) | 88 |
| 6 | tapOn(password) | 1,293 |
| 7 | inputText("pass") | 1,114 |
| 8 | assertVisible(password) | 82 |
| 9 | tapOn(login_button) | 737 |
| 10 | assertVisible("Welcome") | 1,100 |
| 11-17 | assertVisible + scrollUntilVisible × 3 | ~25 |
| 18 | scrollUntilVisible("Item 20") | 2,273 |
| 19-21 | assertVisible/scroll × 2 | ~5 |
| 22 | scrollUntilVisible("Item 30") | 2,276 |
| 23-25 | assertVisible/scroll × 2 | ~12 |
| 26 | scrollUntilVisible("Item 40") | 2,882 |
| 27-30 | assertVisible/scroll × 3 | ~16 |
| 31 | tapOn("Item 50") | 227 |
| 32 | assertVisible("Reached end") | 409 |
| 33-37 | swipe(Up) × 5 | 9,128 |
| 38 | assertVisible("Item 5") | 242 |
| 39-47 | scrollUntilVisible + assert × 4 | ~8,019 |
| 48-50 | assertVisible/tap/assert | 543 |

Key observations:
- **`scrollUntilVisible` dominates** — each scroll page takes ~2,000–5,000ms because it waits for the scroll animation (300ms/swipe × multiple swipes).
- **Assert-visible on already-visible elements** is near-zero (1–12ms) — the Rust engine polls at 200ms intervals but finds the element on the first check.
- **`launchApp` with `clearState`** takes ~1,900ms (pm clear + monkey + waitForIdle).
- **`inputText`** takes ~700–1,300ms because UIAutomator types each character individually via `focused.text = value`.

## Comparison Methodology

To compare Podium against Maestro CLI or an Appium-based runner, run the same flow through each tool and record total wall time and per-step timings.

**Maestro CLI comparison:**
1. Install Maestro: `curl -Ls "https://get.maestro.mobile.dev" | bash`
2. Translate `flows/bench.yaml` to Maestro format (the YAML dialect is intentionally compatible).
3. Run: `maestro test flows/bench.yaml` and note the reported total time.

**Appium/WebDriver Agent comparison:**
1. Start the Appium server and connect it to the same emulator.
2. Implement the same 50-step flow as a WebDriver script.
3. Measure total wall time — note that every `findElement` + `click` call incurs an HTTP round-trip even on a local emulator.

**Thesis:** Podium's interpreter loop runs on-device, making element checks and scroll decisions in-process. The latency budget is entirely UIAutomator execution time (animation + element search) with no network hop. An Appium-based runner on a remote farm adds WAN round-trips per command; on a local emulator it still adds localhost HTTP overhead.

Numbers above reflect only real measured values on this emulator. Remote-farm timing would differ.

## Run: 2026-07-12 11:36 UTC

**Device:** sdk_gphone64_arm64
**Android:** 15
**Flow:** flows/bench.yaml (50 steps)
**Runs:** 5

| Metric | Value |
|--------|-------|
| Mean   | 34760ms |
| Median | 34292ms |
| p95    | 35955ms |
| Run times | 34062,34292,33650,35955,35844 ms |

