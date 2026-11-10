use smart_switch::*;
use std::time::Duration;
static CYCLE_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
#[test]
fn tap() {
    let mut k = PressTracker::default();
    assert!(k.down(ms(0)));
    assert_eq!(k.up(ms(200)), PressAction::Tap);
    assert_eq!(k.up(ms(210)), PressAction::None);
}
#[test]
fn hold() {
    let mut k = PressTracker::default();
    k.down(ms(0));
    assert_eq!(k.held(ms(599)), PressAction::None);
    assert_eq!(k.held(ms(600)), PressAction::Prompt);
    assert_eq!(k.held(ms(800)), PressAction::None);
    assert_eq!(k.up(ms(1000)), PressAction::None);
}
#[test]
fn late_timer() {
    let mut k = PressTracker::default();
    k.down(ms(0));
    assert_eq!(k.up(ms(600)), PressAction::Prompt);
}
#[test]
fn repeat() {
    let mut k = PressTracker::default();
    k.down(ms(0));
    assert!(!k.down(ms(400)));
    assert_eq!(k.held(ms(600)), PressAction::Prompt);
}
#[test]
fn cancellation() {
    let mut k = PressTracker::default();
    k.down(ms(0));
    k.cancel();
    assert_eq!(k.up(ms(1000)), PressAction::None);
    assert!(k.down(ms(1100)));
}
#[test]
fn history() {
    let mut h = History::default();
    for id in ["a", "a", "b", "a", "b", "a", "c"] {
        h.record(id).unwrap();
    }
    assert_eq!(h.activations.len(), 6);
}
#[test]
fn bounded_history() {
    let mut h = History::default();
    for i in 0..300 {
        h.record(&i.to_string()).unwrap();
    }
    assert_eq!(h.activations.len(), 256);
    assert_eq!(h.activations[0], "44");
    let copy: History = serde_json::from_slice(&serde_json::to_vec(&h).unwrap()).unwrap();
    assert_eq!(h.activations, copy.activations);
}

#[test]
fn activity_charges_previous_app_and_excludes_idle_sleep_and_pauses() {
    let mut log = Activity::default();
    log.observe(Some(("editor", "Cursor")), 100, 200, 0.0)
        .unwrap();
    log.observe(Some(("editor", "Cursor")), 101, 1000, 1.0)
        .unwrap();
    log.observe(Some(("browser", "Arc")), 102, 1000, 0.0)
        .unwrap();
    assert_eq!(log.visits[0].active_ms, 2000);
    log.observe(Some(("browser", "Arc")), 103, 1000, 0.0)
        .unwrap();
    log.observe(Some(("browser", "Arc")), 104, 1000, 121.0)
        .unwrap();
    log.observe(Some(("browser", "Arc")), 900, 796000, 0.0)
        .unwrap();
    assert_eq!(log.visits[1].active_ms, 1000);
    log.enabled = false;
    log.observe(Some(("slack", "Slack")), 901, 1000, 0.0)
        .unwrap();
    log.enabled = true;
    log.observe(Some(("slack", "Slack")), 902, 1000, 0.0)
        .unwrap();
    log.observe(Some(("slack", "Slack")), 903, 1000, 0.0)
        .unwrap();
    assert_eq!(log.visits.last().unwrap().active_ms, 1000);
    assert_eq!(log.visits.len(), 3);
}

#[test]
fn activity_restart_does_not_charge_time_while_closed() {
    let mut log = Activity::default();
    log.observe(Some(("editor", "Cursor")), 100, 200, 0.0)
        .unwrap();
    log.observe(Some(("editor", "Cursor")), 101, 1000, 0.0)
        .unwrap();
    let mut loaded: Activity = serde_json::from_slice(&serde_json::to_vec(&log).unwrap()).unwrap();
    loaded
        .observe(Some(("editor", "Cursor")), 10000, 200, 0.0)
        .unwrap();
    assert_eq!(loaded.visits.len(), 2);
    assert_eq!(loaded.visits[0].active_ms, 1000);
    assert_eq!(loaded.visits[1].active_ms, 0);
}

#[test]
fn activity_retention_keeps_current_session_accounting_correct() {
    let mut log = Activity::default();
    log.observe(Some(("old", "Old")), 1, 200, 0.0).unwrap();
    log.observe(Some(("new", "New")), 3000000, 3000000, 0.0)
        .unwrap();
    log.observe(Some(("new", "New")), 3000001, 200, 0.0)
        .unwrap();
    log.prune(3000001).unwrap();
    log.observe(Some(("new", "New")), 3000002, 1000, 0.0)
        .unwrap();
    assert_eq!(log.visits.len(), 1);
    assert_eq!(log.visits[0].active_ms, 1000);
}

#[test]
fn activity_storage_is_private_and_pause_survives_restart() {
    use std::os::unix::fs::PermissionsExt;
    let directory =
        std::env::temp_dir().join(format!("smart-switch-activity-test-{}", std::process::id()));
    let file = directory.join("activity.json");
    let mut log = Activity::default();
    log.enabled = false;
    log.save(&file).unwrap();
    assert!(!Activity::load(&file).enabled);
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cached_prediction_key_changes_when_workflow_or_inventory_changes() {
    let mut snapshot = Snapshot {
        current_app_id: "editor".into(),
        apps: vec![],
        recent_history: vec!["editor".into()],
        recent_activity: vec![],
        known_workflows: vec![],
        known_apps: vec![],
        recent_switches: vec![],
    };
    let original = prediction_key(&snapshot);
    snapshot.recent_history.push("browser".into());
    assert_ne!(original, prediction_key(&snapshot));
    let sequence = prediction_key(&snapshot);
    snapshot.current_app_id = "browser".into();
    assert_ne!(sequence, prediction_key(&snapshot));
}
#[test]
fn ordering() {
    let s="ASN:0x0-0x2: ASN:0x0-0x1: ASN:0x0-0x3:\n1) A ASN:0x0-0x1:\n bundleID=\"a\"\n2) B ASN:0x0-0x2:\n bundleID=\"b\"";
    assert_eq!(ordered_bundle_ids(s).unwrap(), vec!["b", "a"]);
    assert!(ordered_bundle_ids("").unwrap().is_empty());
}
#[test]
fn environment_is_data() {
    let config = parse_environment(
        "JEV_API_KEY='example'\nJEV_MODEL=jev-latest\nPATH=$(touch /tmp/never)\nexport BAD=1",
    );
    assert_eq!(config["JEV_API_KEY"], "example");
    assert!(!config.contains_key("PATH"));
}
#[test]
fn generated_runtime_recovers_after_a_typed_function_error() {
    let mut e = Engine::offline();
    let s = Snapshot {
        current_app_id: "editor".into(),
        apps: vec![
            OpenApp {
                bundle_id: "editor".into(),
                name: "Cursor".into(),
                aliases: vec![],
                recency: 0,
                transitions: 0,
            },
            OpenApp {
                bundle_id: "slack".into(),
                name: "Slack".into(),
                aliases: vec![],
                recency: 1,
                transitions: 0,
            },
        ],
        recent_history: vec![],
        recent_activity: vec![],
        known_workflows: vec![],
        known_apps: vec![],
        recent_switches: vec![],
    };
    assert_eq!(
        e.choose(&s, "").unwrap().bundle_id.as_deref(),
        Some("slack")
    );
    assert!(e.choose(&s, "my team chat").is_err());
    assert_eq!(
        e.choose(&s, "Slack").unwrap().bundle_id.as_deref(),
        Some("slack")
    );
    assert!(e.choose(&s, &"x".repeat(3000)).is_err());
}

#[test]
fn history_save_uses_private_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let directory =
        std::env::temp_dir().join(format!("smart-switch-history-test-{}", std::process::id()));
    let file = directory.join("history.json");
    let mut history = History::default();
    history.record("editor").unwrap();
    history.save(&file).unwrap();
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(History::load(&file).activations, vec!["editor"]);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn duplicate_processes_do_not_duplicate_an_app() {
    let listing = "ASN:0x0-0x2: ASN:0x0-0x1:\n1) Editor ASN:0x0-0x1:\n bundleID=\"editor\"\n2) Editor ASN:0x0-0x2:\n bundleID=\"editor\"";
    assert_eq!(ordered_bundle_ids(listing).unwrap(), vec!["editor"]);
}

#[test]
fn generated_calls_preserve_multiline_input() {
    let mut engine = Engine::offline();
    let snapshot = Snapshot {
        current_app_id: "editor".into(),
        apps: vec![OpenApp {
            bundle_id: "slack".into(),
            name: "Slack".into(),
            aliases: vec![],
            recency: 1,
            transitions: 0,
        }],
        recent_history: vec![],
        recent_activity: vec![],
        known_workflows: vec![],
        known_apps: vec![],
        recent_switches: vec![],
    };
    assert_eq!(
        engine
            .choose(&snapshot, "open\nSlack")
            .unwrap()
            .bundle_id
            .as_deref(),
        Some("slack")
    );
    assert_eq!(
        engine
            .choose(&snapshot, "Slack")
            .unwrap()
            .bundle_id
            .as_deref(),
        Some("slack")
    );
}

#[test]
fn feedback_animation_is_bounded_and_eventually_finishes() {
    let start = ToastMotion::at(ms(0), false);
    assert_eq!(start.opacity, 0.0);
    assert_eq!(start.remaining, 1.0);
    assert!(!start.finished);
    let shown = ToastMotion::at(ms(200), false);
    assert_eq!(shown.opacity, 1.0);
    assert_eq!(shown.offset_y, 0.0);
    let mut previous = 1.0;
    for n in (0..=3500).step_by(16) {
        let frame = ToastMotion::at(ms(n), false);
        assert!((0.0..=1.0).contains(&frame.opacity));
        assert!((0.0..=previous).contains(&frame.remaining));
        previous = frame.remaining;
    }
    assert!(ToastMotion::at(ms(3200), false).finished);
}

#[test]
fn reduced_motion_uses_a_static_message() {
    for n in [0, 150, 1000, 3000] {
        let frame = ToastMotion::at(ms(n), true);
        assert_eq!(frame.opacity, 1.0);
        assert_eq!(frame.offset_y, 0.0);
        assert_eq!(frame.remaining, 1.0);
    }
    assert_eq!(ToastMotion::at(ms(3200), true).opacity, 0.0);
}

fn visit(id: &str, at: i64, seconds: i64) -> Visit {
    Visit {
        bundle_id: id.into(),
        name: id.into(),
        started: at,
        ended: at + seconds,
        active_ms: seconds * 1000,
    }
}
#[test]
fn workflow_sessions_keep_measured_evidence_without_inventing_a_label() {
    let start = local_day_start(unix_now()) + 3600;
    let mut activity = Activity::default();
    activity.visits = vec![
        visit("editor", start, 60),
        visit("browser", start + 60, 30),
        visit("editor", start + 91, 40),
        visit("chat", start + 1000, 60),
    ];
    let sessions = summary(&activity).sessions;
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].active_ms, 130000);
    assert_eq!(sessions[0].confidence, None);
    assert_eq!(sessions[0].title, "Session");
    assert!(sessions[0].evidence.contains("2 app changes"));
    assert_eq!(sessions[0].apps.len(), 2);
}
#[test]
fn workflow_labels_require_supported_confident_observations() {
    let mut activity = Activity::default();
    activity.visits = vec![visit("editor", 10000, 60)];
    for (label, confidence) in [("building", 0.69), ("invented", 0.99), ("building", 1.2)] {
        activity
            .record_workflow(
                10000,
                &WorkflowGuess {
                    label: label.into(),
                    confidence,
                },
            )
            .unwrap();
    }
    assert!(summary(&activity).sessions[0].confidence.is_none());
    activity
        .record_workflow(
            10000,
            &WorkflowGuess {
                label: "building".into(),
                confidence: 0.85,
            },
        )
        .unwrap();
    let session = summary(&activity).sessions.remove(0);
    assert_eq!(session.title, "Building");
    assert_eq!(session.confidence, Some(0.85));
}
#[test]
fn repeated_observed_workflows_become_prediction_context() {
    let start = local_day_start(unix_now()) + 3600;
    let mut activity = Activity::default();
    activity.visits = vec![
        visit("editor", start, 60),
        visit("browser", start + 60, 60),
        visit("browser", start + 2000, 60),
        visit("editor", start + 2060, 60),
    ];
    for at in [start, start + 2000] {
        activity
            .record_workflow(
                at,
                &WorkflowGuess {
                    label: "building".into(),
                    confidence: 0.9,
                },
            )
            .unwrap();
    }
    let hints = summary(&activity).known_workflows;
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].sessions, 2);
    assert_eq!(hints[0].apps, vec!["browser", "editor"]);
}
#[test]
fn historical_app_catalog_is_observation_only_and_deduplicated() {
    let mut activity = Activity::default();
    activity.visits = vec![
        visit("editor", 10000, 30),
        visit("browser", 10031, 20),
        visit("editor", 10052, 60),
    ];
    let known = summary(&activity).known_apps;
    assert_eq!(known.len(), 2);
    assert_eq!(known[0].bundle_id, "editor");
    assert_eq!(known[0].last_seen, 10112);
    let old: Activity =
        serde_json::from_str(r#"{"enabled":true,"visits":[],"decisions":[]}"#).unwrap();
    assert!(old.workflow_notes.is_empty());
    assert!(summary(&old).known_apps.is_empty());
}

#[test]
fn confident_workflow_changes_split_a_session_without_splitting_brief_checks() {
    let start = local_day_start(unix_now()) + 3600;
    let mut activity = Activity::default();
    activity.visits = vec![visit("editor", start, 120), visit("chat", start + 120, 120)];
    activity
        .record_workflow(
            start,
            &WorkflowGuess {
                label: "building".into(),
                confidence: 0.9,
            },
        )
        .unwrap();
    activity
        .record_workflow(
            start + 120,
            &WorkflowGuess {
                label: "communication".into(),
                confidence: 0.9,
            },
        )
        .unwrap();
    let sessions = summary(&activity).sessions;
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].title, "Building");
    assert_eq!(sessions[1].title, "Conversations");
    activity.visits[0].active_ms = 5000;
    assert_eq!(summary(&activity).sessions.len(), 1);
}

fn summary(activity: &Activity) -> ActivitySummary {
    let mut engine = Engine::offline();
    engine.summarize(activity, 0).unwrap()
}
#[test]
fn baml_owns_cycle_order_closed_app_skips_and_final_feedback() {
    let _guard = CYCLE_TEST.lock().unwrap();
    let mut engine = Engine::offline();
    let context = Snapshot {
        current_app_id: "origin".into(),
        apps: ["origin", "slack", "codex", "browser"]
            .iter()
            .enumerate()
            .map(|(i, id)| OpenApp {
                bundle_id: (*id).into(),
                name: (*id).into(),
                aliases: vec![],
                recency: i as i64,
                transitions: 0,
            })
            .collect(),
        recent_history: vec![],
        recent_activity: vec![],
        known_workflows: vec![],
        known_apps: vec![],
        recent_switches: vec![],
    };
    let first = engine
        .cycle_start(&context, "history", "cycle-test", 1000, &[], false)
        .unwrap()
        .unwrap();
    assert_eq!(first.choices[0].bundle_id, "slack");
    assert_eq!(first.choices.len(), 4);
    assert_eq!(first.visible_indices, vec![0, 1, 2, 3]);
    let open = vec!["slack".into(), "codex".into(), "browser".into()];
    let second = engine.cycle_next(&open, None).unwrap().unwrap();
    assert_eq!(second.choices[second.index as usize].bundle_id, "codex");
    let third = engine
        .cycle_next(&["slack".into(), "codex".into()], None)
        .unwrap()
        .unwrap();
    assert_eq!(third.choices[third.index as usize].bundle_id, "slack");
    engine.cycle_next(&open, Some(1)).unwrap();
    let event = engine.cycle_end(1002).unwrap().unwrap();
    assert_eq!(event.origin, "origin");
    assert_eq!(event.initial_target, "slack");
    assert_eq!(event.final_target, "codex");
    assert_eq!(event.advances, 3);
    assert!(engine.cycle_end(1003).unwrap().is_none());
}

#[test]
fn timed_pause_persists_and_never_charges_paused_time_after_resume() {
    let directory = std::env::temp_dir().join(format!("smart-switch-pause-{}", std::process::id()));
    let path = directory.join("activity.json");
    let mut activity = Activity::default();
    activity.observe(Some(("a", "A")), 1000, 200, 0.).unwrap();
    activity.observe(Some(("a", "A")), 1001, 1000, 0.).unwrap();
    activity.set_tracking(&TrackingState {
        enabled: false,
        paused_until: Some(1900),
    });
    activity.observe(Some(("b", "B")), 1500, 1000, 0.).unwrap();
    activity.save(&path).unwrap();
    let mut restored = Activity::load(&path);
    assert!(!restored.enabled);
    assert_eq!(restored.paused_until, Some(1900));
    assert_eq!(restored.visits.len(), 1);
    restored.set_tracking(&TrackingState {
        enabled: true,
        paused_until: None,
    });
    restored.observe(Some(("b", "B")), 1900, 1000, 0.).unwrap();
    assert_eq!(restored.visits[0].active_ms, 1000);
    assert_eq!(restored.visits[1].active_ms, 0);
    restored.observe(Some(("b", "B")), 1901, 1000, 0.).unwrap();
    assert_eq!(restored.visits[1].active_ms, 1000);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn generated_sdk_controls_pause_deadline_and_resume() {
    let mut engine = Engine::offline();
    let running = TrackingState {
        enabled: true,
        paused_until: None,
    };
    let paused = engine.tracking(&running, "15m", 1000).unwrap();
    assert!(!paused.enabled);
    assert_eq!(paused.paused_until, Some(1900));
    assert!(!engine.tracking(&paused, "tick", 1899).unwrap().enabled);
    assert!(engine.tracking(&paused, "tick", 1900).unwrap().enabled);
    assert!(engine.tracking(&paused, "resume", 1100).unwrap().enabled);
}

#[test]
fn preview_cancellation_never_creates_a_settled_switch_event() {
    let _guard = CYCLE_TEST.lock().unwrap();
    let mut engine = Engine::offline();
    let context = Snapshot {
        current_app_id: "origin".into(),
        apps: ["origin", "a", "b", "c", "d"]
            .iter()
            .enumerate()
            .map(|(i, id)| OpenApp {
                bundle_id: (*id).into(),
                name: (*id).into(),
                aliases: vec![],
                recency: i as i64,
                transitions: 0,
            })
            .collect(),
        recent_history: vec![],
        recent_activity: vec![],
        known_workflows: vec![],
        known_apps: vec![],
        recent_switches: vec![],
    };
    let ranked = vec!["d".into(), "b".into(), "not-open".into(), "d".into()];
    let plan = engine
        .cycle_start(&context, "model", "cancel-test", 1000, &ranked, false)
        .unwrap()
        .unwrap();
    assert_eq!(
        plan.choices
            .iter()
            .map(|app| app.bundle_id.as_str())
            .collect::<Vec<_>>(),
        vec!["d", "b", "a", "c", "origin"]
    );
    engine
        .cycle_next(&["a".into(), "b".into(), "c".into(), "d".into()], None)
        .unwrap();
    assert!(engine.cycle_cancel().unwrap().is_none());
    assert!(engine.cycle_end(1002).unwrap().is_none());
}

#[test]
fn settled_feedback_invalidates_a_cached_prediction_without_a_foreground_change() {
    let mut snapshot = Snapshot {
        current_app_id: "a".into(),
        apps: vec![],
        recent_history: vec![],
        recent_activity: vec![],
        known_workflows: vec![],
        known_apps: vec![],
        recent_switches: vec![],
    };
    let before = prediction_key(&snapshot);
    snapshot.recent_switches.push(SwitchEvent {
        first_activation_ms: None,
        settled_activation_ms: None,
        fast: None,
        learning: None,
        event_id: "correction".into(),
        started_at: 1000,
        ended_at: 1001,
        origin: "a".into(),
        initial_target: "b".into(),
        final_target: "a".into(),
        candidates: vec!["b".into(), "a".into()],
        advances: 1,
        source: "model".into(),
    });
    assert_ne!(prediction_key(&snapshot), before);
}

#[test]
fn generated_capture_types_roundtrip_and_direct_calls_need_no_transport() {
    let data = sdk::CaptureData {
        app: Some(sdk::CapturedApp::new("editor".into(), "Editor".into())),
        idle_seconds: Some(0.),
        ..sdk::CaptureData::new(true, 0, 100, 200)
    };
    let copy: sdk::CaptureData =
        serde_json::from_slice(&serde_json::to_vec(&data).unwrap()).unwrap();
    assert_eq!(copy, data);
    let result = sdk::capture_observation_sync(copy).unwrap();
    assert_eq!(result.append.unwrap().bundle_id, "editor");
}
