function test_visit(id: string, at: int, ms: int) -> AppVisit throws never {
    AppVisit { bundle_id: id, name: id, started: at, ended: at + ms / 1000, active_ms: ms }
}

function test_activity() -> ActivityData throws never {
    ActivityData {
        enabled: true,
        visits: [
            test_visit("editor", 1000, 60000),
            test_visit("slack", 1060, 10000),
            test_visit("codex", 1070, 90000),
        ],
        workflow_notes: [],
        switches: [],
    }
}

test "BAML activity totals and routes use observations once" {
    let view = dashboard_view(test_activity(), 0);
    assert.equal(view.total_ms, 160000);
    assert.equal(view.routes.length(), 2);
    assert.equal(view.total_moves, 2);
    assert.equal(view.app_count, 3);
    assert.equal(view.stops.length(), 1);
    assert.equal(view.stops[0].app, "slack");
    assert.equal(view.stops[0].next, "codex")
}

test "a short stop is not a changed selection event" {
    let view = dashboard_view(test_activity(), 0);
    assert.equal(view.stops.length(), 1);
    assert.equal(view.changed, 0);
    assert.equal(view.kept, 0)
}

test "BAML dashboard respects period boundaries and idle gaps" {
    let activity = test_activity();
    activity.visits.push(test_visit("browser", 10000, 1000));
    let view = dashboard_view(activity, 1060);
    assert.equal(view.total_ms, 101000);
    assert.equal(view.routes.length(), 1);
    assert.equal(view.routes[0].from, "slack");
    assert.equal(view.routes[0].average_stay_ms, 90000)
}

test "BAML keeps a cycle shortlist stable and records the final selection" {
    let started = cycle_start(
        CycleStartData {
            ranking: ["slack", "browser", "com.openai.codex"],
            context: fixture(),
            fast: false,
            source: "history",
            event_id: "test",
            at: 1000,
        },
    );
    assert.equal(started.cycle?.choices.at(0)?.bundle_id, "slack");
    let next = cycle_next(CycleNextData { open: ["slack", "browser", "com.openai.codex"], pick: null });
    assert.equal(next.cycle?.index, 1);
    let after_closed = cycle_next(CycleNextData { open: ["slack", "browser"], pick: null });
    assert.equal(after_closed.cycle?.index, 0);
    cycle_next(CycleNextData { open: ["slack", "com.openai.codex"], pick: 2 });
    let ended = cycle_end(1002);
    assert.equal(ended.ended?.initial_target, "slack");
    assert.equal(ended.ended?.final_target, "com.openai.codex");
    assert.equal(ended.ended?.advances, 3);
    assert.equal(cycle_end(1003).ended, null);
    cycle_start(
        CycleStartData {
            ranking: [],
            context: fixture(),
            fast: false,
            source: "history",
            event_id: "cancelled",
            at: 2000,
        },
    );
    cycle_next(CycleNextData { open: ["slack", "browser", "com.openai.codex"], pick: null });
    assert.equal(cycle_cancel().ended, null);
    assert.equal(cycle_end(2001).ended, null)
}

test "BAML derives transition evidence from recorded app order" {
    let context = fixture();
    context.recent_history = ["editor", "browser", "editor", "browser", "editor"];
    assert.equal(transition_counts(context).get("browser"), 2)
}

test "all open apps remain reachable beyond the first page and wrap" {
    let context = fixture();
    context.current_app_id = "origin";
    context.apps
        = baml.Array.generate(8, (i) -> {
            OpenApp {
                bundle_id: `app${i}`,
                name: `App ${i}`,
                aliases: [],
                recency: i,
                transitions: 0,
            }
        });
    context.apps.push(
        OpenApp { bundle_id: "origin", name: "Origin", aliases: [], recency: 0, transitions: 0 },
    );
    let started = cycle_plan(
        CycleStartData {
            ranking: [],
            context: context,
            fast: false,
            source: "history",
            event_id: "full-list",
            at: 1000,
        },
    );
    assert.equal(started != null, true);
    if let plan: CyclePlan = started {
        assert.equal(plan.choices.length(), 9);
        assert.equal(plan.visible_indices, [0, 1, 2, 3, 4, 5, 6]);
        let open = context.apps.map((app) -> {
            app.bundle_id
        });
        for (
            let i in baml.Array.generate(8, (i) -> {
                i + 1
            })
        ) {
            let next = advance_cycle(plan, CycleNextData { open: open, pick: null });
            assert.equal(next.index, i);
            assert.equal(next.visible_indices.includes(i), true);
        }
        assert.equal(advance_cycle(plan, CycleNextData { open: open, pick: null }).index, 0);
        assert.equal(advance_cycle(plan, CycleNextData { open: ["app7"], pick: null }).index, 7);
        assert.equal(advance_cycle(plan, CycleNextData { open: [], pick: null }).index, 7);
        assert.equal(plan.choices[plan.index].bundle_id, "app7");
    }
}

test "an optimistic first choice becomes preview-only on every subsequent tap" {
    let plan = cycle_plan(
        CycleStartData {
            ranking: ["browser", "slack", "com.openai.codex"],
            fast: true,
            context: fixture(),
            source: "model",
            event_id: "fast",
            at: 1000,
        },
    );
    assert.equal(plan?.fast, true);
    if let active: CyclePlan = plan {
        let next = advance_cycle(active, CycleNextData { open: ["browser", "slack", "com.openai.codex"], pick: null });
        assert.equal(next.fast, false);
        assert.equal(next.choices[next.index].bundle_id, "slack");
        assert.equal(
            advance_cycle(next, CycleNextData { open: ["browser", "slack", "com.openai.codex"], pick: null }).fast,
            false,
        )
    }
}
