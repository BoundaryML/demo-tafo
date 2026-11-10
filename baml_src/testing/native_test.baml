// Called only by the native-smoke feature build. Production hosts do not read
// the smoke-mode environment or replace user predictions with this fixture.
function native_smoke_choice(snapshot: Snapshot, fast: bool) -> Selection throws never {
    let choice = predict(snapshot);
    Selection { ...choice, fast: fast }
}

/// baml:$trace=decision_trace
function native_trace_context_probe(args: SwitchRequest) -> trace.Context throws never {
    trace.current_context()
}
