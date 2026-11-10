enum SetupStage {
    Permission,
    Shortcut,
    Ready,
}

class SetupView {
    stage: SetupStage,
    title: string,
    detail: string,
    model: string,
    privacy: string,
}

function setup_trace() -> trace.Options throws never {
    trace.hidden()
}

/// baml:$trace=setup_trace
function setup_view(
    access: bool,
    tested: bool,
    model: bool,
    cloud: string,
) -> SetupView throws never {
    let stage = match (access) {
        false => SetupStage.Permission,
        true if !tested => SetupStage.Shortcut,
        true => SetupStage.Ready,
    };
    SetupView {
        stage: stage,
        title: match (stage) {
            SetupStage.Permission => "One permission. Then you're in.",
            SetupStage.Shortcut => "Give it a tap.",
            SetupStage.Ready => "You're ready to move.",
        },
        detail: match (stage) {
            SetupStage.Permission => {
                "Enable Smart Switch in Accessibility. If already on, turn it off and on again."
            },
            SetupStage.Shortcut => "Press your switcher key, F19, or Control–Option–C.",
            SetupStage.Ready => "Tap to switch. Tap again for more. Hold to ask.",
        },
        model: if (model) {
            "JEV connected"
        } else {
            "Local mode · add JEV whenever you like"
        },
        privacy: if (cloud) {
            `App activity and switch traces go to ${cloud}. Pause from the menu.`
        } else {
            "Activity stays on this Mac. Pause from the menu."
        },
    }
}

test "setup requires permission even after a previous shortcut test" {
    assert.equal(setup_view(false, true, true, "").stage, SetupStage.Permission)
}

test "setup requires a real shortcut event before ready" {
    assert.equal(setup_view(true, false, false, "dev").stage, SetupStage.Shortcut);
    assert.equal(setup_view(true, true, false, "dev").stage, SetupStage.Ready)
}

test "setup discloses cloud destination and optional model" {
    let local = setup_view(true, true, false, "");
    let release = setup_view(true, true, true, "Production");
    assert.contains(local.privacy, "this Mac");
    assert.contains(local.model, "Local mode");
    assert.contains(release.privacy, "Production");
    assert.equal(release.model, "JEV connected")
}
