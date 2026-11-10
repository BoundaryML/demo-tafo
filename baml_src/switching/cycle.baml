class CycleStartData {
    ranking: string[],
    context: Snapshot,
    fast: bool,
    source: string,
    event_id: string,
    at: int,
}

class CycleNextData {
    open: string[],
    pick: int?,
}

class CyclePlan {
    fast: bool,
    context: Snapshot,
    choices: OpenApp[],
    index: int,
    visible_indices: int[],
    advances: int,
    started_at: int,
    event_id: string,
    source: string,
}

class CycleStore {
    plan: CyclePlan?,
}

const CYCLE = CycleStore { plan: null };

class CycleReply {
    cycle: CyclePlan?,
}

class CycleEnded {
    ended: SwitchEvent?,
}

function cycle_plan(data: CycleStartData) -> CyclePlan? throws never {
    let ranked = ranked_apps(data.context);
    let ordered: OpenApp[] = [];
    for (let id in data.ranking) {
        let app = ranked.find((a) -> {
            a.bundle_id == id
        });
        if (
            app != null
                && !ordered.some((a) -> {
                    a.bundle_id == id
                })
        ) {
            ordered.push(app);
        }
    }
    let choices_before_origin = ordered.concat(
        ranked.filter((app) -> {
            !ordered.some((a) -> {
                a.bundle_id == app.bundle_id
            })
        }),
    );
    let origin = data.context.apps.filter((app) -> {
        app.bundle_id == data.context.current_app_id
    });
    let choices = choices_before_origin.concat(origin);
    let plan = if (choices.length() > 0) {
        CyclePlan {
            fast: data.fast,
            context: data.context,
            choices: choices,
            index: 0,
            visible_indices: cycle_visible(choices.length(), 0),
            advances: 0,
            started_at: data.at,
            event_id: data.event_id,
            source: data.source,
        }
    } else {
        null
    };
    plan
}

function cycle_start(data: CycleStartData) -> CycleReply throws never {
    let plan = cycle_plan(data);
    CYCLE.plan = plan;
    CycleReply { cycle: plan }
}

function cycle_next(data: CycleNextData) -> CycleReply throws never {
    let plan = CYCLE.plan
        ?? return CycleReply { cycle: null };
    CycleReply { cycle: advance_cycle(plan, data) }
}

function advance_cycle(plan: CyclePlan, data: CycleNextData) -> CyclePlan throws never {
    // A second tap is a correction: preview alternatives without further activation.
    plan.fast = false;
    let pick = data.pick;
    let indices = if (pick != null) {
        [pick]
    } else {
        baml.Array.generate(plan.choices.length(), (offset) -> {
            (plan.index + offset + 1) % plan.choices.length()
        })
    };
    let next = indices.find((index) -> {
        let app = plan.choices.at(index);
        index >= 0 && app != null && data.open.includes(app.bundle_id)
    });
    if (next != null && next != plan.index) {
        plan.index = next;
        plan.advances += 1;
    }
    plan.visible_indices = cycle_visible(plan.choices.length(), plan.index);
    plan
}

function cycle_visible(count: int, index: int) -> int[] throws never {
    let start = index / 7 * 7;
    let remaining = count - start;
    baml.Array.generate(
        if (remaining < 7) {
            remaining
        } else {
            7
        },
        (offset) -> {
            start + offset
        },
    )
}

/// baml:$trace=cycle_trace
function cycle_end(at: int, timing: SwitchTiming? = null, record: bool = true) -> CycleEnded {
    let plan = CYCLE.plan;
    CYCLE.plan = null;
    if (plan == null) {
        return CycleEnded { ended: null };
    }
    let event = SwitchEvent {
        first_activation_ms: timing?.first_activation_ms,
        settled_activation_ms: timing?.settled_activation_ms,
        fast: timing?.fast,
        learning: pair_learning(plan.context, plan.choices[0].bundle_id),
        event_id: plan.event_id,
        started_at: plan.started_at,
        ended_at: at,
        origin: plan.context.current_app_id,
        initial_target: plan.choices[0].bundle_id,
        final_target: plan.choices[plan.index].bundle_id,
        candidates: plan.choices.map((app) -> {
            app.bundle_id
        }),
        advances: plan.advances,
        source: plan.source,
    };
    if (record) {
        record_switch(event);
    }
    CycleEnded { ended: event }
}

function cycle_cancel() -> CycleEnded throws never {
    CYCLE.plan = null;
    CycleEnded { ended: null }
}

function cycle_selection(plan: CyclePlan) -> Selection throws never {
    let app = plan.choices[plan.index];
    Selection {
        learning: null,
        fast: false,
        ranking: [],
        bundle_id: app.bundle_id,
        reason: "Selected in app switcher",
        workflow: null,
        source: match (plan.source) {
            "model" => "model",
            "name" => "name",
            "no_match" => "no_match",
            _ => "history",
        },
    }
}
