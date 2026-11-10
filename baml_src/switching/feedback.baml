// Presentation policy belongs to the decision engine. The native host renders it.
function selection_feedback(
    choice: Selection,
    snapshot: Snapshot,
    request: string,
) -> string throws never {
    match (choice.source) {
        "model" if !request.trim() && choice.reason != "" => choice.reason,
        "history" if choice.reason.starts_with("JEV ") => choice.reason,
        "history" => {
            let target = snapshot.apps.find((app) -> {
                app.bundle_id == choice.bundle_id
            });
            match (target) {
                let app: OpenApp if app.transitions >= 2 => {
                    let origin = snapshot.apps.find((candidate) -> {
                        candidate.bundle_id == snapshot.current_app_id
                    });
                    let name = match (origin) {
                        let prior: OpenApp => display_app(prior.bundle_id, prior.name),
                        null => "your previous app",
                    };
                    `You switched here ${app.transitions} times from ${name} recently.`
                },
                _ => "Your most recently used other app.",
            }
        },
        _ => {
            let text = whitespace_words(request).join(" ");
            if (!text) {
                return "Selected from your recent apps.";
            }
            let suffix = if (text.length() > 64) {
                "…"
            } else {
                ""
            };
            `Best match for “${text.slice(0, 64)}${suffix}”.`
        },
    }
}

test "selection feedback describes observed transitions and preserves model explanations" {
    let context = fixture();
    context.apps[1].transitions = 3;
    let choice = predict(context);
    assert.equal(
        selection_feedback(choice, context, ""),
        "You switched here 3 times from Cursor recently.",
    );
    choice.source = "model";
    choice.reason = "";
    assert.equal(
        selection_feedback(choice, context, "my team chat"),
        "Best match for “my team chat”.",
    );
    assert.equal(selection_feedback(choice, context, ""), "Selected from your recent apps.");
    choice.reason = "Observed recurring workflow";
    assert.equal(selection_feedback(choice, context, ""), choice.reason)
}

test "feedback normalizes whitespace and truncates by Unicode character" {
    let context = fixture();
    let choice = select_app(context, "browser");
    assert.equal(
        selection_feedback(choice, context, "  open\nSlack  "),
        "Best match for “open Slack”.",
    );
    let text = baml.Array.filled(100, "界").join("");
    let feedback = selection_feedback(choice, context, text);
    assert.equal(feedback.length() < 90, true);
    assert.contains(feedback, "…")
}

test "feedback preserves JEV history explanations and handles a missing origin" {
    let context = fixture();
    let choice = predict(context);
    choice.reason = "JEV ranked your workflow";
    assert.equal(selection_feedback(choice, context, ""), choice.reason);
    choice.reason = "Local history";
    context.apps[1].transitions = 2;
    context.current_app_id = "closed";
    assert.equal(
        selection_feedback(choice, context, ""),
        "You switched here 2 times from your previous app recently.",
    );
    choice.bundle_id = "closed";
    assert.equal(selection_feedback(choice, context, ""), "Your most recently used other app.")
}
