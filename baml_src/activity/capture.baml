// Local, deterministic policies. This endpoint never calls a model or emits traces.
const IDLE_SECONDS = 120.0;
const MAX_CAPTURE_GAP_MS = 2000;
const RETENTION_SECONDS = 30 * 24 * 60 * 60;
const MAX_VISITS = 100000;
const MAX_DECISIONS = 5000;
const MAX_ACTIVATIONS = 256;

class CapturedApp {
    id: string,
    name: string,
}

class CaptureData {
    enabled: bool,
    previous: AppVisit?,
    previous_day: int?,
    day: int,
    app: CapturedApp?,
    at: int,
    delta_ms: int,
    idle_seconds: float?,
}

class CaptureDelta {
    previous: AppVisit?,
    append: AppVisit?,
    keep_current: bool,
}

function capture_observation(data: CaptureData) -> CaptureDelta throws never {
    let idle = data.idle_seconds;
    let active = data.enabled
        && idle != null
        && idle >= 0.0
        && idle < IDLE_SECONDS
        && data.delta_ms >= 0
        && data.delta_ms <= MAX_CAPTURE_GAP_MS;
    let previous = data.previous;
    if (!active) {
        return CaptureDelta { previous: null, append: null, keep_current: false };
    }
    if (previous != null) {
        previous.active_ms += data.delta_ms;
        previous.ended
            = if (data.at > previous.started) {
                data.at
            } else {
                previous.started
            };
    }
    let app = data.app;
    let same = app != null && previous != null && previous.bundle_id == app.id && data.previous_day == data.day;
    let next = if (app != null && !same) {
        AppVisit {
            bundle_id: app.id,
            name: app.name,
            started: data.at,
            ended: data.at,
            active_ms: 0,
        }
    } else {
        null
    };
    CaptureDelta { previous: previous, append: next, keep_current: same }
}

class RetentionData {
    at: int,
    visits: int[],
    decisions: int[],
    workflow_notes: int[],
    switches: int[],
}

class RetentionPlan {
    visits_drop: int,
    decisions_drop: int,
    workflow_notes_keep: int[],
    switches_keep: int[],
}

function retained_indices(times: int[], cutoff: int) -> int[] throws never {
    baml.Array
        .generate(times.length(), (index) -> {
            index
        })
        .filter((index) -> {
            times[index] >= cutoff
        })
}

function expired_prefix(times: int[], cutoff: int, limit: int) -> int throws never {
    let expired = times.find_index((at) -> {
        at >= cutoff
    })
        ?? times.length();
    let excess = times.length() - limit;
    if (expired > excess) {
        expired
    } else {
        excess
    }
}

function retention_plan(data: RetentionData) -> RetentionPlan throws never {
    let cutoff = if (data.at > RETENTION_SECONDS) {
        data.at - RETENTION_SECONDS
    } else {
        0
    };
    let switches = retained_indices(data.switches, cutoff);
    RetentionPlan {
        visits_drop: expired_prefix(data.visits, cutoff, MAX_VISITS),
        decisions_drop: expired_prefix(data.decisions, cutoff, MAX_DECISIONS),
        workflow_notes_keep: retained_indices(data.workflow_notes, cutoff),
        switches_keep: switches.slice(
            if (switches.length() > MAX_DECISIONS) {
                switches.length() - MAX_DECISIONS
            } else {
                0
            },
            switches.length(),
        ),
    }
}

class Decision {
    at: int,
    from: string,
    to: string,
    source: string,
    reason: string,
}

class DecisionData {
    enabled: bool,
    at: int,
    context: Snapshot,
    choice: Selection,
    reason: string,
}

function decision_record(data: DecisionData) -> Decision? throws never {
    let to = data.choice.bundle_id;
    if (!data.enabled || to == null) {
        return null;
    }
    let names: map<string, string> = {};
    for (let app in data.context.apps) {
        names.set(app.bundle_id, display_app(app.bundle_id, app.name));
    }
    Decision {
        at: data.at,
        from: names.get(data.context.current_app_id) ?? data.context.current_app_id,
        to: names.get(to) ?? to,
        source: data.choice.source,
        reason: data.reason,
    }
}

class WorkflowUpdate {
    enabled: bool,
    notes: WorkflowNote[],
    visit_started: int,
    guess: WorkflowGuess,
}

function record_workflow_note(data: WorkflowUpdate) -> WorkflowNote[] throws never {
    if (!data.enabled) {
        return data.notes;
    }
    let note = WorkflowNote { visit_started: data.visit_started, guess: data.guess };
    if (
        data.notes.some((old) -> {
            old.visit_started == data.visit_started
        })
    ) {
        data.notes.map((old) -> {
            if (old.visit_started == data.visit_started) {
                note
            } else {
                old
            }
        })
    } else {
        data.notes.concat([note])
    }
}

class ActivationData {
    activations: string[],
    id: string,
}

function record_activation(data: ActivationData) -> string[] throws never {
    if (!data.id || data.activations.at(data.activations.length() - 1) == data.id) {
        return data.activations;
    }
    let all = data.activations.concat([data.id]);
    all.slice(
        if (all.length() > MAX_ACTIVATIONS) {
            all.length() - MAX_ACTIVATIONS
        } else {
            0
        },
        all.length(),
    )
}

