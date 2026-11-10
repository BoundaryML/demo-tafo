function fixture() -> Snapshot throws never {
    Snapshot {
        current_app_id: "editor",
        apps: [
            OpenApp { bundle_id: "editor", name: "Cursor", aliases: [], recency: 0, transitions: 0 },
            OpenApp {
                bundle_id: "browser",
                name: "Arc",
                aliases: ["browser"],
                recency: 1,
                transitions: 1,
            },
            OpenApp { bundle_id: "slack", name: "Slack", aliases: [], recency: 2, transitions: 0 },
            OpenApp {
                bundle_id: "com.openai.codex",
                name: "ChatGPT",
                aliases: ["Codex"],
                recency: 3,
                transitions: 0,
            },
        ],
        recent_history: ["editor", "browser", "editor"],
        recent_activity: [],
        known_workflows: [],
        known_apps: [],
        recent_switches: [],
    }
}

test "tap uses MRU and excludes the current app" {
    assert.equal(select_app(fixture(), "").bundle_id, "browser")
}

test "old transition frequency cannot bury recent foreground activity" {
    let s = fixture();
    s.apps[2].transitions = 3;
    assert.equal(predict(s).bundle_id, "browser")
}

test "recency breaks learned workflow ties" {
    let s = fixture();
    s.apps[1].transitions = 3;
    s.apps[2].transitions = 3;
    assert.equal(predict(s).bundle_id, "browser")
}

test "one app and no apps do not switch" {
    let s = fixture();
    s.apps = s.apps.slice(0, 1);
    assert.equal(predict(s).bundle_id, null);
    s.apps = [];
    assert.equal(predict(s).bundle_id, null)
}

test "app names and Codex alias resolve without credentials" {
    assert.equal(select_app(fixture(), "Switch to Slack, please.").bundle_id, "slack");
    assert.equal(select_app(fixture(), "codex").bundle_id, "com.openai.codex");
    assert.equal(select_app(fixture(), "browser").bundle_id, "browser")
}

test "substring is not an app-name match" {
    assert.equal(named_apps(fixture(), "search my notes").length(), 0)
}

test "multiple named apps request clarification" {
    assert.equal(select_app(fixture(), "Arc or Slack").bundle_id, null)
}

test "an explicit request may select the current app" {
    assert.equal(select_app(fixture(), "Cursor").bundle_id, "editor")
}

test "model output is constrained to the open apps" {
    let s = fixture();
    assert.equal(
        validate_choice(
            ModelChoice {
                fast_candidates: [],
                scores: {},
                workflow: null,
                bundle_id: "unknown",
                confidence: 0.99,
                reason: "invented",
            },
            s,
        ).bundle_id,
        null,
    );
    assert.equal(
        validate_choice(
            ModelChoice {
                fast_candidates: [],
                scores: {},
                workflow: null,
                bundle_id: "slack",
                confidence: 0.2,
                reason: "unsure",
            },
            s,
        ).bundle_id,
        null,
    );
    assert.equal(
        validate_choice(
            ModelChoice {
                fast_candidates: [],
                scores: {},
                workflow: null,
                bundle_id: "slack",
                confidence: 4.0,
                reason: "invalid",
            },
            s,
        ).bundle_id,
        null,
    );
    assert.equal(
        validate_choice(
            ModelChoice {
                fast_candidates: [],
                scores: {},
                workflow: null,
                bundle_id: null,
                confidence: 0.9,
                reason: "none",
            },
            s,
        ).bundle_id,
        null,
    );
    assert.equal(
        validate_choice(
            ModelChoice {
                fast_candidates: [],
                scores: {},
                workflow: null,
                bundle_id: "slack",
                confidence: 0.9,
                reason: "team chat",
            },
            s,
        ).bundle_id,
        "slack",
    )
}

test "saved Jev reply parses offline" {
    let choice = parse_jev_reply(
        `{"answers":{"app":{"choice":"slack","confidence":0.95,"probabilities":{"slack":0.98,"__no_app__":0.02}}}}`,
    );
    assert.equal(validate_choice(choice, fixture()).bundle_id, "slack")
}

test "Jev request lists open apps and a no-match choice" {
    let request = jev_request(fixture(), "my team chat");
    let question = request.questions.get("app");
    assert.equal(question is JevQuestion, true);
    assert.contains(baml.json.stringify(request), "slack");
    assert.contains(baml.json.stringify(request), "__no_app__")
}

test "Jev no-match and low-confidence replies do not switch" {
    let choice = parse_jev_reply(
        `{"answers":{"app":{"choice":"__no_app__","confidence":0.99,"probabilities":{"__no_app__":0.99}}}}`,
    );
    assert.equal(validate_choice(choice, fixture()).bundle_id, null)
}

test "request text cannot become a shell command" {
    assert.equal(named_apps(fixture(), "$(touch /tmp/owned)").length(), 0)
}

test "workflow model can choose beyond the most recent app" {
    let choice = validate_prediction(
        ModelChoice {
            fast_candidates: [],
            scores: {},
            workflow: null,
            bundle_id: "slack",
            confidence: 0.91,
            reason: "",
        },
        fixture(),
    );
    assert.equal(choice.bundle_id, "slack");
    assert.equal(choice.source, "model");
    assert.equal(choice.ranking.at(0), "slack")
}

test "uncertain workflow predictions retain the local default" {
    let s = fixture();
    assert.equal(
        validate_prediction(
            ModelChoice {
                fast_candidates: [],
                scores: {},
                workflow: null,
                bundle_id: "slack",
                confidence: 0.79,
                reason: "",
            },
            s,
        ).bundle_id,
        "browser",
    );
    assert.equal(
        validate_prediction(
            ModelChoice {
                fast_candidates: [],
                scores: {},
                workflow: null,
                bundle_id: "editor",
                confidence: 0.99,
                reason: "",
            },
            s,
        ).source,
        "history",
    );
    assert.equal(
        validate_prediction(
            ModelChoice {
                fast_candidates: [],
                scores: {},
                workflow: null,
                bundle_id: "closed",
                confidence: 0.99,
                reason: "",
            },
            s,
        ).source,
        "history",
    );
    assert.equal(
        validate_prediction(
            ModelChoice {
                fast_candidates: [],
                scores: {},
                workflow: null,
                bundle_id: null,
                confidence: 0.99,
                reason: "",
            },
            s,
        ).source,
        "history",
    )
}

test "workflow inference is independent from destination confidence" {
    let choice = parse_jev_reply(
        `{"answers":{"app":{"choice":"slack","confidence":0.5,"probabilities":{}},"workflow":{"choice":"building","confidence":0.88,"probabilities":{}}}}`,
    );
    let result = validate_prediction(choice, fixture());
    assert.equal(result.source, "history");
    assert.equal(result.workflow?.label, "building")
}

test "uncertain or unsupported workflow labels stay unknown" {
    let low = parse_jev_reply(
        `{"answers":{"app":{"choice":"slack","confidence":0.9,"probabilities":{}},"workflow":{"choice":"research","confidence":0.4,"probabilities":{}}}}`,
    );
    let invalid = parse_jev_reply(
        `{"answers":{"app":{"choice":"slack","confidence":0.9,"probabilities":{}},"workflow":{"choice":"secret_project","confidence":0.99,"probabilities":{}}}}`,
    );
    assert.equal(low.workflow, null);
    assert.equal(invalid.workflow, null)
}

test "historical apps cannot become closed switch destinations" {
    let s = fixture();
    s.known_apps = [KnownApp { bundle_id: "closed", name: "Old app", last_seen: 1000 }];
    assert.equal(
        validate_prediction(
            ModelChoice {
                fast_candidates: [],
                scores: {},
                workflow: null,
                bundle_id: "closed",
                confidence: 0.99,
                reason: "",
            },
            s,
        ).source,
        "history",
    )
}

test "JEV scores order all open alternatives and ignore closed destinations" {
    let snapshot = fixture();
    let choice = parse_jev_reply(
        `{"answers":{"app":{"choice":"slack","confidence":0.5,"probabilities":{"slack":0.5,"browser":0.5}},"rank:slack":{"score":3.0,"confidence":0.9,"probabilities":{"3":1.0}},"rank:browser":{"score":0.0,"confidence":0.9,"probabilities":{"0":1.0}},"rank:com.openai.codex":{"score":2.5,"confidence":0.8,"probabilities":{"2":0.5,"3":0.5}},"rank:closed":{"score":3.0,"confidence":1.0,"probabilities":{"3":1.0}}}}`,
    );
    let result = validate_prediction(choice, snapshot);
    assert.equal(result.ranking, ["slack", "com.openai.codex", "browser"]);
    assert.equal(result.bundle_id, "slack");
    assert.equal(result.source, "model")
}

test "uncertain invalid and missing relevance scores fall back to observed recency" {
    let snapshot = fixture();
    let choice = parse_jev_reply(
        `{"answers":{"app":{"choice":"__no_app__","confidence":0.2,"probabilities":{"__no_app__":1.0}},"rank:slack":{"score":3.0,"confidence":0.1,"probabilities":{}},"rank:browser":{"score":99.0,"confidence":0.9,"probabilities":{}}}}`,
    );
    assert.equal(choice.scores.length(), 0);
    let result = validate_prediction(choice, snapshot);
    assert.equal(result.bundle_id, "browser");
    assert.equal(result.ranking.length(), 3)
}

test "observed history beats process enumeration and stale high counts stay bounded" {
    let snapshot = fixture();
    snapshot.recent_history = ["editor", "com.openai.codex", "slack", "browser"];
    snapshot.apps[1].transitions = 10000;
    assert.equal(ranked_apps(snapshot)[0].bundle_id, "com.openai.codex")
}

test "background request scores each app while text requests avoid irrelevant scoring" {
    let snapshot = fixture();
    let background = jev_request(snapshot, "");
    assert.equal(background.questions.get("rank:slack") is JevScoreQuestion, true);
    assert.equal(background.questions.get("rank:com.openai.codex") is JevScoreQuestion, true);
    assert.equal(background.questions.get("rank:editor"), null);
    assert.equal(jev_request(snapshot, "Slack").questions.get("rank:slack"), null)
}

function fast_fixture_choice() -> ModelChoice throws never {
    ModelChoice {
        fast_candidates: ["browser"],
        scores: { "browser": 3.0, "slack": 0.0, "com.openai.codex": 0.0 },
        bundle_id: "browser",
        confidence: 0.99,
        reason: "",
        workflow: null,
    }
}

test "fast activation requires agreement high confidence and a clear lead" {
    let context = fixture();
    let choice = fast_fixture_choice();
    assert.equal(validate_prediction(choice, context).fast, true);
    choice.confidence = 0.7;
    assert.equal(validate_prediction(choice, context).fast, false);
    choice.confidence = 0.99;
    choice.fast_candidates = [];
    assert.equal(validate_prediction(choice, context).fast, false);
    choice.fast_candidates = ["browser"];
    choice.bundle_id = "slack";
    assert.equal(validate_prediction(choice, context).fast, false);
    choice.bundle_id = "browser";
    context.recent_history = [];
    choice.scores = { "browser": 3.0, "slack": 3.0, "com.openai.codex": 0.0 };
    assert.equal(validate_prediction(choice, context).fast, false)
}

test "a rapid corrected initial choice loses priority and disables its fast path" {
    let context = fixture();
    let choice = fast_fixture_choice();
    let before = app_priority(context, context.apps[1], choice.scores);
    context.recent_switches
        = [
            SwitchEvent {
                first_activation_ms: null,
                settled_activation_ms: null,
                fast: null,
                learning: null,
                event_id: "correction",
                started_at: 1000,
                ended_at: 1001,
                origin: "editor",
                initial_target: "browser",
                final_target: "slack",
                candidates: ["browser", "slack"],
                advances: 1,
                source: "model",
            },
        ];
    assert.equal(validate_prediction(choice, context).fast, false);
    assert.equal(app_priority(context, context.apps[1], choice.scores) < before, true)
}

test "Score confidence gates the immediate path independently of the next-app confidence" {
    let choice = parse_jev_reply(
        `{"answers":{"app":{"choice":"browser","confidence":0.99,"probabilities":{"browser":1.0}},"rank:browser":{"score":3.0,"confidence":0.6,"probabilities":{"3":1.0}}}}`,
    );
    assert.equal(choice.scores.get("browser"), 3.0);
    assert.equal(validate_prediction(choice, fixture()).fast, false)
}
