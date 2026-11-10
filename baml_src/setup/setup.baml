enum SetupStage {
    Permission,
    Shortcut,
    Ready,
}

class SetupView {
    stage: SetupStage,
    title: string,
    action: string,
    detail: string,
}

function setup_trace() -> trace.Options throws never {
    trace.hidden()
}

/// baml:$trace=setup_trace
function setup_view(access: bool, tested: bool) -> SetupView throws never {
    let stage = match (access) {
        false => SetupStage.Permission,
        true if !tested => SetupStage.Shortcut,
        true => SetupStage.Ready,
    };
    SetupView {
        stage: stage,
        title: match (stage) {
            SetupStage.Permission => "Meet Tafo.",
            SetupStage.Shortcut => "Give it a tap.",
            SetupStage.Ready => "You're ready to move.",
        },
        action: match (stage) {
            SetupStage.Permission => "Open Settings",
            SetupStage.Shortcut => "Waiting for your key…",
            SetupStage.Ready => "Let’s go",
        },
        detail: match (stage) {
            SetupStage.Permission => "Enable Tafo in Accessibility.",
            SetupStage.Shortcut => "Press your switcher key, F19, or Control–Option–C.",
            SetupStage.Ready => "Tap to switch. Tap again for more. Hold to ask.",
        },
    }
}

test "setup requires permission even after a previous shortcut test" {
    assert.equal(setup_view(false, true).stage, SetupStage.Permission)
}

test "setup requires a real shortcut event before ready" {
    assert.equal(setup_view(true, false).stage, SetupStage.Shortcut);
    assert.equal(setup_view(true, true).stage, SetupStage.Ready)
}
