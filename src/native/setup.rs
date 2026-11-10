use super::journal::{button, fill, text, tint};
use super::*;

pub(super) struct SetupWindow {
    window: Retained<AnyObject>,
    title: Retained<AnyObject>,
    detail: Retained<AnyObject>,
    done: Retained<AnyObject>,
    tested: bool,
    checked: Instant,
}

impl Controller {
    pub(super) unsafe fn open_access_settings(&self) {
        let enabled: *mut AnyObject = msg_send![class!(NSNumber),numberWithBool:true];
        let options: *mut AnyObject = msg_send![class!(NSDictionary),dictionaryWithObject:enabled,forKey:kAXTrustedCheckOptionPrompt];
        AXIsProcessTrustedWithOptions(options as *const c_void);
        let url: *mut AnyObject = msg_send![class!(NSURL),URLWithString:&*ns("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")];
        let _: bool = msg_send![workspace(),openURL:url];
    }

    pub(super) unsafe fn show_setup(&self) {
        if let Some(setup) = self.ivars().borrow().setup.as_ref() {
            let _: () = msg_send![&*setup.window,makeKeyAndOrderFront:NIL];
            return;
        }
        self.hide(true);
        let raw: *mut AnyObject = msg_send![class!(NSWindow), alloc];
        let window = owned(
            msg_send![raw,initWithContentRect:rect(0.,0.,540.,450.),styleMask:1usize,backing:2usize,defer:false],
        );
        let _: () = msg_send![&*window,setReleasedWhenClosed:false];
        let _: () = msg_send![&*window,setTitle:&*ns("Welcome to Tafo")];
        let _: () = msg_send![&*window,setTitlebarAppearsTransparent:true];
        let _: () = msg_send![&*window,setTitleVisibility:1isize];
        let root = new_view("NSView", rect(0., 0., 540., 450.));
        let parent = &*root as *const _ as *mut AnyObject;
        fill(
            parent,
            rect(40., 342., 52., 52.),
            color(0.85, 0.93, 0.88),
            15.,
        );
        label(
            parent,
            "⇄",
            rect(47., 348., 40., 40.),
            32.,
            0.4,
            color(0.15, 0.36, 0.26),
        );
        text(parent, "TAFO", rect(108., 360., 160., 22.), 13., 0.5, true);
        let title = label(
            parent,
            "",
            rect(40., 284., 468., 42.),
            26.,
            0.4,
            tint(sel!(labelColor)),
        );
        let detail = label(
            parent,
            "",
            rect(40., 231., 460., 46.),
            14.,
            0.,
            tint(sel!(secondaryLabelColor)),
        );
        let _: () = msg_send![&*detail,setMaximumNumberOfLines:2isize];
        button(
            parent,
            "Open Accessibility Settings",
            rect(36., 177., 260., 34.),
            sel!(access:),
            self,
            true,
        );
        button(
            parent,
            "Model settings…",
            rect(320., 177., 178., 34.),
            sel!(modelSetup:),
            self,
            false,
        );
        text(
            parent,
            "Hold your key to type or dictate a destination.",
            rect(40., 132., 460., 22.),
            12.,
            0.,
            true,
        );
        let model_ready = std::env::var("JEV_API_KEY")
            .or_else(|_| std::env::var("TYPESAFE_API_KEY"))
            .is_ok_and(|v| !v.is_empty());
        let cloud = if cloud_tracing_enabled() {
            option_env!("TAFO_CLOUD_LABEL").unwrap_or("your Cloud project")
        } else {
            ""
        };
        let view = sdk::setup_view_sync(
            ACCESS.load(Ordering::Relaxed),
            false,
            model_ready,
            cloud.into(),
        );
        if let Ok(view) = view {
            text(
                parent,
                &view.model,
                rect(40., 105., 460., 20.),
                12.,
                0.,
                true,
            );
            let privacy = label(
                parent,
                &view.privacy,
                rect(40., 52., 306., 42.),
                11.,
                0.,
                tint(sel!(secondaryLabelColor)),
            );
            let _: () = msg_send![&*privacy,setMaximumNumberOfLines:2isize];
        }
        let done = new_view("NSButton", rect(365., 56., 135., 34.));
        let _: () = msg_send![&*done,setTitle:&*ns("Start switching")];
        let _: () = msg_send![&*done,setBezelStyle:1usize];
        let _: () = msg_send![&*done,setTarget:self];
        let _: () = msg_send![&*done,setAction:sel!(finishSetup:)];
        let _: () = msg_send![&*done,setKeyEquivalent:&*ns("\r")];
        let _: () = msg_send![parent,addSubview:&*done];
        let _: () = msg_send![&*window,setContentView:&*root];
        let _: () = msg_send![&*window, center];
        let _: () = msg_send![&*window,makeKeyAndOrderFront:NIL];
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app,activateIgnoringOtherApps:true];
        self.ivars().borrow_mut().setup = Some(SetupWindow {
            window,
            title,
            detail,
            done,
            tested: false,
            checked: Instant::now(),
        });
        self.render_setup();
    }

    unsafe fn render_setup(&self) {
        let state = self.ivars().borrow();
        let Some(setup) = state.setup.as_ref() else {
            return;
        };
        match sdk::setup_view_sync(
            ACCESS.load(Ordering::Relaxed),
            setup.tested,
            false,
            String::new(),
        ) {
            Ok(view) => {
                let _: () = msg_send![&*setup.title,setStringValue:&*ns(&view.title)];
                let _: () = msg_send![&*setup.detail,setStringValue:&*ns(&view.detail)];
                let _: () =
                    msg_send![&*setup.done,setEnabled:matches!(view.stage,sdk::SetupStage::Ready)];
            }
            Err(_) => {
                let _: () = msg_send![&*setup.title,setStringValue:&*ns("Setup is unavailable")];
                let _: () = msg_send![&*setup.done,setEnabled:false];
            }
        }
    }

    pub(super) unsafe fn poll_setup(&self) -> bool {
        let mut state = self.ivars().borrow_mut();
        let Some(setup) = state.setup.as_mut() else {
            return false;
        };
        let mut changed = false;
        if setup.checked.elapsed() > Duration::from_millis(500) {
            setup.checked = Instant::now();
            let access = AXIsProcessTrusted();
            if access && !ACCESS.load(Ordering::Relaxed) {
                ACCESS.store(install_tap(), Ordering::Relaxed);
                changed = true;
            } else if !access && ACCESS.swap(false, Ordering::Relaxed) {
                setup.tested = false;
                changed = true;
            }
        }
        if let Ok(mut keys) = KEYS.get_or_init(Default::default).lock() {
            let held = keys.tracker.held(now());
            let pressed = keys
                .actions
                .iter()
                .chain(std::iter::once(&held))
                .any(|a| matches!(a, PressAction::Tap | PressAction::Prompt));
            if pressed && ACCESS.load(Ordering::Relaxed) && !setup.tested {
                setup.tested = true;
                changed = true;
            }
            keys.actions.clear();
        }
        let quitting = state
            .messages
            .try_iter()
            .any(|m| matches!(m, Message::Quit));
        drop(state);
        if quitting {
            let _: () = msg_send![self,quit:NIL];
        }
        if changed {
            self.render_setup();
        }
        true
    }

    pub(super) unsafe fn finish_setup(&self) {
        let ready = self
            .ivars()
            .borrow()
            .setup
            .as_ref()
            .is_some_and(|s| s.tested)
            && ACCESS.load(Ordering::Relaxed);
        if !ready {
            return;
        }
        if activity::save_private_json(&storage_dir().join("setup-v1.json"), &true).is_err() {
            return;
        }
        if let Some(setup) = self.ivars().borrow_mut().setup.take() {
            let _: () = msg_send![&*setup.window,orderOut:NIL];
        }
        self.ivars().borrow_mut().last_poll = Instant::now();
    }

    pub(super) unsafe fn open_model_config(&self) {
        use std::os::unix::fs::OpenOptionsExt;
        let path = if config_path().exists() {
            config_path()
        } else {
            storage_dir().join(".env")
        };
        let result = (|| -> std::io::Result<()> {
            fs::create_dir_all(path.parent().unwrap())?;
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(mut file) => file.write_all(
                    b"# Optional: add your JEV key, save, then restart Tafo.\nJEV_API_KEY=\n",
                )?,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
            Ok(())
        })();
        if result.is_ok() {
            let url: *mut AnyObject =
                msg_send![class!(NSURL),fileURLWithPath:&*ns(&path.to_string_lossy())];
            let _: bool = msg_send![workspace(),openURL:url];
        }
    }
}
