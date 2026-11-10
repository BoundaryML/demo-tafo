class TrackingState {
    enabled: bool,
    paused_until: int?,
}

enum TrackingAction {
    FifteenMinutes,
    OneHour,
    Today,
    Pause,
    Resume,
    Tick,
}

function tracking_action(value: string) -> TrackingAction throws baml.errors.InvalidArgument {
    match (value) {
        "15m" => TrackingAction.FifteenMinutes,
        "1h" => TrackingAction.OneHour,
        "today" => TrackingAction.Today,
        "pause" => TrackingAction.Pause,
        "resume" => TrackingAction.Resume,
        "tick" => TrackingAction.Tick,
        _ => throw baml.errors.InvalidArgument { message: `Unknown tracking action: ${value}` },
    }
}

class TrackingData {
    state: TrackingState,
    action: TrackingAction,
    at: int,
    tomorrow: int,
}

function update_tracking(data: TrackingData) -> TrackingState throws never {
    match (data.action) {
        TrackingAction.Resume => TrackingState { enabled: true, paused_until: null },
        TrackingAction.Pause => TrackingState { enabled: false, paused_until: null },
        TrackingAction.FifteenMinutes => {
            TrackingState { enabled: false, paused_until: data.at + 900 }
        },
        TrackingAction.OneHour => TrackingState { enabled: false, paused_until: data.at + 3600 },
        TrackingAction.Today => TrackingState { enabled: false, paused_until: data.tomorrow },
        TrackingAction.Tick => {
            let until = data.state.paused_until;
            if (!data.state.enabled && until != null && data.at >= until) {
                TrackingState { enabled: true, paused_until: null }
            } else {
                data.state
            }
        },
    }
}

test "timed pauses resume at their deadline and manual pauses remain paused" {
    let running = TrackingState { enabled: true, paused_until: null };
    let paused = update_tracking(
        TrackingData {
            state: running,
            action: TrackingAction.FifteenMinutes,
            at: 1000,
            tomorrow: 86400,
        },
    );
    assert.equal(paused.paused_until, 1900);
    assert.equal(
        update_tracking(TrackingData { state: paused, action: TrackingAction.Tick, at: 1899, tomorrow: 86400 }).enabled,
        false,
    );
    assert.equal(
        update_tracking(TrackingData { state: paused, action: TrackingAction.Tick, at: 1900, tomorrow: 86400 }).enabled,
        true,
    );
    let manual = update_tracking(TrackingData { state: running, action: TrackingAction.Pause, at: 1000, tomorrow: 86400 });
    assert.equal(
        update_tracking(TrackingData { state: manual, action: TrackingAction.Tick, at: 999999, tomorrow: 1000000 }).enabled,
        false,
    );
    assert.equal(
        update_tracking(TrackingData { state: manual, action: TrackingAction.Resume, at: 1001, tomorrow: 86400 }).enabled,
        true,
    )
}

test "one hour and rest of day use explicit clock boundaries" {
    let running = TrackingState { enabled: true, paused_until: null };
    assert.equal(
        update_tracking(TrackingData { state: running, action: TrackingAction.OneHour, at: 1000, tomorrow: 86400 }).paused_until,
        4600,
    );
    assert.equal(
        update_tracking(TrackingData { state: running, action: TrackingAction.Today, at: 1000, tomorrow: 86400 }).paused_until,
        86400,
    )
}
