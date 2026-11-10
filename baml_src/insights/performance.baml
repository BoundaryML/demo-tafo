class PairProgress {
    origin: string,
    target: string,
    confidence: float,
    observations: int,
    accepted: int,
    corrected: int,
    ready: bool,
}

class SwitchPerformance {
    measured: int,
    first_p50_ms: int?,
    first_p95_ms: int?,
    settled_p50_ms: int?,
    fast_switches: int,
    preview_switches: int,
    fast_percent: float?,
    changed_percent: float?,
    learning: PairProgress[],
}

// Nearest-rank percentiles. Unknown historical timings remain unknown.
function switch_percentile(values: int[], percent: int) -> int? throws never {
    if (values.length() == 0) {
        return null;
    }
    let sorted = values.sort_by_key((value) -> {
        value
    });
    sorted.at((sorted.length() * percent + 99) / 100 - 1)
}

function switch_performance(events: SwitchEvent[], all_events: SwitchEvent[]) -> SwitchPerformance {
    let first = events.filter_map((event) -> {
        event.first_activation_ms
    });
    let settled = events.filter_map((event) -> {
        event.settled_activation_ms
    });
    let known = events.filter((event) -> {
        event.fast != null
    });
    let fast = known
        .filter((event) -> {
            event.fast == true
        })
        .length();
    let changed = events
        .filter((event) -> {
            event.initial_target != event.final_target
        })
        .length();
    let recent = all_events.reverse().slice(0, 16);
    let pairs: map<string, PairProgress> = {};
    for (let event in recent) {
        let key = baml.json.stringify(RouteKey { from: event.origin, to: event.initial_target });
        if (!pairs.has(key)) {
            let evidence = pair_evidence(recent, event.origin, event.initial_target);
            if (evidence.observations > 0) {
                pairs.set(
                    key,
                    PairProgress {
                        origin: event.origin,
                        target: event.initial_target,
                        confidence: evidence.confidence,
                        observations: evidence.observations,
                        accepted: evidence.accepted,
                        corrected: evidence.corrected,
                        ready: evidence.ready,
                    },
                );
            }
        }
    }
    SwitchPerformance {
        measured: first.length(),
        first_p50_ms: switch_percentile(first, 50),
        first_p95_ms: switch_percentile(first, 95),
        settled_p50_ms: switch_percentile(settled, 50),
        fast_switches: fast,
        preview_switches: known.length() - fast,
        fast_percent: if (known.length() > 0) {
            fast * 100.0 / known.length()
        } else {
            null
        },
        changed_percent: if (events.length() > 0) {
            changed * 100.0 / events.length()
        } else {
            null
        },
        learning: pairs.values()
            .sort_by_key((pair) -> {
                0 - pair.observations
            })
            .slice(0, 4),
    }
}

test "performance separates fast requests preview delay and missing historical timing" {
    let fast = SwitchEvent {
        ...learned_event(1),
        fast: true,
        first_activation_ms: 80,
        settled_activation_ms: 80,
    };
    let preview = SwitchEvent {
        ...learned_event(2),
        fast: false,
        first_activation_ms: 1680,
        settled_activation_ms: 1680,
    };
    let corrected = SwitchEvent {
        ...learned_event(3, final = "slack", advances = 1),
        fast: true,
        first_activation_ms: 90,
        settled_activation_ms: 1750,
    };
    let events = [learned_event(0), fast, preview, corrected];
    let result = switch_performance(events, events);
    assert.equal(result.measured, 3);
    assert.equal(result.first_p50_ms, 90);
    assert.equal(result.first_p95_ms, 1680);
    assert.equal(result.settled_p50_ms, 1680);
    assert.equal(result.fast_switches, 2);
    assert.equal(result.preview_switches, 1);
    assert.equal(result.changed_percent, 25.0);
    assert.equal((result.fast_percent ?? 0.0) > 66.0 && (result.fast_percent ?? 0.0) < 67.0, true)
}

test "old switches do not gain invented latency or fast-path coverage" {
    let events = [learned_event(1)];
    let result = switch_performance(events, events);
    assert.equal(result.measured, 0);
    assert.equal(result.first_p50_ms, null);
    assert.equal(result.fast_percent, null)
}
