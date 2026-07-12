# Future Work

Ideas considered during the PoC that are explicitly out of scope. Parked here rather than as GitHub issues to avoid polluting the backlog with premature plans.

## iOS Driver

Compile `podium-core` as a universal static library for iOS targets and generate Swift bindings with `uniffi-bindgen --language swift`. Implement `Driver` over `XCUITest`. The engine and all retry semantics require zero changes — that's the point of the UniFFI FFI boundary.

## JavaScript / Kotlin Step Scripting

Allow individual steps to be arbitrary JS or Kotlin lambdas for cases where the 10 built-in commands don't cover a use-case. Requires embedding a JS engine (e.g. QuickJS via Rust) or a Kotlin scripting runtime, and versioning the driver contract carefully.

## Parallel Flow Execution

Run independent flows in parallel on a multi-device farm. The main constraint is that each device gets its own `UiAutomatorDriver` instance — the Rust engine already accepts an `Arc<dyn Driver>`, so concurrency is a CLI/orchestration concern, not an engine concern.

## `runFlow` Composition

Allow flows to call other flows: `- runFlow: flows/setup.yaml`. Requires cycle detection in the parser and a stack-depth limit.

## Rich Report Dashboard

Generate a self-contained HTML report from the result JSON files, including screenshots and a step-by-step timeline. The JSON schema is already complete.

## Appium-Compatible Mode

Accept full Appium YAML (not just the Maestro subset). Requires expanding the command set and selector model significantly.

## Cloud Credentials Integration

Auto-authenticate to BrowserStack / Sauce Labs and trigger runs from `podium test --cloud browserstack`. Gated on API credentials from the user.

## Hierarchy Matching in Rust

Move the element-lookup logic from UIAutomator into the Rust engine by passing the full accessibility tree as a serialized snapshot. Would enable offline replay and stronger cross-platform guarantees, at the cost of significant complexity.
