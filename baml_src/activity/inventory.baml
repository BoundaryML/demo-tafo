// The host collects the OS inventory; BAML interprets it and supplies ordering.

function process_order(listing: string) -> string[] throws never {
    let lines = listing.lines();
    let order = whitespace_words(lines.at(0) ?? "");
    let apps: map<string, string> = {};
    let current: string? = null;
    for (let line in lines.slice(1, lines.length())) {
        let process = line.split("ASN:0x").at(1);
        let bundle = line.split("bundleID=\"").at(1);
        if (process != null) {
            current = "ASN:0x" + (whitespace_words(process).at(0) ?? "");
        } else if (bundle != null && current != null) {
            apps.set(current, bundle.split("\"").at(0) ?? "");
        }
    }
    let seen: map<string, bool> = {};
    order
        .filter_map((process) -> {
            apps.get(process)
        })
        .filter((id) -> {
            if (seen.has(id)) {
                return false;
            }
            seen.set(id, true);
            true
        })
}

class SnapshotSeed {
    current_app_id: string,
    own_app_id: string,
    apps: CapturedApp[],
    activations: string[],
    process_listing: string,
    recent_activity: AppVisit[],
    recent_switches: SwitchEvent[],
}

function snapshot_context(data: SnapshotSeed) -> Snapshot throws never {
    let history = data.activations.reverse();
    let order = process_order(data.process_listing);
    let apps = data.apps
        .map((app) -> {
            OpenApp {
                bundle_id: app.id,
                name: display_app(app.id, app.name),
                aliases: if (app.id == "com.openai.codex") {
                    ["Codex"]
                } else {
                    []
                },
                transitions: 0,
                recency: history.find_index((id) -> {
                    id == app.id
                })
                    ?? (
                        1000
                            + (
                                order.find_index((id) -> {
                                    id == app.id
                                })
                                    ?? 1000
                            )
                    ),
            }
        })
        .sort_by((a, b) -> {
            if (a.recency != b.recency) {
                a.recency.cmp(b.recency)
            } else {
                a.bundle_id.cmp(b.bundle_id)
            }
        });
    Snapshot {
        current_app_id: if (!data.current_app_id || data.current_app_id == data.own_app_id) {
            history.at(0) ?? ""
        } else {
            data.current_app_id
        },
        apps: apps,
        recent_history: history.slice(0, MAX_ACTIVATIONS),
        known_apps: [],
        known_workflows: [],
        recent_activity: data.recent_activity,
        recent_switches: data.recent_switches,
    }
}

test "process inventory ordering deduplicates apps and tolerates incomplete records" {
    let listing = `
        ASN:0x0-0x2: ASN:0x0-0x1: ASN:0x0-0x3:
        1) A ASN:0x0-0x1:
         bundleID="a"
        2) B ASN:0x0-0x2:
         bundleID="b"
        3) A ASN:0x0-0x3:
         bundleID="a"
    `;
    assert.equal(process_order(listing), ["b", "a"]);
    assert.equal(process_order(""), [])
}

test "inventory uses observed recency and resolves host focus and native names" {
    let data = SnapshotSeed {
        current_app_id: "switcher",
        own_app_id: "switcher",
        apps: [
            CapturedApp { id: "com.openai.codex", name: "Codex Helper" },
            CapturedApp { id: "editor", name: "Editor" },
            CapturedApp { id: "browser", name: "Browser" },
        ],
        activations: ["editor", "com.openai.codex"],
        process_listing: "",
        recent_activity: [],
        recent_switches: [],
    };
    let context = snapshot_context(data);
    assert.equal(context.current_app_id, "com.openai.codex");
    assert.equal(context.apps[0].name, "Codex");
    assert.equal(context.apps[0].aliases, ["Codex"]);
    assert.equal(context.apps[1].bundle_id, "editor");
    assert.equal(context.apps[2].recency, 2000);
    data.current_app_id = "browser";
    assert.equal(snapshot_context(data).current_app_id, "browser")
}
