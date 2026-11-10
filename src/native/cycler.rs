use super::*;
const WINDOW: Duration = Duration::from_millis(1600);
pub(super) struct ActiveCycle {
    token: u64,
    plan: Option<CyclePlan>,
    last_tap: Instant,
    pending_replies: usize,
    panel: Option<Retained<AnyObject>>,
    applied: Option<String>,
    started: Instant,
    first_activation_ms: Option<i64>,
    mascot: Option<mascot::Mascot>,
    progress: Option<Retained<AnyObject>>,
    progress_width: f64,
}
impl ActiveCycle {
    pub(super) unsafe fn animate_mascot(&self) {
        if let Some(m) = self.mascot.as_ref() {
            m.animate();
        }
        if let Some(progress) = self.progress.as_ref() {
            let remaining =
                (1. - self.last_tap.elapsed().as_secs_f64() / WINDOW.as_secs_f64()).clamp(0., 1.);
            let _: () =
                msg_send![&**progress,setFrame:rect(0.,0.,self.progress_width*remaining,2.)];
        }
    }
    pub(super) fn previewing(&self) -> bool {
        self.applied.is_none() || self.plan.as_ref().is_none_or(|p| !p.fast)
    }
}
impl Controller {
    pub(super) unsafe fn begin_cycle(
        &self,
        context: Snapshot,
        source: &str,
        ranking: Vec<String>,
        fast: bool,
    ) {
        let (queued, commit) = {
            let state = self.ivars().borrow();
            (state.queued_cycle_taps, state.cycle_commit_pending)
        };
        self.cancel_cycle();
        CYCLE_VISIBLE.store(true, Ordering::Relaxed);
        let mut state = self.ivars().borrow_mut();
        state.generation += 1;
        let token = state.generation;
        state.cycle = Some(ActiveCycle {
            token,
            plan: None,
            last_tap: Instant::now(),
            pending_replies: 1,
            panel: None,
            applied: None,
            started: state.tap_started,
            first_activation_ms: None,
            mascot: None,
            progress: None,
            progress_width: 0.,
        });
        let event_id = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let _ = state.work.send(Work::CycleStart {
            token,
            context,
            fast,
            ranking,
            source: source.into(),
            event_id,
            at: unix_now(),
        });
        drop(state);
        for _ in 0..queued {
            self.send_cycle_input(None);
        }
        if commit {
            self.finish_cycle();
        }
    }
    pub(super) unsafe fn advance_cycle(&self) -> bool {
        self.expire_cycle();
        if self.ivars().borrow().cycle.is_none() {
            return false;
        }
        self.send_cycle_input(None);
        true
    }
    pub(super) unsafe fn pick_cycle(&self, index: usize) {
        self.send_cycle_input(Some(index));
    }
    unsafe fn send_cycle_input(&self, pick: Option<usize>) {
        let mut state = self.ivars().borrow_mut();
        let Some(c) = state.cycle.as_mut() else {
            return;
        };
        c.last_tap = Instant::now();
        c.pending_replies += 1;
        let token = c.token;
        let open = running().into_iter().map(|(id, _, _)| id).collect();
        let _ = state.work.send(Work::CycleNext { token, open, pick });
    }
    pub(super) unsafe fn expire_cycle(&self) {
        #[cfg(feature = "native-smoke")]
        if std::env::var("TAFO_SMOKE_HOLD_PREVIEW").as_deref() == Ok("1") {
            return;
        }

        if self.ivars().borrow().cycle.as_ref().is_some_and(|c| {
            c.plan.is_some() && c.pending_replies == 0 && c.last_tap.elapsed() >= WINDOW
        }) {
            self.finish_cycle();
        }
    }
    pub(super) unsafe fn finish_cycle(&self) {
        {
            let mut state = self.ivars().borrow_mut();
            if state.cycle.is_none()
                && state.busy
                && state
                    .active_request
                    .as_ref()
                    .is_some_and(|(_, text)| text.is_empty())
            {
                state.cycle_commit_pending = true;
                return;
            }
        }
        if let Some(c) = self.ivars().borrow_mut().cycle.as_mut() {
            if c.pending_replies > 0 {
                c.last_tap = Instant::now() - WINDOW;
                return;
            }
        }
        let Some(c) = self.ivars().borrow_mut().cycle.take() else {
            return;
        };
        if let Some(panel) = c.panel {
            let _: () = msg_send![&*panel,orderOut:NIL];
        }
        CYCLE_VISIBLE.store(false, Ordering::Relaxed);
        FEEDBACK_VISIBLE.store(false, Ordering::Relaxed);
        let committed = c
            .plan
            .as_ref()
            .and_then(|plan| plan.choices.get(plan.index as usize).map(|app| (plan, app)))
            .filter(|(_, app)| {
                c.applied.as_deref() == Some(app.bundle_id.as_str())
                    || activate(&app.bundle_id).is_ok()
            });
        if let Some((plan, app)) = committed {
            let mut state = self.ivars().borrow_mut();
            let selection = match sdk::cycle_selection_sync(plan) {
                Ok(selection) => selection,
                Err(error) => {
                    report_local_error(Err(error.to_string()));
                    return;
                }
            };
            report_local_error(state.activity.record_decision(
                unix_now(),
                &plan.context,
                &selection,
                selection.reason.clone(),
            ));
            let _ = state.activity.save(&storage_dir().join("activity.json"));
            let elapsed = c.started.elapsed().as_millis() as i64;
            let same = c.applied.as_deref() == Some(app.bundle_id.as_str());
            let first = c.first_activation_ms.unwrap_or(elapsed);
            let timing = SwitchTiming {
                first_activation_ms: Some(first),
                settled_activation_ms: Some(if same { first } else { elapsed }),
                fast: c.first_activation_ms.is_some(),
            };
            let _ = state.work.send(Work::CycleEnd {
                at: unix_now(),
                timing,
            });
        } else {
            let _ = self.ivars().borrow().work.send(Work::CycleCancel);
        }
    }
    pub(super) unsafe fn cancel_cycle(&self) {
        CYCLE_VISIBLE.store(false, Ordering::Relaxed);
        let c = {
            let mut state = self.ivars().borrow_mut();
            state.queued_cycle_taps = 0;
            state.cycle_commit_pending = false;
            state.generation += 1;
            state.busy = false;
            state.active_request = None;
            if let Some(submit) = &state.submit {
                let _: () = msg_send![&**submit,setEnabled:true];
            }
            state.cycle.take()
        };
        let Some(c) = c else {
            return;
        };
        if let Some(panel) = c.panel {
            let _: () = msg_send![&*panel,orderOut:NIL];
        }
        if c.applied.is_some() {
            if let Some(plan) = c.plan {
                let _ = activate(&plan.context.current_app_id);
            }
        }
        FEEDBACK_VISIBLE.store(false, Ordering::Relaxed);
        let _ = self.ivars().borrow().work.send(Work::CycleCancel);
    }
    pub(super) unsafe fn receive_cycle(&self) {
        let replies = self
            .ivars()
            .borrow()
            .cycle_results
            .try_iter()
            .collect::<Vec<_>>();
        for reply in replies {
            match reply {
                CycleResponse::Plan(token, Ok(Some(plan))) => {
                    let applicable = self
                        .ivars()
                        .borrow()
                        .cycle
                        .as_ref()
                        .is_some_and(|c| c.token == token);
                    if !applicable {
                        continue;
                    }
                    let optimistic = {
                        let state = self.ivars().borrow();
                        state
                            .cycle
                            .as_ref()
                            .is_some_and(|c| c.pending_replies == 1 && c.applied.is_none())
                            && plan.fast
                            && plan.advances == 0
                    };
                    let applied = if optimistic {
                        plan.choices
                            .get(plan.index as usize)
                            .filter(|app| activate(&app.bundle_id).is_ok())
                            .map(|app| app.bundle_id.clone())
                    } else {
                        None
                    };
                    if let Some(c) = self.ivars().borrow_mut().cycle.as_mut() {
                        c.pending_replies = c.pending_replies.saturating_sub(1);
                        if applied.is_some() {
                            c.first_activation_ms = Some(c.started.elapsed().as_millis() as i64);
                            c.applied = applied.clone();
                        }
                        c.plan = Some(plan);
                    }
                    let previewing = self
                        .ivars()
                        .borrow()
                        .cycle
                        .as_ref()
                        .is_some_and(|c| c.previewing());
                    if previewing {
                        self.show_cycle();
                    } else {
                        CYCLE_VISIBLE.store(false, Ordering::Relaxed);
                        if let Some(id) = applied {
                            self.show_toast(&id, "Your next app.");
                        }
                    }
                }
                CycleResponse::Plan(token, _) => {
                    if self
                        .ivars()
                        .borrow()
                        .cycle
                        .as_ref()
                        .is_some_and(|c| c.token == token)
                    {
                        self.cancel_cycle();
                    }
                }
                CycleResponse::Ended(Ok(Some(event))) => {
                    let mut state = self.ivars().borrow_mut();
                    if state.activity.enabled {
                        state.activity.switches.push(event);
                        let _ = state.activity.save(&storage_dir().join("activity.json"));
                    }
                }
                CycleResponse::Ended(_) => {}
            }
        }
    }
    unsafe fn show_cycle(&self) {
        let mut state = self.ivars().borrow_mut();
        if let Some(toast) = state.toast.take() {
            let _: () = msg_send![&*toast.window,orderOut:NIL];
        }
        let Some(active) = state.cycle.as_mut() else {
            return;
        };
        let Some(c) = active.plan.as_ref() else {
            return;
        };
        let design = theme::tokens();
        let inset = design.space.inset;
        let tile = design.switcher.tile;
        let step = tile + design.space.gap;
        let capacity = c.choices.len().min(7) as f64;
        let width = (inset * 2. + capacity * step - design.space.gap).max(360.);
        let height = design.switcher.height;
        let panel = if let Some(panel) = active.panel.take() {
            panel
        } else {
            let raw: *mut AnyObject = msg_send![class!(NSPanel), alloc];
            let panel = owned(
                msg_send![raw,initWithContentRect:rect(0.,0.,width,height),styleMask:128usize,backing:2usize,defer:false],
            );
            let _: () = msg_send![&*panel,setReleasedWhenClosed:false];
            let _: () = msg_send![&*panel,setHidesOnDeactivate:false];
            let _: () = msg_send![&*panel,setOpaque:false];
            let clear: *mut AnyObject = msg_send![class!(NSColor), clearColor];
            let _: () = msg_send![&*panel,setBackgroundColor:clear];
            let _: () = msg_send![&*panel,setHasShadow:true];
            let _: () = msg_send![&*panel,setLevel:3isize];
            let _: () = msg_send![&*panel,setTitle:&*ns("Tafo · Switch apps")];
            panel
        };
        let root = new_view("NSView", rect(0., 0., width, height));
        let parent = &*root as *const _ as *mut AnyObject;
        theme::surface(parent, design.radius.window);
        let _: () = msg_send![&*panel,setContentView:&*root];
        let selected = c
            .choices
            .get(c.index as usize)
            .map(|a| display_name(&a.bundle_id, &a.name))
            .unwrap_or_default();
        label(
            parent,
            &selected,
            rect(84., height - 53., width - 190., 30.),
            design.text.title,
            0.4,
            themed(Role::Ink),
        );
        let count = label(
            parent,
            &format!("{} / {}", c.index + 1, c.choices.len()),
            rect(width - 94., height - 47., width.min(70.), 20.),
            design.text.caption,
            0.,
            themed(Role::Sub),
        );
        let _: () = msg_send![&*count,setAlignment:2isize];
        active.mascot = Some(mascot::Mascot::new(
            parent,
            rect(20., height - 68., 56., 56.),
            if c.advances > 0 {
                mascot::Pose::Correct
            } else {
                mascot::Pose::Idle
            },
        ));
        let shown = c.visible_indices.len() as f64;
        let start = (width - (shown * step - design.space.gap)) / 2.;
        let open = running();
        for (slot_index, &index) in c.visible_indices.iter().enumerate() {
            let Some(app) = c.choices.get(index as usize) else {
                continue;
            };
            let slot = new_view(
                "NSButton",
                rect(start + slot_index as f64 * step, 42., tile, 100.),
            );
            let selected = index == c.index;
            let _: () = msg_send![&*slot,setTitle:&*ns("")];
            let _: () = msg_send![&*slot,setBordered:false];
            let _: () = msg_send![&*slot,setTarget:self];
            let _: () = msg_send![&*slot,setAction:sel!(cyclePick:)];
            let _: () = msg_send![&*slot,setTag:index as isize];
            let name = display_name(&app.bundle_id, &app.name);
            let _: () = msg_send![&*slot,setAccessibilityLabel:&*ns(&format!("{}{}",name,if selected {", selected"}else{""}))];
            rounded(
                &*slot as *const _ as *mut _,
                design.radius.card,
                themed(if selected {
                    Role::Selection
                } else {
                    Role::Paper
                }),
            );
            if selected {
                let layer: *mut AnyObject = msg_send![&*slot, layer];
                let border: *const c_void = msg_send![themed(Role::AccentSoft), CGColor];
                let _: () = msg_send![layer,setBorderWidth:1.0f64];
                let _: () = msg_send![layer,setBorderColor:border];
            }
            let _: () = msg_send![parent,addSubview:&*slot];
            if let Some((_, _, native)) = open.iter().find(|(id, _, _)| id == &app.bundle_id) {
                let image: *mut AnyObject = msg_send![*native, icon];
                let size = design.switcher.icon;
                let view = new_view("NSImageView", rect((tile - size) / 2., 12., size, size));
                let _: () = msg_send![&*view,setImage:image];
                let _: () = msg_send![&*view,setImageScaling:3usize];
                let _: () = msg_send![&*slot,addSubview:&*view];
            }
            let caption = label(
                &*slot as *const _ as *mut _,
                &name,
                rect(4., 72., tile - 8., 19.),
                design.text.caption,
                if selected { 0.4 } else { 0. },
                themed(if selected { Role::Ink } else { Role::Sub }),
            );
            let _: () = msg_send![&*caption,setAlignment:1isize];
            let _: () = msg_send![&*caption,setToolTip:&*ns(&name)];
        }
        let track = new_view("NSView", rect(inset, 20., width - inset * 2., 2.));
        rounded(&*track as *const _ as *mut _, 1., themed(Role::Rule));
        let progress = new_view("NSView", rect(0., 0., width - inset * 2., 2.));
        rounded(&*progress as *const _ as *mut _, 1., themed(Role::Accent));
        let _: () = msg_send![&*track,addSubview:&*progress];
        let _: () = msg_send![parent,addSubview:&*track];
        active.progress = Some(progress);
        active.progress_width = width - inset * 2.;
        let _: () = msg_send![&*panel,setContentSize:NSSize::new(width,height)];
        let screen: *mut AnyObject = msg_send![class!(NSScreen), mainScreen];
        let frame: NSRect = msg_send![screen, visibleFrame];
        let _: () = msg_send![&*panel,setFrameOrigin:NSPoint::new(frame.origin.x+(frame.size.width-width)/2.,frame.origin.y+frame.size.height*0.52-height/2.)];
        let _: () = msg_send![&*panel, orderFrontRegardless];
        active.panel = Some(panel);
        CYCLE_VISIBLE.store(true, Ordering::Relaxed);
        FEEDBACK_VISIBLE.store(true, Ordering::Relaxed);
    }
}
