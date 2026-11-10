function capture_fixture() -> CaptureData throws never {
    CaptureData {
        enabled: true,
        previous: null,
        previous_day: null,
        day: 0,
        app: CapturedApp { id: "editor", name: "Editor" },
        at: 100,
        delta_ms: 200,
        idle_seconds: 0.0,
    }
}

test "capture charges the previous app before starting a new visit" {
    let data = capture_fixture();
    let first = capture_observation(data);
    assert.equal(first.append?.active_ms, 0);
    data.previous = first.append;
    data.previous_day = 0;
    data.app = CapturedApp { id: "browser", name: "Browser" };
    data.at = 101;
    data.delta_ms = 1000;
    let changed = capture_observation(data);
    assert.equal(changed.previous?.bundle_id, "editor");
    assert.equal(changed.previous?.active_ms, 1000);
    assert.equal(changed.append?.bundle_id, "browser");
    assert.equal(changed.append?.active_ms, 0);
    assert.equal(changed.keep_current, false)
}

test "idle sleep paused and invalid measurements never charge time" {
    for (let idle in [null, -1.0, 120.0, 1000.0]) {
        let data = capture_fixture();
        data.previous
            = AppVisit {
                bundle_id: "editor",
                name: "Editor",
                started: 90,
                ended: 99,
                active_ms: 1000,
            };
        data.idle_seconds = idle;
        let result = capture_observation(data);
        assert.equal(result.previous, null);
        assert.equal(result.append, null);
        assert.equal(result.keep_current, false);
    }
    let data = capture_fixture();
    data.delta_ms = 2001;
    assert.equal(capture_observation(data).append, null);
    data.delta_ms = 2000;
    assert.equal(capture_observation(data).append != null, true);
    data.enabled = false;
    assert.equal(capture_observation(data).append, null)
}

test "restart and local midnight create fresh visits without charging closed time" {
    let data = capture_fixture();
    data.previous
        = AppVisit { bundle_id: "editor", name: "Editor", started: 90, ended: 99, active_ms: 1000 };
    data.previous_day = 0;
    let same = capture_observation(data);
    assert.equal(same.keep_current, true);
    assert.equal(same.append, null);
    data.day = 86400;
    let midnight = capture_observation(data);
    assert.equal(midnight.append != null, true);
    assert.equal(midnight.previous?.active_ms, 1400);
    data.previous = null;
    let restarted = capture_observation(data);
    assert.equal(restarted.previous, null);
    assert.equal(restarted.append?.active_ms, 0)
}

test "retention applies age and volume limits and preserves cutoff equality" {
    let plan = retention_plan(
        RetentionData {
            at: RETENTION_SECONDS + 100,
            visits: [99, 100, 101],
            decisions: [10, 100],
            workflow_notes: [100, 10, 101],
            switches: [1, 100, 101],
        },
    );
    assert.equal(plan.visits_drop, 1);
    assert.equal(plan.decisions_drop, 1);
    assert.equal(plan.workflow_notes_keep, [0, 2]);
    assert.equal(plan.switches_keep, [1, 2]);
    assert.equal(expired_prefix(baml.Array.filled(MAX_VISITS + 3, 100), 0, MAX_VISITS), 3);
    assert.equal(expired_prefix([], 100, MAX_DECISIONS), 0);
    assert.equal(expired_prefix([1, 2], 100, MAX_DECISIONS), 2);
    let many = retention_plan(
        RetentionData {
            at: 100,
            visits: [],
            decisions: [],
            workflow_notes: [],
            switches: baml.Array.filled(MAX_DECISIONS + 2, 100),
        },
    );
    assert.equal(many.switches_keep.length(), MAX_DECISIONS);
    assert.equal(many.switches_keep[0], 2)
}

test "activation history ignores empty duplicate events and retains its newest bound" {
    assert.equal(record_activation(ActivationData { activations: ["a"], id: "a" }), ["a"]);
    assert.equal(record_activation(ActivationData { activations: ["a"], id: "" }), ["a"]);
    let history = baml.Array.generate(MAX_ACTIVATIONS, (n) -> {
        `${n}`
    });
    let next = record_activation(ActivationData { activations: history, id: "new" });
    assert.equal(next.length(), MAX_ACTIVATIONS);
    assert.equal(next[0], "1");
    assert.equal(next[MAX_ACTIVATIONS - 1], "new")
}

test "workflow updates replace one visit and paused tracking rejects new notes" {
    let data = WorkflowUpdate {
        enabled: true,
        notes: [],
        visit_started: 100,
        guess: WorkflowGuess { label: "building", confidence: 0.9 },
    };
    data.notes = record_workflow_note(data);
    data.guess = WorkflowGuess { label: "research", confidence: 0.8 };
    let updated = record_workflow_note(data);
    assert.equal(updated.length(), 1);
    assert.equal(updated[0].guess.label, "research");
    data.enabled = false;
    data.visit_started = 101;
    assert.equal(record_workflow_note(data).length(), 1)
}

test "decision records use observed names and paused tracking rejects records" {
    let context = fixture();
    let choice = select_app(context, "browser");
    let data = DecisionData { enabled: true, at: 100, context: context, choice: choice, reason: "Chosen" };
    assert.equal(decision_record(data)?.reason, "Chosen");
    assert.equal(decision_record(data)?.from, "Cursor");
    data.enabled = false;
    assert.equal(decision_record(data), null);
    data.enabled = true;
    choice.bundle_id = null;
    assert.equal(decision_record(data), null)
}
