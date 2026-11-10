// Observed acceptance is behavioral evidence, not calibrated model accuracy.
class PairLearning {
    observations: int,
    accepted: int,
    corrected: int,
    confidence: float,
    ready: bool,
}

function pair_evidence(
    events: SwitchEvent[],
    origin: string,
    target: string,
) -> PairLearning throws never {
    let trials = events
        .filter((event) -> {
            event.origin == origin
                && event.initial_target == target
                && (event.advances == 0 || event.final_target != target)
        })
        .slice(0, 8);
    let accepted = trials
        .filter((event) -> {
            event.final_target == target && event.advances == 0
        })
        .length();
    let indices = baml.Array.generate(trials.length(), (i) -> {
        i
    });
    let weight = indices.reduce(
        (total, i) -> {
            total + (8 - i) / 8.0
        },
        0.0,
    );
    let accepted_weight = indices.reduce(
        (total, i) -> {
            let event = trials[i];
            total
                + if (event.final_target == target && event.advances == 0) {
                    (8 - i) / 8.0
                } else {
                    0.0
                }
        },
        0.0,
    );
    let confidence = (accepted_weight + 1.0) / (weight + 2.0);
    let recent = trials.slice(0, 3);
    let consistent = recent.length() == 3
        && recent.every((event) -> {
            event.final_target == target && event.advances == 0
        });
    PairLearning {
        observations: trials.length(),
        accepted: accepted,
        corrected: trials.length() - accepted,
        confidence: confidence,
        ready: accepted >= 3 && confidence >= 0.70 && consistent,
    }
}

function pair_learning(snapshot: Snapshot, target: string) -> PairLearning throws never {
    pair_evidence(snapshot.recent_switches, snapshot.current_app_id, target)
}

function learned_event(
    at: int,
    final: string = "browser",
    advances: int = 0,
) -> SwitchEvent throws never {
    SwitchEvent {
        first_activation_ms: null,
        settled_activation_ms: null,
        fast: null,
        learning: null,
        event_id: at.to_string(),
        started_at: at,
        ended_at: at + 1,
        origin: "editor",
        initial_target: "browser",
        final_target: final,
        candidates: ["browser", "slack"],
        advances: advances,
        source: "history",
    }
}

test "accepted choices progressively build learned confidence without a model call" {
    let context = fixture();
    let previous = pair_learning(context, "browser").confidence;
    assert.equal(previous, 0.5);
    for (let i in [1, 2, 3]) {
        context.recent_switches.unshift(learned_event(i));
        let evidence = pair_learning(context, "browser");
        assert.equal(evidence.confidence > previous, true);
        previous = evidence.confidence;
        assert.equal(predict(context).fast, i >= 3);
    }
    assert.equal(predict(context).learning?.accepted, 3)
}

test "a correction drops learned confidence and fresh consistent acceptances restore it" {
    let context = fixture();
    context.recent_switches = [learned_event(3), learned_event(2), learned_event(1)];
    assert.equal(predict(context).fast, true);
    context.recent_switches.unshift(learned_event(4, final = "slack", advances = 1));
    assert.equal(pair_learning(context, "browser").ready, false);
    assert.equal(pair_learning(context, "browser").confidence < 0.8, true);
    for (let i in [5, 6, 7]) {
        context.recent_switches.unshift(learned_event(i));
    }
    assert.equal(pair_learning(context, "browser").ready, true)
}

test "exploring back to the initial app is not counted as an accepted prediction" {
    let context = fixture();
    context.recent_switches = [learned_event(1, advances = 19)];
    assert.equal(pair_learning(context, "browser").observations, 0);
    assert.equal(predict(context).fast, false)
}
