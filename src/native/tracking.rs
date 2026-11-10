use super::*;
use crate::TrackingState;
impl Controller {
    pub(super) unsafe fn change_tracking(&self, action: &str) {
        if self.ivars().borrow().tracking_pending {
            return;
        }
        let mut state = self.ivars().borrow_mut();
        let previous = TrackingState {
            enabled: state.activity.enabled,
            paused_until: state.activity.paused_until,
        };
        // Stop capture immediately; BAML resolves the requested deadline on its worker.
        if action != "tick" && action != "resume" {
            state.activity.set_tracking(&TrackingState {
                enabled: false,
                paused_until: None,
            });
            let _ = state.activity.save(&storage_dir().join("activity.json"));
        }
        state.prediction_cache = None;
        state.prediction_requested_key = None;
        state.tracking_pending = true;
        let _ = state.work.send(Work::Tracking {
            state: previous,
            action: action.into(),
            at: unix_now(),
        });
    }
    pub(super) unsafe fn receive_tracking(&self) {
        let results = self
            .ivars()
            .borrow()
            .tracking_results
            .try_iter()
            .collect::<Vec<_>>();
        let mut changed = false;
        for result in results {
            let mut state = self.ivars().borrow_mut();
            state.tracking_pending = false;
            match result {
                Ok(tracking) => {
                    state.activity.set_tracking(&tracking);
                    state.last_poll = Instant::now();
                    state.dashboard_saved = Instant::now();
                    state.prediction_status = if tracking.enabled {
                        "Learning from app order."
                    } else {
                        "Tracking paused."
                    }
                    .into();
                    let _ = state.activity.save(&storage_dir().join("activity.json"));
                }
                Err(_) => {
                    state.prediction_status =
                        "Could not update tracking. Try again from the menu.".into();
                }
            }
            changed = true;
        }
        let due = {
            let state = self.ivars().borrow();
            !state.tracking_pending
                && !state.activity.enabled
                && state
                    .activity
                    .paused_until
                    .is_some_and(|until| unix_now() >= until)
        };
        if due {
            self.change_tracking("tick");
        }
        if changed {
            let visible = self
                .ivars()
                .borrow()
                .activity_window
                .as_ref()
                .is_some_and(|window| {
                    let visible: bool = msg_send![&**window, isVisible];
                    visible
                });
            if visible {
                self.show_activity();
            }
            if let Some(status) = self.ivars().borrow().status.as_ref() {
                let button: *mut AnyObject = msg_send![&**status, button];
                let label = if self.ivars().borrow().activity.enabled {
                    "⇄"
                } else {
                    "⇄ ⏸"
                };
                let _: () = msg_send![button,setTitle:&*ns(label)];
            }
        }
    }
}
pub(super) unsafe fn tracking_menu(controller: &Controller) -> Retained<AnyObject> {
    let raw: *mut AnyObject = msg_send![class!(NSMenu), alloc];
    let menu = owned(msg_send![raw, init]);
    for (tag, title) in [
        "Pause for 15 minutes",
        "Pause for one hour",
        "Pause for today",
        "Pause until I resume",
        "Resume tracking",
    ]
    .iter()
    .enumerate()
    {
        let raw: *mut AnyObject = msg_send![class!(NSMenuItem), alloc];
        let item = owned(
            msg_send![raw,initWithTitle:&*ns(title),action:sel!(tracking:),keyEquivalent:&*ns("")],
        );
        let _: () = msg_send![&*item,setTarget:controller];
        let _: () = msg_send![&*item,setTag:tag as isize];
        let _: () = msg_send![&*menu,addItem:&*item];
    }
    menu
}
