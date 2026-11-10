class SwitchRequest {
    snapshot: Snapshot,
    request: string,
    prediction: bool,
    activity: ActivityData,
    record: bool,
}

class Recorded {
    recorded: bool,
}

/// baml:$trace=decision_trace
function decide(args: SwitchRequest) -> Selection {
    if (args.request.length() > 2048) {
        throw baml.errors.InvalidArgument { message: "Please use a shorter app request" };
    }
    let context = enrich_snapshot(args.snapshot, args.activity);
    let span = if (recording_allowed()) {
        trace.start_span(
            "smart_switch.decision",
            inputs = { "snapshot": context, "request": args.request, "prediction": args.prediction },
        )
    } else {
        null
    };
    defer { if (span != null) { span.close(); } }
    let choice = if (args.prediction) {
        predict_workflow(context)
    } else {
        select_app(context, args.request)
    };
    let selection = if (choice.bundle_id != null) {
        Selection { ...choice, reason: selection_feedback(choice, context, args.request) }
    } else {
        choice
    };
    if (span != null) {
        span.finish(selection);
    }
    selection
}
