class OpenApp {
    bundle_id: string,
    name: string,
    aliases: string[],
    recency: int,
    transitions: int,
}

class AppVisit {
    bundle_id: string,
    name: string,
    started: int,
    ended: int,
    active_ms: int,
}

class WorkflowGuess {
    label: string,
    confidence: float,
}

class WorkflowHint {
    label: string,
    apps: string[],
    sessions: int,
}

class KnownApp {
    bundle_id: string,
    name: string,
    last_seen: int,
}

class Snapshot {
    current_app_id: string,
    apps: OpenApp[],
    recent_history: string[],
    recent_activity: AppVisit[],
    known_workflows: WorkflowHint[],
    known_apps: KnownApp[],
    recent_switches: SwitchEvent[],
}

class Selection {
    learning: PairLearning?,
    fast: bool,
    ranking: string[],
    bundle_id: string?,
    reason: string,
    source: "history" | "name" | "model" | "no_match",
    workflow: WorkflowGuess?,
}

function normalize(text: string) -> string throws never {
    text.to_lower_case()
        .chars()
        .map((c) -> {
            if (c.is_alphanumeric()) {
                c
            } else {
                " "
            }
        })
        .join("")
        .split(" ")
        .filter((s) -> {
            s != ""
        })
        .join(" ")
}

function app_names(app: OpenApp) -> string[] throws never {
    [app.name, app.bundle_id].concat(app.aliases)
        .map((s) -> {
            normalize(s)
        })
        .filter((s) -> {
            s != ""
        })
}

function named_apps(snapshot: Snapshot, request: string) -> OpenApp[] throws never {
    let query = ` ${normalize(request)} `;
    snapshot.apps.filter((app) -> {
        app_names(app).some((name) -> {
            query.includes(` ${name} `)
        })
    })
}

const MAX_PRIORITY_EVIDENCE = 3;
const MIN_WORKFLOW_CONFIDENCE = 0.70;
const WORKFLOW_LABELS = ["building", "research", "communication", "planning", "writing"];

function valid_workflow(label: string, confidence: float) -> bool throws never {
    confidence >= MIN_WORKFLOW_CONFIDENCE && confidence <= 1.0 && WORKFLOW_LABELS.includes(label)
}

// A shared saturation rule prevents old frequency counts dominating recent work.
function bounded_evidence(count: int) -> int throws never {
    if (count > MAX_PRIORITY_EVIDENCE) {
        MAX_PRIORITY_EVIDENCE
    } else {
        count
    }
}

// Count distinct foreground apps, preserving their first occurrence in newest-first history.
function observed_rank(history: string[], id: string) -> int? throws never {
    let ranks: map<string, int> = {};
    for (let app in history) {
        if (!ranks.has(app)) {
            ranks.set(app, ranks.length());
        }
    }
    ranks.get(id)
}

function app_priority(
    snapshot: Snapshot,
    app: OpenApp,
    scores: map<string, float>,
) -> float throws never {
    let rank = observed_rank(snapshot.recent_history, app.bundle_id);
    let recency = match (rank) {
        let observed: int => 60.0 / (observed + 2),
        null => 2.0 / (app.recency + 2),
    };
    let affinity = snapshot.known_workflows
        .filter((flow) -> {
            flow.apps.includes(snapshot.current_app_id) && flow.apps.includes(app.bundle_id)
        })
        .reduce(
            (total, flow) -> {
                total + bounded_evidence(flow.sessions)
            },
            0,
        );
    let accepted = snapshot.recent_switches.slice(0, 16)
        .filter((event) -> {
            event.origin == snapshot.current_app_id
                && event.final_target == app.bundle_id
                && event.final_target != event.origin
        })
        .length();
    let rejected = snapshot.recent_switches.slice(0, 8)
        .filter((event) -> {
            event.origin == snapshot.current_app_id
                && event.initial_target == app.bundle_id
                && event.final_target != app.bundle_id
                && event.advances > 0
        })
        .length();
    (
        recency
            + bounded_evidence(app.transitions) * 2.0
            + bounded_evidence(affinity) * 3.0
            + bounded_evidence(accepted) * 4.0
            - bounded_evidence(rejected) * 10.0
            + (scores.get(app.bundle_id) ?? 0.0) * 12.0
    )
}

function ranked_with_scores(
    snapshot: Snapshot,
    scores: map<string, float>,
) -> OpenApp[] throws never {
    snapshot.apps
        .filter((app) -> {
            app.bundle_id != snapshot.current_app_id
        })
        .sort_by_key((app) -> {
            0.0 - app_priority(snapshot, app, scores)
        })
}

function ranked_apps(snapshot: Snapshot) -> OpenApp[] throws never {
    ranked_with_scores(snapshot, {})
}

function predict(snapshot: Snapshot) -> Selection throws never {
    let ranked = ranked_apps(snapshot);
    match (ranked.at(0)) {
        let app: OpenApp => {
            let learned = pair_learning(snapshot, app.bundle_id);
            Selection {
                learning: learned,
                fast: learned.ready,
                ranking: ranked.map((candidate) -> {
                    candidate.bundle_id
                }),
                workflow: null,
                bundle_id: app.bundle_id,
                reason: if (app.transitions >= 2) {
                    "Your repeated next app from here"
                } else {
                    "Your most recently used other app"
                },
                source: "history",
            }
        },
        _ => {
            Selection {
                learning: null,
                fast: false,
                ranking: [],
                workflow: null,
                bundle_id: null,
                reason: "No other open app to switch to",
                source: "no_match",
            }
        },
    }
}

function validate_choice(choice: ModelChoice, snapshot: Snapshot) -> Selection throws never {
    let id = choice.bundle_id;
    if (id == null || choice.confidence < 0.65 || choice.confidence > 1.0) {
        return Selection {
            learning: null,
            fast: false,
            ranking: [],
            workflow: null,
            bundle_id: null,
            reason: "Please name the app more specifically",
            source: "no_match",
        };
    }
    if (
        !snapshot.apps.some((app) -> {
            app.bundle_id == id
        })
    ) {
        return Selection {
            learning: null,
            fast: false,
            ranking: [],
            workflow: null,
            bundle_id: null,
            reason: "That app is not currently open",
            source: "no_match",
        };
    }
    Selection {
        learning: null,
        fast: false,
        ranking: [],
        workflow: choice.workflow,
        bundle_id: id,
        reason: choice.reason,
        source: "model",
    }
}

function select_app(snapshot: Snapshot, request: string) -> Selection {
    //# Fast local selection
    if (!request.trim()) {
        return predict(snapshot);
    }
    let matches = named_apps(snapshot, request);
    if (matches.length() == 1) {
        let app = matches[0];
        return Selection {
            learning: null,
            fast: false,
            ranking: [],
            workflow: null,
            bundle_id: app.bundle_id,
            reason: "Matched the app you named",
            source: "name",
        };
    }
    if (matches.length() > 1) {
        return Selection {
            learning: null,
            fast: false,
            ranking: [],
            workflow: null,
            bundle_id: null,
            reason: "You named multiple open apps; choose one",
            source: "no_match",
        };
    }
    //# Interpret a spoken or typed description with a model
    let choice = InterpretApp(snapshot, request);
    validate_choice(choice, snapshot)
}

// Used by the background predictor, never on the keyboard event thread.
function confident_fast_path(
    choice: ModelChoice,
    snapshot: Snapshot,
    ranked: OpenApp[],
) -> bool throws never {
    let best = ranked.at(0)
        ?? return false;
    if (
        choice.bundle_id != best.bundle_id
            || choice.confidence < 0.92
            || choice.confidence > 1.0
            || !choice.fast_candidates.includes(best.bundle_id)
    ) {
        return false;
    }
    let next = ranked.at(1);
    let margin = if (next != null) {
        app_priority(snapshot, best, choice.scores) - app_priority(snapshot, next, choice.scores)
    } else {
        100.0
    };
    let corrected = snapshot.recent_switches.slice(0, 8).some((event) -> {
        event.origin == snapshot.current_app_id
            && event.initial_target == best.bundle_id
            && event.final_target != best.bundle_id
            && event.advances > 0
    });
    margin >= 10.0 && !corrected
}

function validate_prediction(choice: ModelChoice, snapshot: Snapshot) -> Selection throws never {
    let selected = validate_choice(choice, snapshot);
    let ranked = ranked_with_scores(snapshot, choice.scores);
    let best = ranked.at(0);
    let useful = choice.scores.length() > 0;
    if (useful && best != null) {
        return Selection {
            learning: pair_learning(snapshot, best.bundle_id),
            fast: confident_fast_path(choice, snapshot, ranked)
                || pair_learning(snapshot, best.bundle_id).ready,
            ranking: ranked.map((app) -> {
                app.bundle_id
            }),
            workflow: choice.workflow,
            bundle_id: best.bundle_id,
            reason: "Ordered by JEV workflow relevance, recent activity and settled choices",
            source: "model",
        };
    }
    if (
        choice.confidence < 0.8
            || selected.bundle_id == snapshot.current_app_id
            || selected.bundle_id == null
    ) {
        let fallback = predict(snapshot);
        return Selection {
            learning: fallback.learning,
            fast: fallback.fast,
            ranking: ranked.map((app) -> {
                app.bundle_id
            }),
            workflow: choice.workflow,
            bundle_id: fallback.bundle_id,
            reason: fallback.reason,
            source: fallback.source,
        };
    }
    Selection {
        learning: pair_learning(snapshot, selected.bundle_id ?? ""),
        fast: pair_learning(snapshot, selected.bundle_id ?? "").ready,
        ranking: [selected.bundle_id ?? ""].concat(
            ranked
                .filter((app) -> {
                    app.bundle_id != selected.bundle_id
                })
                .map((app) -> {
                    app.bundle_id
                }),
        ),
        workflow: choice.workflow,
        bundle_id: selected.bundle_id,
        reason: "JEV workflow prediction, with recent apps behind it",
        source: "model",
    }
}

function predict_workflow(snapshot: Snapshot) -> Selection {
    validate_prediction(InterpretApp(snapshot, ""), snapshot)
}
