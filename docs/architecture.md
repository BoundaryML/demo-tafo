# Language boundaries

BAML owns matching, ranking, inference, learning, cycling, workflow grouping,
activity accounting, retention, decision records, dashboard aggregates, and trace
policy. Rust owns macOS APIs, AppKit, physical input/animation timing, private
filesystem access, calendar boundaries and the native control socket.

## Generated Rust SDK

`baml.toml` pins the toolchain and declares the Rust generator. `baml generate`
produces the ignored `baml_sdk/` path dependency. Its native types, Serde support
and generated sync/async calls replace handwritten mirrors and JSON message
adapters. Rust 1.91.1 is pinned in `rust-toolchain.toml` to match the SDK minimum.

No BAML executable is spawned. The generated SDK invokes the bundled, matching
BAML shared library in process. The build script obtains it through the official
bridge loader and places it in the app's Frameworks directory; startup selects
that copy before SDK initialization. Runtime execution does not require a compiler,
shell profile, package download or external BAML service.

Inference stays on worker threads. Frequent capture calls invoke deterministic
BAML directly, with just the current visit and OS measurements. Retention passes
timestamp metadata and receives indices. The global keyboard callback never
invokes BAML. Native Activity and History wrappers own private disk storage and
the transient capture cursor; their nested data types come from BAML.

## Scoped tracing

BAML declared hooks use `trace.context` to attach recording eligibility and app
metadata to each decision, dashboard and settled-cycle invocation. Descendants
read `trace.current_context`; they do not share a mutable global tracking flag.
Paused/offline operations hide their spans. Only eligible operations create the
named Cloud spans used by the existing dashboard. Live builds use the bridge's regular recording mode, preserving built-in HTTP timing
spans. HTTP body capture remains disabled. An already
in-flight prediction retains its original context and may finish after pause.

Trace policy tests include a separate SDK process with hooks enabled, synthetic
inputs and a localhost collector address. Normal tests run with telemetry off.
The native-smoke feature alone admits deterministic BAML selection fixtures;
production builds do not read that test switch.

## Rust audit

Every authored Rust function and native ABI declaration has a specific rationale
in `rust-boundaries.json`. `scripts/check_rust_boundaries.py` rejects missing or
stale entries and runs in the standard test command. Generated SDK implementation
is excluded: its source of truth is BAML. Coverage does not prove a rationale is
sound; reviewers should continue moving portable behavior into BAML.


## Source layout

- `baml_src/switching/`: matching, model calls, feedback, learning, and cycling.
- `baml_src/activity/`: capture, retention, inventory, and workflow grouping.
- `baml_src/insights/`: dashboard, performance, and cost/value calculations.
- `baml_src/telemetry.baml`: declared hooks and scoped tracing context.
- `baml_src/text.baml`: shared whitespace normalization.

These folders do not introduce BAML namespaces. The generated API remains shared
with the native host. The cost-scenario constructor lives on `SwitchValue`, and
feedback uses a source match instead of a nested sequence of early branches.
