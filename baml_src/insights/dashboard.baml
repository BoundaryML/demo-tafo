class PeriodBoundary {
    key: string,
    since: int,
}

class DashboardData {
    activity: ActivityData,
    periods: PeriodBoundary[],
    at: int,
}

class RouteKey {
    from: string,
    to: string,
}

class LoopKey {
    apps: string[],
}

class AppRoute {
    from: string,
    to: string,
    count: int,
    stay_ms: int,
    average_stay_ms: int,
}

class AppLoop {
    apps: string[],
    moves: int,
}

class ShortStop {
    app: string,
    next: string,
    at: int,
    active_ms: int,
}

class WorkflowStat {
    label: string,
    active_ms: int,
    sessions: int,
}

class DashboardSession {
    started: int,
    ended: int,
    title: string,
    confidence: float?,
    active_ms: int,
    apps: string[],
}

class DashboardView {
    value: SwitchValue,
    performance: SwitchPerformance,
    since: int,
    total_ms: int,
    app_count: int,
    total_moves: int,
    apps: AppTime[],
    routes: AppRoute[],
    loops: AppLoop[],
    sessions: DashboardSession[],
    workflows: WorkflowStat[],
    recent_visits: AppVisit[],
    switches: SwitchEvent[],
    kept: int,
    changed: int,
    stops: ShortStop[],
}

class DashboardSnapshot {
    at: int,
    periods: map<string, DashboardView>,
}

class DashboardReply {
    dashboard: DashboardSnapshot,
}

function dashboard_view(activity: ActivityData, since: int) -> DashboardView {
    let visits = activity.visits.filter((visit) -> {
        visit.started >= since
    });
    let summary = summarize_activity(SummarizeData { activity: activity, since: since });
    let routes: map<string, AppRoute> = {};
    let loops: map<string, AppLoop> = {};
    let stops: ShortStop[] = [];
    let previous: AppVisit? = null;
    for (let visit in visits) {
        let prior = previous;
        if (prior != null && prior.bundle_id != visit.bundle_id && visit.started - prior.ended <= 600) {
            let key = baml.json.stringify(RouteKey { from: prior.bundle_id, to: visit.bundle_id });
            let route = routes.get(key)
                ?? AppRoute {
                    from: prior.bundle_id,
                    to: visit.bundle_id,
                    count: 0,
                    stay_ms: 0,
                    average_stay_ms: 0,
                };
            route.count += 1;
            route.stay_ms += visit.active_ms;
            route.average_stay_ms = route.stay_ms / route.count;
            routes.set(key, route);
            let pair = [prior.bundle_id, visit.bundle_id].sort_by_key((id) -> {
                id
            });
            let loop_key = baml.json.stringify(LoopKey { apps: pair });
            let loop = loops.get(loop_key) ?? AppLoop { apps: pair, moves: 0 };
            loop.moves += 1;
            loops.set(loop_key, loop);
            if (prior.active_ms > 0 && prior.active_ms <= 15000 && visit.started - prior.ended <= 60) {
                stops.push(
                    ShortStop {
                        app: prior.bundle_id,
                        next: visit.bundle_id,
                        at: prior.started,
                        active_ms: prior.active_ms,
                    },
                );
            }
        }
        previous = visit;
    }
    let workflows: map<string, WorkflowStat> = {};
    for (let session in workflow_sessions(activity, since)) {
        let stat = workflows.get(session.title)
            ?? WorkflowStat { label: session.title, active_ms: 0, sessions: 0 };
        stat.active_ms += session.active_ms;
        stat.sessions += 1;
        workflows.set(session.title, stat);
    }
    let switches = activity.switches.filter((event) -> {
        event.ended_at >= since
    });
    let changed = switches
        .filter((event) -> {
            event.final_target != event.initial_target
        })
        .length();
    let moves = routes.values().reduce(
        (total, route) -> {
            total + route.count
        },
        0,
    );
    DashboardView {
        value: switch_value(visits, switches, moves),
        performance: switch_performance(switches, activity.switches),
        since: since,
        total_ms: summary.total_ms,
        app_count: summary.apps.length(),
        total_moves: routes.values().reduce(
            (total, route) -> {
                total + route.count
            },
            0,
        ),
        apps: summary.apps.slice(0, 12),
        routes: routes.values()
            .sort_by_key((route) -> {
                0 - route.count
            })
            .slice(0, 40),
        loops: loops.values()
            .sort_by_key((loop) -> {
                0 - loop.moves
            })
            .slice(0, 8),
        sessions: summary.sessions.reverse().slice(0, 12).map((s) -> {
            DashboardSession {
                started: s.started,
                ended: s.ended,
                title: s.title,
                confidence: s.confidence,
                active_ms: s.active_ms,
                apps: s.apps.map((app) -> {
                    app.id
                }),
            }
        }),
        workflows: workflows.values().sort_by_key((stat) -> {
            0 - stat.active_ms
        }),
        recent_visits: visits.reverse().slice(0, 30),
        switches: switches.reverse().slice(0, 30),
        kept: switches.length() - changed,
        changed: changed,
        stops: stops.reverse().slice(0, 20),
    }
}

/// baml:$trace=dashboard_trace
function record_dashboard(data: DashboardData, record: bool = true) -> DashboardReply {
    let periods: map<string, DashboardView> = {};
    for (let period in data.periods) {
        periods.set(period.key, dashboard_view(data.activity, period.since));
    }
    let snapshot = DashboardSnapshot { at: data.at, periods: periods };
    let span = if (recording_allowed()) {
        trace.start_span("smart_switch.dashboard")
    } else {
        null
    };
    if (span != null) {
        span.finish(snapshot);
    }
    DashboardReply { dashboard: snapshot }
}
