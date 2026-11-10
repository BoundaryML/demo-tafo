// Context is scoped to each invocation and inherited by its descendants, so an
// overlapping prediction cannot change the recording gate of a paused action.
function decision_trace(
    args: SwitchRequest,
    settings: trace.Settings,
) -> trace.Options throws never {
    trace.context(
        metadata = {
            "app": "smart-switch",
            "tracking_enabled": args.record && args.activity.enabled,
            "prediction": args.prediction,
        },
    )
}

function dashboard_trace(
    data: DashboardData,
    record: bool,
    settings: trace.Settings,
) -> trace.Options throws never {
    trace.context(
        metadata = { "app": "smart-switch", "tracking_enabled": record && data.activity.enabled },
    )
}

function cycle_trace(
    at: int,
    timing: SwitchTiming?,
    record: bool,
    settings: trace.Settings,
) -> trace.Options throws never {
    trace.context(
        metadata = { "app": "smart-switch", "tracking_enabled": record, "event_id": CYCLE.plan?.event_id },
    )
}

function app_trace(settings: trace.Settings) -> trace.Options throws never {
    if (settings.context.metadata.get("tracking_enabled") == true) {
        trace.options()
    } else {
        trace.hidden()
    }
}

function recording_allowed() -> bool throws never {
    trace.current_context().metadata.get("tracking_enabled") == true
}

test "trace policy gates capture using scoped tracking context" {
    let args = SwitchRequest {
        snapshot: fixture(),
        request: "",
        prediction: false,
        activity: ActivityData { enabled: true, visits: [], workflow_notes: [], switches: [] },
        record: true,
    };
    let allowed = decision_trace(args, trace.options().inspect()).inspect();
    assert.equal(allowed.context.metadata.get("tracking_enabled"), true);
    args.activity.enabled = false;
    let paused = decision_trace(args, trace.options().inspect()).inspect();
    assert.equal(paused.context.metadata.get("tracking_enabled"), false);
    assert.equal(app_trace(paused).inspect().mode, trace.Mode.Hidden);
    args.activity.enabled = true;
    args.record = false;
    assert.equal(
        decision_trace(args, trace.options().inspect()).inspect().context.metadata.get(
            "tracking_enabled",
        ),
        false,
    );
    // A separate context patch cannot mutate an earlier invocation's options.
    assert.equal(allowed.context.metadata.get("tracking_enabled"), true)
}

function switch_trace(event: SwitchEvent, settings: trace.Settings) -> trace.Options throws never {
    app_trace(settings)
}

test "trace context is scoped and inherited without leaking to the next invocation" {
    let allowed = (
        () -> {
            recording_allowed()
        }
    )($trace = trace.context(metadata = { "tracking_enabled": true }));
    let paused = (
        () -> {
            recording_allowed()
        }
    )($trace = trace.context(metadata = { "tracking_enabled": false }));
    assert.equal(allowed, true);
    assert.equal(paused, false);
    assert.equal(recording_allowed(), false)
}
