//! Exercise declared hooks with telemetry enabled in a separate process. The
//! collector points at localhost, with synthetic inputs and no user credentials.
use tafo::sdk;

#[test]
fn declared_trace_hooks_apply_scoped_context() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "trace_hook_child", "--nocapture"])
        .env("SMART_SWITCH_TRACE_HOOK_CHILD", "1")
        .env("BAML_TELEMETRY", "medium")
        .env("BOUNDARY_API_URL", "http://127.0.0.1:9")
        .env("BOUNDARY_API_KEY", "local-test")
        .env("BOUNDARY_PROJECT", "local-test/trace-hooks")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn trace_hook_child() {
    if std::env::var_os("SMART_SWITCH_TRACE_HOOK_CHILD").is_none() {
        return;
    }
    let snapshot = sdk::Snapshot::new(
        "editor".into(),
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    );
    let activity = sdk::ActivityData::new(true, vec![], vec![], vec![]);
    let mut args = sdk::SwitchRequest::new(snapshot, "".into(), false, activity, true);
    let first = sdk::native_trace_context_probe_sync(&args).unwrap();
    assert!(matches!(
        first.metadata.get("tracking_enabled"),
        Some(sdk::trace::StringOrIntOrFloatOrBool::Bool(true))
    ));
    args.activity.enabled = false;
    let paused = sdk::native_trace_context_probe_sync(&args).unwrap();
    assert!(matches!(
        paused.metadata.get("tracking_enabled"),
        Some(sdk::trace::StringOrIntOrFloatOrBool::Bool(false))
    ));
    let ambient = sdk::trace::current_context_sync().unwrap();
    assert!(ambient.metadata.get("tracking_enabled").is_none());
}
