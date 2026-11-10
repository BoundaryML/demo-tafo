class SwitchTiming {
    first_activation_ms: int?,
    settled_activation_ms: int?,
    fast: bool,
}

class SwitchEvent {
    first_activation_ms: int?,
    settled_activation_ms: int?,
    fast: bool?,
    learning: PairLearning?,
    event_id: string,
    started_at: int,
    ended_at: int,
    origin: string,
    initial_target: string,
    final_target: string,
    candidates: string[],
    advances: int,
    source: string,
}

/// baml:$trace=switch_trace
function record_switch(event: SwitchEvent) -> Recorded {
    if (!recording_allowed()) {
        return Recorded { recorded: false };
    }
    let span = trace.start_span("smart_switch.switch", inputs = { "event": event });
    span.finish(event);
    Recorded { recorded: true }
}
