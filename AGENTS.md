# Implementation boundaries

Read `.agents/skills/baml-core/SKILL.md` before changing BAML.

Keep portable application behavior in BAML. Rust is the macOS host: native APIs,
rendering, physical input/animation timing, secure filesystem access, and the native
control socket. Shared types and BAML invocations must use the generated Rust SDK;
do not introduce handwritten BAML JSON protocols or subprocess adapters. Do not add ranking, learning, activity accounting, retention, workflow,
or decision policy to Rust.

Every Rust function and imported native ABI declaration must have a specific,
reviewable reason in `docs/rust-boundaries.json`. Update it when adding, removing,
or changing responsibilities. A generic claim that something is "native" is not
sufficient. Run `python3 scripts/check_rust_boundaries.py`; it is also part of
`scripts/test.sh`. Prefer removing a function or moving behavior into BAML over
inventing a justification.

Run `scripts/test.sh`, `cargo clippy --all-targets -- -D warnings`, and
`baml codequality --functions` after a behavior migration. Preserve private local
state and keep model credentials out of source, traces, and test fixtures.
