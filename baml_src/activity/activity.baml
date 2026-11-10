class WorkflowNote {
    visit_started: int,
    guess: WorkflowGuess,
}

class ActivityData {
    enabled: bool,
    visits: AppVisit[],
    workflow_notes: WorkflowNote[],
    switches: SwitchEvent[],
}

class AppTime {
    id: string,
    name: string,
    active_ms: int,
    visits: int,
}

class WorkflowSession {
    started: int,
    ended: int,
    title: string,
    confidence: float?,
    visits: AppVisit[],
    active_ms: int,
    apps: AppTime[],
    evidence: string,

    function can_extend(self, visit: AppVisit, note: WorkflowGuess?) -> bool throws never {
        if (visit.started - self.ended > 600 || visit.started - self.started >= 2700) {
            return false;
        }
        if (note == null || note.confidence < 0.85 || self.confidence == null) {
            return true;
        }
        self.title == workflow_title(note.label) || self.active_ms < 60000
    }
}

class ActivitySummary {
    total_ms: int,
    apps: AppTime[],
    sessions: WorkflowSession[],
    known_workflows: WorkflowHint[],
    known_apps: KnownApp[],
}

class SummarizeData {
    activity: ActivityData,
    since: int,
}

class PatternKey {
    label: string,
    apps: string[],
}

function display_app(id: string, name: string) -> string throws never {
    if (id == "com.openai.codex") {
        "Codex"
    } else {
        name
    }
}

function workflow_title(label: string) -> string throws never {
    match (label) {
        "building" => "Building",
        "research" => "Research",
        "communication" => "Conversations",
        "planning" => "Planning",
        "writing" => "Writing",
        _ => "Session",
    }
}

function active_time(ms: int) -> string throws never {
    let seconds = ms / 1000;
    if (seconds >= 3600) {
        `${seconds / 3600}h ${(seconds % 3600) / 60}m`
    } else if (seconds >= 60) {
        `${seconds / 60}m ${seconds % 60}s`
    } else {
        `${seconds}s`
    }
}

function app_times(visits: AppVisit[]) -> AppTime[] throws never {
    let totals: map<string, AppTime> = {};
    for (let visit in visits) {
        let app = totals.get(visit.bundle_id)
            ?? AppTime {
                id: visit.bundle_id,
                name: display_app(visit.bundle_id, visit.name),
                active_ms: 0,
                visits: 0,
            };
        app.active_ms += visit.active_ms;
        app.visits += 1;
        totals.set(visit.bundle_id, app);
    }
    totals.values().sort_by_key((app) -> {
        0 - app.active_ms
    })
}

function session_evidence(session: WorkflowSession) -> string throws never {
    let names = session.apps.slice(0, 3)
        .map((app) -> {
            app.name
        })
        .join(", ");
    let moves = 0;
    let previous = "";
    for (let visit in session.visits) {
        if (previous != "" && previous != visit.bundle_id) {
            moves += 1;
        }
        previous = visit.bundle_id;
    }
    if (moves == 0) {
        `${active_time(session.active_ms)} in ${names}. Still learning the pattern.`
    } else {
        `${active_time(session.active_ms)} active · ${moves} app changes across ${names}.`
    }
}

function workflow_sessions(activity: ActivityData, since: int) -> WorkflowSession[] throws never {
    let notes: map<string, WorkflowGuess> = {};
    for (
        let note in activity.workflow_notes.filter((note) -> {
            valid_workflow(note.guess.label, note.guess.confidence)
        })
    ) {
        notes.set(note.visit_started.to_string(), note.guess);
    }
    let sessions: WorkflowSession[] = [];
    for (
        let visit in activity.visits.filter((v) -> {
            v.started >= since
        })
    ) {
        let note = notes.get(visit.started.to_string());
        let session = match (sessions.at(-1)) {
            let current: WorkflowSession if current.can_extend(visit, note) => current,
            _ => {
                let fresh = WorkflowSession {
                    started: visit.started,
                    ended: visit.ended,
                    title: "Session",
                    confidence: null,
                    visits: [],
                    active_ms: 0,
                    apps: [],
                    evidence: "",
                };
                sessions.push(fresh);
                fresh
            },
        };
        session.ended
            = if (visit.ended > session.ended) {
                visit.ended
            } else {
                session.ended
            };
        session.visits.push(visit);
        session.active_ms += visit.active_ms;
        if (note != null) {
            session.title = workflow_title(note.label);
            session.confidence = note.confidence;
        }
    }
    sessions.map((session) -> {
        session.apps = app_times(session.visits);
        session.evidence = session_evidence(session);
        session
    })
}

function workflow_hints(sessions: WorkflowSession[]) -> WorkflowHint[] {
    let patterns: map<string, WorkflowHint> = {};
    for (let session in sessions.reverse()) {
        if (session.confidence != null) {
            let apps = session.apps
                .map((app) -> {
                    app.id
                })
                .sort_by_key((id) -> {
                    id
                });
            let key = baml.json.stringify(PatternKey { label: session.title, apps: apps });
            let pattern = patterns.get(key) ?? WorkflowHint { label: session.title, apps: apps, sessions: 0 };
            pattern.sessions += 1;
            patterns.set(key, pattern);
        }
    }
    patterns.values()
        .sort_by_key((pattern) -> {
            0 - pattern.sessions
        })
        .slice(0, 8)
}

function known_apps(visits: AppVisit[]) -> KnownApp[] throws never {
    let apps: map<string, KnownApp> = {};
    for (let visit in visits) {
        apps.set(
            visit.bundle_id,
            KnownApp {
                bundle_id: visit.bundle_id,
                name: display_app(visit.bundle_id, visit.name),
                last_seen: visit.ended,
            },
        );
    }
    apps.values()
        .sort_by_key((app) -> {
            0 - app.last_seen
        })
        .slice(0, 128)
}

function summarize_activity(data: SummarizeData) -> ActivitySummary {
    let visits = data.activity.visits.filter((v) -> {
        v.started >= data.since
    });
    let total = 0;
    for (let visit in visits) {
        total += visit.active_ms;
    }
    ActivitySummary {
        total_ms: total,
        apps: app_times(visits),
        sessions: workflow_sessions(data.activity, data.since).reverse().slice(0, 100).reverse().map((s) -> {
            WorkflowSession {
                started: s.started,
                ended: s.ended,
                title: s.title,
                confidence: s.confidence,
                visits: [],
                active_ms: s.active_ms,
                apps: s.apps,
                evidence: s.evidence,
            }
        }),
        known_workflows: workflow_hints(workflow_sessions(data.activity, 0)),
        known_apps: known_apps(data.activity.visits),
    }
}

function transition_counts(snapshot: Snapshot) -> map<string, int> throws never {
    let counts: map<string, int> = {};
    let previous = "";
    for (let id in snapshot.recent_history.reverse()) {
        if (previous == snapshot.current_app_id) {
            counts.set(id, (counts.get(id) ?? 0) + 1);
        }
        previous = id;
    }
    counts
}

function enrich_snapshot(snapshot: Snapshot, activity: ActivityData) -> Snapshot {
    let summary = summarize_activity(SummarizeData { activity: activity, since: 0 });
    let counts = transition_counts(snapshot);
    Snapshot {
        current_app_id: snapshot.current_app_id,
        apps: snapshot.apps.map((app) -> {
            OpenApp {
                bundle_id: app.bundle_id,
                name: app.name,
                aliases: app.aliases,
                recency: app.recency,
                transitions: counts.get(app.bundle_id) ?? 0,
            }
        }),
        recent_history: snapshot.recent_history,
        recent_activity: snapshot.recent_activity,
        known_workflows: summary.known_workflows,
        known_apps: summary.known_apps,
        recent_switches: activity.switches.reverse().slice(0, 16),
    }
}
