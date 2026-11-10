use super::*;

unsafe fn tint(selector: Sel) -> *mut AnyObject {
    // All colors follow the user's macOS appearance.
    let color: *mut AnyObject = msg_send![class!(NSColor), performSelector:selector];
    color
}
unsafe fn text(
    parent: *mut AnyObject,
    value: &str,
    frame: NSRect,
    size: f64,
    weight: f64,
    secondary: bool,
) {
    let view = label(
        parent,
        value,
        frame,
        size,
        weight,
        tint(if secondary {
            sel!(secondaryLabelColor)
        } else {
            sel!(labelColor)
        }),
    );
    let _: () = msg_send![&*view,setToolTip:&*ns(value)];
}
unsafe fn fill(parent: *mut AnyObject, frame: NSRect, tint: *mut AnyObject, radius: f64) {
    let view = new_view("NSView", frame);
    rounded(&*view as *const _ as *mut _, radius, tint);
    let _: () = msg_send![parent,addSubview:&*view];
}
unsafe fn button(
    parent: *mut AnyObject,
    title: &str,
    frame: NSRect,
    action: Sel,
    target: &Controller,
    selected: bool,
) {
    let view = new_view("NSButton", frame);
    let _: () = msg_send![&*view,setTitle:&*ns(title)];
    if title == "↻" {
        let _: () = msg_send![&*view,setAccessibilityLabel:&*ns("Refresh")];
    }
    let _: () = msg_send![&*view,setBordered:false];
    let _: () = msg_send![&*view,setFont:font(12.,if selected {0.3} else {0.0})];
    let _: () = msg_send![&*view,setTarget:target];
    let _: () = msg_send![&*view,setAction:action];
    if selected {
        rounded(
            &*view as *const _ as *mut _,
            7.,
            tint(sel!(quaternaryLabelColor)),
        );
    }
    let _: () = msg_send![parent,addSubview:&*view];
}
unsafe fn icon(parent: *mut AnyObject, id: &str, name: &str, x: f64, y: f64, size: f64) {
    let image = running()
        .into_iter()
        .find(|(i, _, _)| i == id)
        .map(|(_, _, app)| {
            let image: *mut AnyObject = msg_send![app, icon];
            image
        });
    if let Some(image) = image.filter(|p| !p.is_null()) {
        let view = new_view("NSImageView", rect(x, y, size, size));
        let _: () = msg_send![&*view,setImage:image];
        let _: () = msg_send![&*view,setImageScaling:0usize];
        let _: () = msg_send![&*view,setAccessibilityLabel:&*ns(name)];
        let _: () = msg_send![parent,addSubview:&*view];
    } else {
        fill(
            parent,
            rect(x, y, size, size),
            tint(sel!(quaternaryLabelColor)),
            5.,
        );
        text(
            parent,
            &name.chars().take(1).collect::<String>(),
            rect(x + size * 0.25, y + size * 0.1, size * 0.7, size * 0.8),
            size * 0.55,
            0.3,
            false,
        );
    }
}
impl Controller {
    pub(super) unsafe fn render_activity(&self) {
        let mut state = self.ivars().borrow_mut();
        report_local_error(state.activity.prune(unix_now()));
        let summary = state.journal_summary.clone();
        let sessions = summary.sessions;
        let apps = summary.apps;
        let total = summary.total_ms;
        let fresh = state.activity_window.is_none();
        let window = if let Some(window) = &state.activity_window {
            window.clone()
        } else {
            let raw: *mut AnyObject = msg_send![class!(NSWindow), alloc];
            owned(
                msg_send![raw,initWithContentRect:rect(0.,0.,840.,600.),styleMask:3usize,backing:2usize,defer:false],
            )
        };
        let _: () = msg_send![&*window,setReleasedWhenClosed:false];
        let _: () = msg_send![&*window,setTitle:&*ns("Activity")];
        let _: () = msg_send![&*window,setContentSize:NSSize::new(840.,600.)];
        let root = new_view("NSVisualEffectView", rect(0., 0., 840., 600.));
        let _: () = msg_send![&*root,setMaterial:12isize];
        let _: () = msg_send![&*root,setBlendingMode:0isize];
        let _: () = msg_send![&*window,setContentView:&*root];
        let root = &*root as *const _ as *mut AnyObject;
        text(
            root,
            "Activity",
            rect(27., 543., 430., 32.),
            24.,
            0.5,
            false,
        );
        button(
            root,
            "Today",
            rect(604., 545., 76., 29.),
            sel!(today:),
            self,
            state.activity_today,
        );
        button(
            root,
            "30 days",
            rect(683., 545., 88., 29.),
            sel!(month:),
            self,
            !state.activity_today,
        );
        button(
            root,
            "↻",
            rect(782., 545., 30., 29.),
            sel!(activity:),
            self,
            false,
        );
        text(
            root,
            &duration(total),
            rect(27., 476., 240., 42.),
            31.,
            0.4,
            false,
        );
        text(
            root,
            "Active time",
            rect(29., 454., 230., 20.),
            11.,
            0.,
            true,
        );
        text(
            root,
            &apps.len().to_string(),
            rect(289., 480., 150., 35.),
            26.,
            0.3,
            false,
        );
        text(root, "Apps", rect(290., 454., 140., 20.), 11., 0., true);
        let next = state
            .prediction_cache
            .as_ref()
            .and_then(|(_, c)| c.bundle_id.as_ref())
            .and_then(|id| running().into_iter().find(|(i, _, _)| i == id));
        if let Some((id, name, _)) = next {
            icon(root, &id, &name, 579., 483., 27.);
            text(
                root,
                &display_name(&id, &name),
                rect(614., 482., 190., 29.),
                17.,
                0.3,
                false,
            );
        } else {
            text(root, "—", rect(580., 482., 200., 29.), 19., 0., true);
        }
        text(
            root,
            "Last suggestion",
            rect(580., 454., 225., 20.),
            11.,
            0.,
            true,
        );
        fill(
            root,
            rect(28., 434., 784., 0.5),
            tint(sel!(separatorColor)),
            0.,
        );
        text(
            root,
            "Sessions",
            rect(28., 394., 510., 25.),
            14.,
            0.4,
            false,
        );
        text(root, "Apps", rect(580., 394., 230., 25.), 14., 0.4, false);
        fill(
            root,
            rect(555., 43., 0.5, 365.),
            tint(sel!(separatorColor)),
            0.,
        );
        let scroll = new_view("NSScrollView", rect(22., 48., 523., 335.));
        let _: () = msg_send![&*scroll,setHasVerticalScroller:true];
        let _: () = msg_send![&*scroll,setScrollerStyle:1isize];
        let _: () = msg_send![&*scroll,setAutohidesScrollers:true];
        let _: () = msg_send![&*scroll,setDrawsBackground:false];
        let height = (sessions.len().clamp(1, 100) as f64 * 82.).max(335.);
        let document = new_view("NSView", rect(0., 0., 505., height));
        let doc = &*document as *const _ as *mut AnyObject;
        if sessions.is_empty() {
            text(
                doc,
                "No activity yet",
                rect(8., height - 42., 485., 25.),
                14.,
                0.,
                true,
            );
        }
        for (index, session) in sessions.iter().rev().take(100).enumerate() {
            let y = height - index as f64 * 82.;
            let session_apps = session.apps.clone();
            let title = if session.confidence.is_some() {
                session.title.clone()
            } else {
                session_apps
                    .iter()
                    .take(3)
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" + ")
            };
            fill(
                doc,
                rect(9., y - 25., 5., 5.),
                tint(sel!(tertiaryLabelColor)),
                2.5,
            );
            text(doc, &title, rect(29., y - 36., 322., 27.), 14., 0.3, false);
            text(
                doc,
                &format!(
                    "{} · {}",
                    if state.activity_today {
                        short_time(session.started)
                    } else {
                        local_time(session.started).chars().take(12).collect()
                    },
                    duration(session.active_ms)
                ),
                rect(30., y - 59., 325., 20.),
                11.,
                0.,
                true,
            );
            for (i, app) in session_apps.iter().take(3).enumerate() {
                icon(doc, &app.id, &app.name, 410. + i as f64 * 28., y - 36., 23.);
            }
            if let Some(confidence) = session.confidence {
                let badge = label(
                    doc,
                    "≈",
                    rect(363., y - 32., 24., 23.),
                    14.,
                    0.,
                    tint(sel!(tertiaryLabelColor)),
                );
                let _: () = msg_send![&*badge,setToolTip:&*ns(&format!("Inferred from app activity · {:.0}% confidence\n{}",confidence*100.,session.evidence))];
            }
            fill(
                doc,
                rect(29., y - 76., 468., 0.5),
                tint(sel!(separatorColor)),
                0.,
            );
        }
        let _: () = msg_send![&*scroll,setDocumentView:&*document];
        let _: () = msg_send![root,addSubview:&*scroll];
        let clip: *mut AnyObject = msg_send![&*scroll, contentView];
        let _: () = msg_send![clip,scrollToPoint:NSPoint::new(0.,height-335.)];
        let _: () = msg_send![&*scroll,reflectScrolledClipView:clip];
        for (index, app) in apps.iter().take(6).enumerate() {
            let y = 353. - index as f64 * 49.;
            icon(root, &app.id, &app.name, 581., y - 2., 24.);
            text(
                root,
                &app.name,
                rect(614., y + 6., 116., 21.),
                12.,
                0.2,
                false,
            );
            text(
                root,
                &duration(app.active_ms),
                rect(743., y + 6., 69., 21.),
                11.,
                0.,
                true,
            );
            fill(
                root,
                rect(615., y - 5., 194., 3.),
                tint(sel!(quaternaryLabelColor)),
                1.5,
            );
            if total > 0 {
                fill(
                    root,
                    rect(615., y - 5., 194. * app.active_ms as f64 / total as f64, 3.),
                    tint(sel!(controlAccentColor)),
                    1.5,
                );
            }
        }
        let tracking_label = if state.activity.enabled {
            "Recording".to_owned()
        } else if let Some(until) = state.activity.paused_until {
            format!("Paused until {}", short_time(until))
        } else {
            "Paused".to_owned()
        };
        text(
            root,
            &tracking_label,
            rect(29., 14., 160., 20.),
            10.,
            0.,
            true,
        );
        button(
            root,
            if state.activity.enabled {
                "Pause 15m"
            } else {
                "Resume"
            },
            rect(712., 10., 100., 26.),
            sel!(recording:),
            self,
            false,
        );
        if fresh {
            let _: () = msg_send![&*window, center];
        }
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app,activateIgnoringOtherApps:true];
        let _: () = msg_send![&*window,makeKeyAndOrderFront:NIL];
        state.activity_window = Some(window);
        drop(state);
        self.review_workflow();
    }
    pub(super) fn request_activity_summary(&self) {
        let mut state = self.ivars().borrow_mut();
        if state.journal_pending && state.journal_requested_scope == state.activity_today {
            return;
        }
        state.journal_pending = true;
        state.journal_requested_scope = state.activity_today;
        let since = if state.activity_today {
            local_day_start(unix_now())
        } else {
            0
        };
        let _ = state.work.send(Work::Summarize {
            activity: state.activity.clone(),
            since,
            today: state.activity_today,
        });
    }
    unsafe fn review_workflow(&self) {
        let mut state = self.ivars().borrow_mut();
        if !state.activity.enabled || state.prediction_pending || state.activity.visits.is_empty() {
            return;
        }
        if let Ok(mut context) = snapshot(&state.history) {
            context.recent_activity = state
                .activity
                .visits
                .iter()
                .rev()
                .take(16)
                .cloned()
                .collect();
            context.recent_switches = state
                .activity
                .switches
                .iter()
                .rev()
                .take(16)
                .cloned()
                .collect();
            let key = prediction_key(&context);
            if state.prediction_requested_key.as_ref() != Some(&key)
                && state.prediction_tx.try_send(context).is_ok()
            {
                state.prediction_requested_key = Some(key);
                state.prediction_pending = true;
            }
        }
    }
}
