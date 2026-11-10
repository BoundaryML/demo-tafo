//! Native macOS UI in Rust. AppKit calls stay on the main thread; BAML runs on a worker.
mod cycler;
mod tracking;
use tracking::tracking_menu;
mod journal;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{class, define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2_foundation::{
    MainThreadMarker, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use smart_switch::*;
use std::{
    cell::RefCell,
    collections::HashSet,
    ffi::c_void,
    fs,
    io::{Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    sync::{
        atomic::{AtomicBool, AtomicPtr, Ordering},
        mpsc, Mutex, OnceLock,
    },
    thread,
    time::{Duration, Instant},
};

#[link(name = "AppKit", kind = "framework")]
extern "C" {}
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn CGEventTapCreate(
        location: u32,
        place: u32,
        options: u32,
        mask: u64,
        callback: extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
        user: *mut c_void,
    ) -> *mut c_void;
    fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
    fn CGEventGetFlags(event: *mut c_void) -> u64;
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    fn CGEventTapEnable(tap: *mut c_void, enabled: bool);
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: *mut c_void,
        order: isize,
    ) -> *mut c_void;
    fn CFRunLoopGetMain() -> *mut c_void;
    fn CFRunLoopAddSource(run_loop: *mut c_void, source: *mut c_void, mode: *const c_void);
    static kCFRunLoopCommonModes: *const c_void;
}
const APP_ID: &str = "local.baml.smart-switch";
const NIL: *mut AnyObject = std::ptr::null_mut();
fn report_local_error(result: Result<()>) {
    if let Err(error) = result {
        eprintln!("Local BAML state update failed: {error}");
    }
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}
fn ns(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}
unsafe fn string(obj: *mut NSString) -> String {
    obj.as_ref().map(|v| v.to_string()).unwrap_or_default()
}
unsafe fn owned(obj: *mut AnyObject) -> Retained<AnyObject> {
    Retained::from_raw(obj).expect("AppKit allocation")
}
unsafe fn new_view(name: &str, frame: NSRect) -> Retained<AnyObject> {
    let class =
        objc2::runtime::AnyClass::get(std::ffi::CString::new(name).unwrap().as_c_str()).unwrap();
    let raw: *mut AnyObject = msg_send![class, alloc];
    owned(msg_send![raw,initWithFrame:frame])
}
unsafe fn color(r: f64, g: f64, b: f64) -> *mut AnyObject {
    msg_send![class!(NSColor),colorWithSRGBRed:r, green:g, blue:b, alpha:1.0f64]
}
unsafe fn font(size: f64, weight: f64) -> *mut AnyObject {
    msg_send![class!(NSFont),systemFontOfSize:size, weight:weight]
}
unsafe fn rounded(view: *mut AnyObject, radius: f64, background: *mut AnyObject) {
    let _: () = msg_send![view,setWantsLayer:true];
    let layer: *mut AnyObject = msg_send![view, layer];
    let _: () = msg_send![layer,setCornerRadius:radius];
    let _: () = msg_send![layer,setMasksToBounds:true];
    if !background.is_null() {
        let cg: *const c_void = msg_send![background, CGColor];
        let _: () = msg_send![layer,setBackgroundColor:cg];
    }
}
unsafe fn label(
    parent: *mut AnyObject,
    text: &str,
    frame: NSRect,
    size: f64,
    weight: f64,
    tint: *mut AnyObject,
) -> Retained<AnyObject> {
    let raw: *mut AnyObject = msg_send![class!(NSTextField),labelWithString:&*ns(text)];
    let label = Retained::retain(raw).unwrap();
    let _: () = msg_send![&*label,setFrame:frame];
    let _: () = msg_send![&*label,setFont:font(size,weight)];
    let _: () = msg_send![&*label,setTextColor:tint];
    let _: () = msg_send![parent,addSubview:&*label];
    label
}
unsafe fn workspace() -> *mut AnyObject {
    msg_send![class!(NSWorkspace), sharedWorkspace]
}
unsafe fn front() -> (String, bool) {
    let app: *mut AnyObject = msg_send![workspace(), frontmostApplication];
    if app.is_null() {
        return (String::new(), false);
    }
    let id = string(msg_send![app, bundleIdentifier]);
    let policy: isize = msg_send![app, activationPolicy];
    (id, policy == 0)
}
unsafe fn running() -> Vec<(String, String, *mut AnyObject)> {
    let list: *mut AnyObject = msg_send![workspace(), runningApplications];
    let count: usize = msg_send![list, count];
    let mut apps = vec![];
    let mut seen = HashSet::new();
    for i in 0..count {
        let app: *mut AnyObject = msg_send![list,objectAtIndex:i];
        let policy: isize = msg_send![app, activationPolicy];
        let terminated: bool = msg_send![app, isTerminated];
        let id = string(msg_send![app, bundleIdentifier]);
        if policy == 0 && !terminated && !id.is_empty() && id != APP_ID && seen.insert(id.clone()) {
            apps.push((id, string(msg_send![app, localizedName]), app));
        }
    }
    apps
}
pub unsafe fn snapshot(history: &History) -> Result<Snapshot> {
    let output = std::process::Command::new("/usr/bin/lsappinfo")
        .args(["-nonames", "visibleProcessList", "-includeHidden", "list"])
        .output()
        .map_err(|e| e.to_string())?;
    let (current, _) = front();
    let apps = running()
        .into_iter()
        .map(|(id, name, _)| sdk::CapturedApp::new(id, name))
        .collect::<Vec<_>>();
    let activity = Activity::load(&storage_dir().join("activity.json"));
    sdk::snapshot_context_sync(sdk::SnapshotSeed::new(
        current,
        APP_ID.to_owned(),
        apps,
        history.activations.clone(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        activity.visits.iter().rev().take(16).cloned().collect(),
        activity.switches.iter().rev().take(16).cloned().collect(),
    ))
    .map_err(|e| e.to_string())
}

pub unsafe fn activate(id: &str) -> Result<()> {
    let Some((_, _, app)) = running().into_iter().find(|(value, _, _)| value == id) else {
        return Err("That app has closed. Choose another.".into());
    };
    let success: bool = msg_send![app,activateWithOptions:3usize];
    if success {
        Ok(())
    } else {
        Err("macOS could not activate that app.".into())
    }
}

#[derive(Default)]
struct KeyState {
    tracker: PressTracker,
    code: Option<i64>,
    cycle_key: Option<i64>,
    actions: Vec<PressAction>,
}
static KEYS: OnceLock<Mutex<KeyState>> = OnceLock::new();
static EPOCH: OnceLock<Instant> = OnceLock::new();
static TAP: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ACCESS: AtomicBool = AtomicBool::new(false);
static CYCLE_VISIBLE: AtomicBool = AtomicBool::new(false);
static FEEDBACK_VISIBLE: AtomicBool = AtomicBool::new(false);
fn now() -> Duration {
    EPOCH.get_or_init(Instant::now).elapsed()
}
extern "C" fn key_event(
    _: *mut c_void,
    kind: u32,
    event: *mut c_void,
    _: *mut c_void,
) -> *mut c_void {
    let Ok(mut state) = KEYS.get_or_init(Default::default).lock() else {
        return event;
    };
    if kind == u32::MAX || kind == u32::MAX - 1 {
        state.tracker.cancel();
        state.code = None;
        unsafe {
            CGEventTapEnable(TAP.load(Ordering::Relaxed), true);
        }
        return event;
    }
    let code = unsafe { CGEventGetIntegerValueField(event, 9) };
    let flags = unsafe { CGEventGetFlags(event) } & ((1 << 17) | (1 << 18) | (1 << 19) | (1 << 20));
    if kind == 11 && state.cycle_key == Some(code) {
        state.cycle_key = None;
        return std::ptr::null_mut();
    }
    if kind == 10
        && flags == 0
        && CYCLE_VISIBLE.load(Ordering::Relaxed)
        && (code == 36 || code == 53)
    {
        if state.cycle_key.is_none() {
            state.actions.push(if code == 36 {
                PressAction::Commit
            } else {
                PressAction::Cancel
            });
        }
        state.cycle_key = Some(code);
        return std::ptr::null_mut();
    }
    let matches = (code == 8 && flags == ((1 << 18) | (1 << 19))) || (code == 80 && flags == 0);
    if kind == 10 && matches {
        if state.tracker.down(now()) {
            state.code = Some(code);
        }
        return std::ptr::null_mut();
    }
    if kind == 11 && state.code == Some(code) && state.tracker.is_down() {
        let action = state.tracker.up(now());
        if action == PressAction::Tap {
            CYCLE_VISIBLE.store(true, Ordering::Relaxed);
        }
        state.actions.push(action);
        state.code = None;
        return std::ptr::null_mut();
    }
    event
}
unsafe fn install_tap() -> bool {
    if !AXIsProcessTrusted() {
        return false;
    }
    let tap = CGEventTapCreate(
        1,
        0,
        0,
        (1 << 10) | (1 << 11),
        key_event,
        std::ptr::null_mut(),
    );
    if tap.is_null() {
        return false;
    }
    TAP.store(tap, Ordering::Relaxed);
    let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
    CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopCommonModes);
    CGEventTapEnable(tap, true);
    true
}

pub fn control(command: &str) -> Result<String> {
    let mut stream =
        UnixStream::connect(storage_dir().join("control.sock")).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream
        .write_all(format!("{command}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let mut reply = String::new();
    stream
        .read_to_string(&mut reply)
        .map_err(|e| e.to_string())?;
    Ok(reply)
}
enum Message {
    Prompt,
    Tap,
    Quit,
    Activity,
}
enum Work {
    Tracking {
        state: TrackingState,
        action: String,
        at: i64,
    },
    Dashboard(Activity),
    Select {
        generation: u64,
        snapshot: Snapshot,
        text: String,
        cached: Option<Selection>,
    },
    Summarize {
        activity: Activity,
        since: i64,
        today: bool,
    },
    CycleStart {
        token: u64,
        context: Snapshot,
        fast: bool,
        ranking: Vec<String>,
        source: String,
        event_id: String,
        at: i64,
    },
    CycleNext {
        token: u64,
        open: Vec<String>,
        pick: Option<usize>,
    },
    CycleEnd {
        at: i64,
        timing: SwitchTiming,
    },
    CycleCancel,
}
enum CycleResponse {
    Plan(u64, Result<Option<CyclePlan>>),
    Ended(Result<Option<SwitchEvent>>),
}
struct Toast {
    window: Retained<AnyObject>,
    progress: Retained<AnyObject>,
    started: Instant,
    origin: NSPoint,
    reduce_motion: bool,
}
struct State {
    window: Option<Retained<AnyObject>>,
    input: Option<Retained<AnyObject>>,
    detail: Option<Retained<AnyObject>>,
    submit: Option<Retained<AnyObject>>,
    rows: Vec<Retained<AnyObject>>,
    history: History,
    activity: Activity,
    activity_window: Option<Retained<AnyObject>>,
    activity_today: bool,
    journal_summary: ActivitySummary,
    journal_pending: bool,
    journal_requested_scope: bool,
    journal_results: mpsc::Receiver<(bool, Result<ActivitySummary>)>,
    cycle_results: mpsc::Receiver<CycleResponse>,
    tracking_results: mpsc::Receiver<Result<TrackingState>>,
    tracking_pending: bool,
    activity_saved: Instant,
    dashboard_saved: Instant,
    prediction_tx: mpsc::SyncSender<Snapshot>,
    predictions: mpsc::Receiver<(Snapshot, Result<Selection>)>,
    prediction_cache: Option<(String, Selection)>,
    prediction_status: String,
    prediction_pending: bool,
    prediction_requested_key: Option<String>,
    prediction_attempt: Instant,
    front_changed: Instant,
    pending: Option<Snapshot>,
    original: String,
    generation: u64,
    busy: bool,
    active_request: Option<(Snapshot, String)>,
    tap_started: Instant,
    toast: Option<Toast>,
    cycle: Option<cycler::ActiveCycle>,
    queued_cycle_taps: usize,
    cycle_commit_pending: bool,
    last_poll: Instant,
    work: mpsc::Sender<Work>,
    results: mpsc::Receiver<(u64, Result<Selection>)>,
    messages: mpsc::Receiver<Message>,
    status: Option<Retained<AnyObject>>,
    timer: Option<Retained<AnyObject>>,
}
define_class!(
    #[unsafe(super=NSObject)]
    #[thread_kind=MainThreadOnly]
    #[ivars=RefCell<State>]
    struct Controller;
    unsafe impl NSObjectProtocol for Controller {}
    impl Controller {
        #[unsafe(method(dismiss:))] fn dismiss(&self,_:*mut AnyObject){unsafe{self.hide(true);}}
        #[unsafe(method(prompt:))] fn prompt(&self,_:*mut AnyObject){unsafe{self.show_prompt();}}
        #[unsafe(method(activity:))] fn activity(&self,_:*mut AnyObject){unsafe{self.show_activity();}}
        #[unsafe(method(today:))] fn today(&self,_:*mut AnyObject){self.ivars().borrow_mut().activity_today=true;unsafe{self.show_activity();}}
        #[unsafe(method(month:))] fn month(&self,_:*mut AnyObject){self.ivars().borrow_mut().activity_today=false;unsafe{self.show_activity();}}
        #[unsafe(method(recording:))] fn recording(&self,_:*mut AnyObject){
            let action = if self.ivars().borrow().activity.enabled { "15m" } else { "resume" };
            unsafe { self.change_tracking(action); }
        }
        #[unsafe(method(tracking:))] fn tracking(&self,sender:*mut AnyObject){unsafe {
            let tag: isize = msg_send![sender, tag];
            let action = match tag { 0 => "15m", 1 => "1h", 2 => "today", 3 => "pause", _ => "resume" };
            self.change_tracking(action);
        }}
        #[unsafe(method(tap:))] fn tap(&self,_:*mut AnyObject){unsafe{self.tap_switch();}}
        #[unsafe(method(cyclePick:))] fn cycle_pick(&self,sender:*mut AnyObject){unsafe{let index:isize=msg_send![sender,tag];self.pick_cycle(index as usize);}}
        #[unsafe(method(submit:))] fn submit(&self,_:*mut AnyObject){unsafe{self.submit_request();}}
        #[unsafe(method(recent:))] fn recent(&self,sender:*mut AnyObject){unsafe{let index:isize=msg_send![sender,tag];let id=self.ivars().borrow().pending.as_ref().and_then(|s|s.apps.iter().filter(|a|a.bundle_id!=s.current_app_id).nth(index as usize)).map(|a|a.bundle_id.clone());if let Some(id)=id{match activate(&id){Ok(())=>{self.hide(false);self.show_toast(&id,"Selected from your recent apps.");},Err(error)=>self.show_error(&error)}}}}
        #[unsafe(method(quit:))] fn quit(&self,_:*mut AnyObject){let _=self.ivars().borrow().activity.save(&storage_dir().join("activity.json"));unsafe{let app:*mut AnyObject=msg_send![class!(NSApplication),sharedApplication];let _:()=msg_send![app,terminate:NIL];}}
        #[unsafe(method(access:))] fn access(&self,_:*mut AnyObject){unsafe{let url:*mut AnyObject=msg_send![class!(NSURL),URLWithString:&*ns("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")];let _:bool=msg_send![workspace(),openURL:url];}}
        #[unsafe(method(tick:))] fn tick(&self,_:*mut AnyObject){unsafe{self.on_tick();}}
        #[unsafe(method(control:textView:doCommandBySelector:))]
        fn command(&self,_:*mut AnyObject,_:*mut AnyObject,selector:Sel)->bool{if selector==sel!(cancelOperation:){unsafe{self.hide(true);}true}else{false}}
        #[unsafe(method(windowShouldClose:))] fn closing(&self,_:*mut AnyObject)->bool{unsafe{self.hide(true);}false}
    }
);
impl Controller {
    fn new(mtm: MainThreadMarker, state: State) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(RefCell::new(state));
        unsafe { msg_send![super(this), init] }
    }
    unsafe fn build(&self) {
        let green = color(0.12, 0.28, 0.23);
        let ink = color(0.13, 0.17, 0.15);
        let gray = color(0.48, 0.50, 0.47);
        let raw: *mut AnyObject = msg_send![class!(NSWindow), alloc];
        let window = owned(
            msg_send![raw,initWithContentRect:rect(0.,0.,480.,120.), styleMask:(1usize|2usize|(1usize<<15)), backing:2usize, defer:false],
        );
        let _: () = msg_send![&*window,setReleasedWhenClosed:false];
        let _: () = msg_send![&*window,setTitle:&*ns("Smart Switch")];
        let _: () = msg_send![&*window,setTitleVisibility:1isize];
        let _: () = msg_send![&*window,setTitlebarAppearsTransparent:true];
        for kind in 0usize..3 {
            let button: *mut AnyObject = msg_send![&*window,standardWindowButton:kind];
            if !button.is_null() {
                let _: () = msg_send![button,setHidden:true];
            }
        }
        let _: () = msg_send![&*window,setOpaque:false];
        let clear: *mut AnyObject = msg_send![class!(NSColor), clearColor];
        let _: () = msg_send![&*window,setBackgroundColor:clear];
        let _: () = msg_send![&*window,setHasShadow:true];
        let _: () = msg_send![&*window,setLevel:3isize];
        let _: () = msg_send![&*window,setCollectionBehavior:((1usize<<1)|(1usize<<8))];
        let _: () = msg_send![&*window,setDelegate:self];
        let root = new_view("NSVisualEffectView", rect(0., 0., 480., 120.));
        let _: () = msg_send![&*root,setMaterial:21isize];
        let _: () = msg_send![&*root,setBlendingMode:0isize];
        let _: () = msg_send![&*root,setState:1isize];
        rounded(&*root as *const _ as *mut _, 22., color(0.98, 0.98, 0.96));
        let appearance: *mut AnyObject =
            msg_send![class!(NSAppearance),appearanceNamed:&*ns("NSAppearanceNameAqua")];
        let _: () = msg_send![&*root,setAppearance:appearance];
        let _: () = msg_send![&*window,setContentView:&*root];
        let inputbox = new_view("NSView", rect(14., 48., 452., 56.));
        rounded(&*inputbox as *const _ as *mut _, 12., color(1., 1., 0.99));
        let layer: *mut AnyObject = msg_send![&*inputbox, layer];
        let _: () = msg_send![layer,setBorderWidth:1.0f64];
        let border: *const c_void = msg_send![color(0.85, 0.89, 0.83), CGColor];
        let _: () = msg_send![layer,setBorderColor:border];
        let _: () = msg_send![&*root,addSubview:&*inputbox];
        let input = new_view("NSTextField", rect(27., 61., 380., 28.));
        let _: () = msg_send![&*input,setFont:font(17.,0.)];
        let _: () = msg_send![&*input,setTextColor:ink];
        let _: () = msg_send![&*input,setBezeled:false];
        let _: () = msg_send![&*input,setDrawsBackground:false];
        let _: () = msg_send![&*input,setFocusRingType:1usize];
        let _: () = msg_send![&*input,setPlaceholderString:&*ns("Name an app, or Enter for your next app…")];
        let _: () = msg_send![&*input,setAccessibilityLabel:&*ns("App request")];
        let _: () = msg_send![&*input,setTarget:self];
        let _: () = msg_send![&*input,setAction:sel!(submit:)];
        let _: () = msg_send![&*input,setDelegate:self];
        let _: () = msg_send![&*root,addSubview:&*input];
        let submit = new_view("NSButton", rect(418., 60., 34., 32.));
        let _: () = msg_send![&*submit,setTitle:&*ns("↵")];
        let _: () = msg_send![&*submit,setBordered:false];
        let _: () = msg_send![&*submit,setFont:font(18.,0.3)];
        let _: () = msg_send![&*submit,setContentTintColor:color(1.,1.,1.)];
        rounded(&*submit as *const _ as *mut _, 8., green);
        let _: () = msg_send![&*submit,setTarget:self];
        let _: () = msg_send![&*submit,setAction:sel!(submit:)];
        let _: () = msg_send![&*submit,setAccessibilityLabel:&*ns("Switch to app")];
        let _: () = msg_send![&*root,addSubview:&*submit];
        let detail = label(
            &*root as *const _ as *mut _,
            "Type a destination · Enter to switch · Esc to close",
            rect(27., 16., 426., 21.),
            11.,
            0.,
            gray,
        );
        let mut state = self.ivars().borrow_mut();
        state.window = Some(window);
        state.input = Some(input);
        state.detail = Some(detail);
        state.submit = Some(submit);
    }
    unsafe fn detail(&self, text: &str, error: bool) {
        let state = self.ivars().borrow();
        if let Some(label) = &state.detail {
            let _: () = msg_send![&**label,setStringValue:&*ns(text)];
            let _: () = msg_send![&**label,setTextColor:if error{color(0.67,0.30,0.21)}else{color(0.44,0.49,0.44)}];
            let _: () = msg_send![&**label,setToolTip:&*ns(text)];
        }
    }
    unsafe fn show_prompt(&self) {
        self.cancel_cycle();
        let history = self.ivars().borrow().history.clone();
        let context = match snapshot(&history) {
            Ok(s) => s,
            Err(e) => {
                self.detail(&e, true);
                return;
            }
        };
        let mut state = self.ivars().borrow_mut();
        state.original = context.current_app_id.clone();
        state.pending = Some(context.clone());
        state.generation += 1;
        state.busy = false;
        let input = state.input.as_ref().unwrap();
        let _: () = msg_send![&**input,setEnabled:true];
        let _: () = msg_send![&**input,setStringValue:&*ns("")];
        let _: () = msg_send![&**state.submit.as_ref().unwrap(),setEnabled:true];
        for row in state.rows.drain(..) {
            let _: () = msg_send![&*row, removeFromSuperview];
        }
        let window = state.window.as_ref().unwrap();
        let _: () = msg_send![&**window, center];
        let screen: *mut AnyObject = msg_send![class!(NSScreen), mainScreen];
        if !screen.is_null() {
            let frame: NSRect = msg_send![screen, visibleFrame];
            let _: () = msg_send![&**window,setFrameOrigin:NSPoint::new(frame.origin.x+(frame.size.width-480.)/2.,frame.origin.y+frame.size.height*0.55-60.)];
        }
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app,activateIgnoringOtherApps:true];
        let _: () = msg_send![&**window,makeKeyAndOrderFront:NIL];
        let _: bool = msg_send![&**window,makeFirstResponder:&**state.input.as_ref().unwrap()];
        drop(state);
        self.detail(
            "Enter accepts the default · Type or dictate to choose",
            false,
        );
    }
    unsafe fn hide(&self, restore: bool) {
        let mut state = self.ivars().borrow_mut();
        state.generation += 1;
        state.busy = false;
        if let Some(window) = &state.window {
            let _: () = msg_send![&**window,orderOut:NIL];
        }
        if restore {
            let _ = activate(&state.original);
        }
    }
    unsafe fn show_error(&self, error: &str) {
        let visible = {
            let state = self.ivars().borrow();
            let result: bool = msg_send![&**state.window.as_ref().unwrap(), isVisible];
            result
        };
        if !visible {
            self.show_prompt();
        }
        self.detail(error, true);
    }
    unsafe fn submit_request(&self) {
        let text = {
            let state = self.ivars().borrow();
            if state.busy {
                return;
            }
            let value: *mut NSString = msg_send![&**state.input.as_ref().unwrap(), stringValue];
            string(value)
        };
        if text.len() > 2048 {
            self.detail("Please use a shorter app request.", true);
            return;
        }
        let context = self.ivars().borrow().pending.clone();
        if let Some(context) = context {
            self.ivars().borrow_mut().tap_started = Instant::now();
            self.resolve(context, text);
        }
    }
    unsafe fn tap_switch(&self) {
        if self.advance_cycle() {
            return;
        }
        if self.ivars().borrow().busy {
            let mut state = self.ivars().borrow_mut();
            if state
                .active_request
                .as_ref()
                .is_some_and(|(_, text)| text.is_empty())
            {
                state.queued_cycle_taps += 1;
            }
            return;
        }
        self.ivars().borrow_mut().tap_started = Instant::now();
        CYCLE_VISIBLE.store(true, Ordering::Relaxed);
        let history = self.ivars().borrow().history.clone();
        match snapshot(&history) {
            Ok(s) => self.resolve(s, String::new()),
            Err(e) => {
                self.cancel_cycle();
                self.show_error(&e);
            }
        }
    }
    unsafe fn resolve(&self, context: Snapshot, text: String) {
        let mut state = self.ivars().borrow_mut();
        state.generation += 1;
        state.busy = true;
        state.active_request = Some((context.clone(), text.clone()));
        let generation = state.generation;
        let _: () = msg_send![&**state.submit.as_ref().unwrap(),setEnabled:false];
        let cached = if text.is_empty() && state.activity.enabled {
            let key = prediction_key(&context);
            state
                .prediction_cache
                .as_ref()
                .filter(|(k, _)| *k == key)
                .map(|(_, c)| c.clone())
        } else {
            None
        };
        let _ = state.work.send(Work::Select {
            cached,
            generation,
            snapshot: context,
            text,
        });
        drop(state);
        self.detail("Finding your app…", false);
    }
    unsafe fn show_activity(&self) {
        self.request_activity_summary();
        self.render_activity();
    }
    unsafe fn show_toast(&self, id: &str, explanation: &str) {
        let mut state = self.ivars().borrow_mut();
        if let Some(old) = state.toast.take() {
            let _: () = msg_send![&*old.window, orderOut:NIL];
        }
        let raw: *mut AnyObject = msg_send![class!(NSPanel), alloc];
        // Nonactivating panel: the destination app keeps keyboard focus.
        let window = owned(
            msg_send![raw, initWithContentRect:rect(0.,0.,380.,104.), styleMask:128usize, backing:2usize, defer:false],
        );
        let _: () = msg_send![&*window, setReleasedWhenClosed:false];
        let _: () = msg_send![&*window, setHidesOnDeactivate:false];
        let _: () = msg_send![&*window, setIgnoresMouseEvents:true];
        let _: () = msg_send![&*window, setOpaque:false];
        let clear: *mut AnyObject = msg_send![class!(NSColor), clearColor];
        let _: () = msg_send![&*window, setBackgroundColor:clear];
        let _: () = msg_send![&*window, setHasShadow:true];
        let _: () = msg_send![&*window, setLevel:3isize];
        let _: () = msg_send![&*window, setCollectionBehavior:((1usize<<0)|(1usize<<8))];
        let app = running()
            .into_iter()
            .find(|(candidate, _, _)| candidate == id);
        let name = if id == "com.openai.codex" {
            "Codex".to_string()
        } else {
            app.as_ref()
                .map(|(_, name, _)| name.clone())
                .unwrap_or_else(|| id.to_string())
        };
        let _: () = msg_send![&*window,setTitle:&*ns(&format!("Switched to {name}"))];
        let root = new_view("NSView", rect(0., 0., 380., 104.));
        rounded(&*root as *const _ as *mut _, 18., color(0.96, 0.98, 0.94));
        let appearance: *mut AnyObject =
            msg_send![class!(NSAppearance),appearanceNamed:&*ns("NSAppearanceNameAqua")];
        let _: () = msg_send![&*root,setAppearance:appearance];
        let _: () = msg_send![&*window,setContentView:&*root];
        if let Some((_, _, native)) = app {
            let image: *mut AnyObject = msg_send![native, icon];
            let view = new_view("NSImageView", rect(18., 39., 34., 34.));
            let _: () = msg_send![&*view,setImage:image];
            let _: () = msg_send![&*view,setImageScaling:0usize];
            let _: () = msg_send![&*root,addSubview:&*view];
        }
        label(
            &*root as *const _ as *mut _,
            &format!("Switched to {name}"),
            rect(64., 61., 296., 23.),
            14.,
            0.4,
            color(0.12, 0.27, 0.21),
        );
        let reason = label(
            &*root as *const _ as *mut _,
            explanation,
            rect(64., 24., 294., 34.),
            11.5,
            0.,
            color(0.38, 0.45, 0.39),
        );
        let _: () = msg_send![&*reason,setMaximumNumberOfLines:2isize];
        let cell: *mut AnyObject = msg_send![&*reason, cell];
        let _: () = msg_send![cell,setWraps:true];
        let track = new_view("NSView", rect(18., 12., 344., 3.));
        rounded(&*track as *const _ as *mut _, 1.5, color(0.84, 0.89, 0.82));
        let _: () = msg_send![&*root,addSubview:&*track];
        let progress = new_view("NSView", rect(0., 0., 344., 3.));
        rounded(
            &*progress as *const _ as *mut _,
            1.5,
            color(0.38, 0.61, 0.43),
        );
        let _: () = msg_send![&*track,addSubview:&*progress];
        let screen: *mut AnyObject = msg_send![class!(NSScreen), mainScreen];
        let frame: NSRect = msg_send![screen, visibleFrame];
        let origin = NSPoint::new(
            frame.origin.x + (frame.size.width - 380.) / 2.,
            frame.origin.y + 48.,
        );
        let _: () = msg_send![&*window,setFrameOrigin:origin];
        let _: () = msg_send![&*window,setAlphaValue:0.0f64];
        let _: () = msg_send![&*window, orderFrontRegardless];
        let reduce_motion: bool = msg_send![workspace(), accessibilityDisplayShouldReduceMotion];
        state.toast = Some(Toast {
            window,
            progress,
            started: Instant::now(),
            origin,
            reduce_motion,
        });
        FEEDBACK_VISIBLE.store(true, Ordering::Relaxed);
    }
    unsafe fn animate_toast(&self) {
        let mut state = self.ivars().borrow_mut();
        if let Some(toast) = state.toast.as_ref() {
            let motion = ToastMotion::at(toast.started.elapsed(), toast.reduce_motion);
            if motion.finished {
                let _: () = msg_send![&*toast.window,orderOut:NIL];
                state.toast = None;
                FEEDBACK_VISIBLE.store(false, Ordering::Relaxed);
            } else {
                let _: () = msg_send![&*toast.window,setAlphaValue:motion.opacity];
                let _: () = msg_send![&*toast.window,setFrameOrigin:NSPoint::new(toast.origin.x,toast.origin.y+motion.offset_y)];
                let _: () =
                    msg_send![&*toast.progress,setFrame:rect(0.,0.,344.*motion.remaining,3.)];
            }
        }
    }
    unsafe fn on_tick(&self) {
        self.receive_tracking();
        self.receive_cycle();
        let summaries = self
            .ivars()
            .borrow()
            .journal_results
            .try_iter()
            .collect::<Vec<_>>();
        if let Some((today, result)) = summaries.into_iter().last() {
            let mut state = self.ivars().borrow_mut();
            if today != state.activity_today {
                return;
            }
            state.journal_pending = false;
            if let Ok(summary) = result {
                state.journal_summary = summary;
            }
            let visible = state.activity_window.as_ref().is_some_and(|w| {
                let key: bool = msg_send![&**w, isKeyWindow];
                key
            });
            drop(state);
            if visible {
                self.render_activity();
            }
        }
        self.expire_cycle();
        self.animate_toast();
        let mut actions = vec![];
        if let Ok(mut keys) = KEYS.get_or_init(Default::default).lock() {
            let held = keys.tracker.held(now());
            actions.push(held);
            actions.append(&mut keys.actions);
        }
        for action in actions {
            match action {
                PressAction::Tap => self.tap_switch(),
                PressAction::Prompt => self.show_prompt(),
                PressAction::Commit => self.finish_cycle(),
                PressAction::Cancel => self.cancel_cycle(),
                PressAction::None => {}
            }
        }
        let messages = self
            .ivars()
            .borrow()
            .messages
            .try_iter()
            .collect::<Vec<_>>();
        for message in messages {
            match message {
                Message::Prompt => self.show_prompt(),
                Message::Tap => self.tap_switch(),
                Message::Activity => self.show_activity(),
                Message::Quit => {
                    self.cancel_cycle();
                    let _ = self
                        .ivars()
                        .borrow()
                        .activity
                        .save(&storage_dir().join("activity.json"));
                    let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
                    let _: () = msg_send![app,terminate:NIL];
                }
            }
        }
        let replies = self.ivars().borrow().results.try_iter().collect::<Vec<_>>();
        for (generation, result) in replies {
            if generation != self.ivars().borrow().generation {
                continue;
            }
            self.ivars().borrow_mut().busy = false;
            {
                let state = self.ivars().borrow();
                let _: () = msg_send![&**state.submit.as_ref().unwrap(),setEnabled:true];
            }
            match result {
                Ok(choice) => {
                    let explanation = choice.reason.clone();
                    if let Some(id) = &choice.bundle_id {
                        let preview = self
                            .ivars()
                            .borrow()
                            .active_request
                            .clone()
                            .filter(|(_, request)| request.is_empty());
                        if let Some((context, _)) = preview {
                            self.hide(false);
                            self.begin_cycle(
                                context,
                                source_name(&choice.source),
                                choice.ranking.clone(),
                                choice.fast,
                            );
                            continue;
                        }
                        match activate(id) {
                            Ok(()) => {
                                {
                                    let mut state = self.ivars().borrow_mut();
                                    if let Some((context, _)) = state.active_request.clone() {
                                        report_local_error(state.activity.record_decision(
                                            unix_now(),
                                            &context,
                                            &choice,
                                            explanation.clone(),
                                        ));
                                        let _ = state
                                            .activity
                                            .save(&storage_dir().join("activity.json"));
                                    }
                                }
                                self.hide(false);
                                self.show_toast(id, &explanation);
                            }
                            Err(e) => {
                                self.show_error(&e);
                            }
                        }
                    } else {
                        self.cancel_cycle();
                        self.show_error(&choice.reason);
                    }
                }
                Err(e) => {
                    self.cancel_cycle();
                    self.show_error(&e);
                }
            }
        }
        let mut journal_changed = false;
        let mut state = self.ivars().borrow_mut();
        for (context, result) in state.predictions.try_iter().collect::<Vec<_>>() {
            journal_changed = true;
            let key = prediction_key(&context);
            state.prediction_pending = false;
            if !state.activity.enabled {
                continue;
            }
            match result {
                Ok(choice) => {
                    if let (Some(guess), Some(visit)) =
                        (&choice.workflow, context.recent_activity.first())
                    {
                        report_local_error(state.activity.record_workflow(visit.started, guess));
                        let _ = state.activity.save(&storage_dir().join("activity.json"));
                    }
                    let context = Some(context);
                    let destination = context
                        .as_ref()
                        .and_then(|s| {
                            s.apps
                                .iter()
                                .find(|a| Some(&a.bundle_id) == choice.bundle_id.as_ref())
                        })
                        .map(|a| display_name(&a.bundle_id, &a.name))
                        .unwrap_or_else(|| "No destination".into());
                    state.prediction_status = format!(
                        "Suggested: {destination} [{}]\n{}",
                        source_name(&choice.source),
                        choice.reason
                    );
                    state.prediction_cache = Some((key, choice));
                }
                Err(_) => {
                    state.prediction_status =
                        "JEV unavailable. Using local workflow patterns, then recent app.".into();
                    state.prediction_cache = None;
                }
            }
        }
        if state.last_poll.elapsed() > Duration::from_millis(200) {
            let elapsed = state.last_poll.elapsed().as_millis() as i64;
            state.last_poll = Instant::now();
            let (id, regular) = front();
            let idle = CGEventSourceSecondsSinceLastEventType(1, u32::MAX);
            let eligible = regular && !id.is_empty() && id != APP_ID;
            let name = if eligible {
                running()
                    .into_iter()
                    .find(|(i, _, _)| *i == id)
                    .map(|(_, n, _)| n)
                    .unwrap_or_else(|| id.clone())
            } else {
                String::new()
            };
            let cycling = state.cycle.as_ref().is_some_and(|cycle| cycle.previewing());
            if state.activity.enabled {
                report_local_error(state.activity.observe(
                    if eligible && !cycling {
                        Some((&id, &name))
                    } else {
                        None
                    },
                    unix_now(),
                    elapsed,
                    idle,
                ));
            }
            if eligible
                && !cycling
                && state.activity.enabled
                && state.history.activations.last() != Some(&id)
            {
                report_local_error(state.history.record(&id));
                state.front_changed = Instant::now();
                let _ = state.history.save(&storage_dir().join("history.json"));
            }
            if state.activity.enabled && state.dashboard_saved.elapsed() > Duration::from_secs(60) {
                state.dashboard_saved = Instant::now();
                let _ = state.work.send(Work::Dashboard(state.activity.clone()));
            }
            if state.activity_saved.elapsed() > Duration::from_secs(15) {
                report_local_error(state.activity.prune(unix_now()));
                let _ = state.activity.save(&storage_dir().join("activity.json"));
                state.activity_saved = Instant::now();
            }
            if eligible
                && !cycling
                && state.activity.enabled
                && idle < 120.0
                && !state.prediction_pending
                && state.front_changed.elapsed() > Duration::from_millis(1200)
                && state.prediction_attempt.elapsed() > Duration::from_secs(10)
            {
                state.prediction_attempt = Instant::now();
                if let Ok(mut context) = snapshot(&state.history) {
                    context.recent_switches = state
                        .activity
                        .switches
                        .iter()
                        .rev()
                        .take(16)
                        .cloned()
                        .collect();
                    context.recent_activity = state
                        .activity
                        .visits
                        .iter()
                        .rev()
                        .take(16)
                        .cloned()
                        .collect();
                    let key = prediction_key(&context);
                    if state.prediction_requested_key.as_ref() != Some(&key)
                        && state
                            .prediction_cache
                            .as_ref()
                            .is_none_or(|(cached, _)| *cached != key)
                        && state.prediction_tx.try_send(context).is_ok()
                    {
                        state.prediction_requested_key = Some(key);
                        state.prediction_pending = true;
                        state.prediction_status =
                            "JEV is considering your recent app sequence…".into();
                    }
                }
            }
        }
        let journal_key = state.activity_window.as_ref().is_some_and(|window| {
            let key: bool = msg_send![&**window, isKeyWindow];
            key
        });
        drop(state);
        if journal_changed && journal_key {
            self.show_activity();
        }
    }
}

// AppKit routes Command-V through the Edit menu, including transcript insertion
// from dictation apps. Keep the standard responder-chain actions available.
unsafe fn install_edit_menu(app: *mut AnyObject) {
    let raw: *mut AnyObject = msg_send![class!(NSMenu), alloc];
    let main = owned(msg_send![raw, init]);
    let raw: *mut AnyObject = msg_send![class!(NSMenuItem), alloc];
    let heading = owned(
        msg_send![raw,initWithTitle:&*ns("Edit"), action:Option::<Sel>::None, keyEquivalent:&*ns("")],
    );
    let raw: *mut AnyObject = msg_send![class!(NSMenu), alloc];
    let edit = owned(msg_send![raw,initWithTitle:&*ns("Edit")]);
    for (title, action, key) in [
        ("Undo", sel!(undo:), "z"),
        ("Cut", sel!(cut:), "x"),
        ("Copy", sel!(copy:), "c"),
        ("Paste", sel!(paste:), "v"),
        ("Select All", sel!(selectAll:), "a"),
    ] {
        let raw: *mut AnyObject = msg_send![class!(NSMenuItem), alloc];
        let item =
            owned(msg_send![raw,initWithTitle:&*ns(title), action:action, keyEquivalent:&*ns(key)]);
        let _: () = msg_send![&*edit,addItem:&*item];
    }
    let _: () = msg_send![&*heading,setSubmenu:&*edit];
    let _: () = msg_send![&*main,addItem:&*heading];
    let _: () = msg_send![app,setMainMenu:&*main];
}

pub unsafe fn run(prompt: bool) -> Result<()> {
    initialize_runtime()?;
    let _mtm = MainThreadMarker::new().ok_or("UI must run on the main thread")?;
    let (work_tx, work_rx) = mpsc::channel::<Work>();
    let (result_tx, result_rx) = mpsc::channel();
    let (journal_tx, journal_results) = mpsc::channel();
    let (cycle_tx, cycle_results) = mpsc::channel();
    let (tracking_tx, tracking_results) = mpsc::channel();
    thread::spawn(move || {
        let mut engine = Engine::live();
        for work in work_rx {
            let e = &mut engine;
            match work {
                Work::Tracking { state, action, at } => {
                    let _ = tracking_tx.send(e.tracking(&state, &action, at));
                }
                Work::Dashboard(activity) => {
                    let _ = e.dashboard(&activity);
                }
                Work::Select {
                    generation,
                    snapshot,
                    text,
                    cached,
                } => {
                    let result = if let Some(c) = cached {
                        Ok(c)
                    } else {
                        e.choose(&snapshot, &text)
                    };
                    let _ = result_tx.send((generation, result));
                }
                Work::Summarize {
                    activity,
                    since,
                    today,
                } => {
                    let _ = journal_tx.send((today, e.summarize(&activity, since)));
                }
                Work::CycleStart {
                    token,
                    context,
                    fast,
                    ranking,
                    source,
                    event_id,
                    at,
                } => {
                    let _ = cycle_tx.send(CycleResponse::Plan(
                        token,
                        e.cycle_start(&context, &source, &event_id, at, &ranking, fast),
                    ));
                }
                Work::CycleNext { token, open, pick } => {
                    let _ = cycle_tx.send(CycleResponse::Plan(token, e.cycle_next(&open, pick)));
                }
                Work::CycleCancel => {
                    let _ = cycle_tx.send(CycleResponse::Ended(e.cycle_cancel()));
                }
                Work::CycleEnd { at, timing } => {
                    let _ =
                        cycle_tx.send(CycleResponse::Ended(e.cycle_end_timed(at, Some(timing))));
                }
            }
        }
    });
    let (prediction_tx, prediction_rx) = mpsc::sync_channel::<Snapshot>(1);
    let (predicted_tx, predictions) = mpsc::channel();
    thread::spawn(move || {
        let mut engine = Engine::live();
        for context in prediction_rx {
            if !Activity::load(&storage_dir().join("activity.json")).enabled {
                let _ = predicted_tx.send((context, Err("Tracking is paused".into())));
                continue;
            }
            let result = engine.predict(&context);
            let _ = predicted_tx.send((context, result));
        }
    });
    let (message_tx, message_rx) = mpsc::channel();
    let directory = storage_dir();
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    let socket = directory.join("control.sock");
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket).map_err(|e| e.to_string())?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
            let mut bytes = [0u8; 64];
            let n = stream.read(&mut bytes).unwrap_or(0);
            let command = String::from_utf8_lossy(&bytes[..n]);
            let message = match command.trim() {
                "tap" => Some(Message::Tap),
                "prompt" => Some(Message::Prompt),
                "quit" => Some(Message::Quit),
                "activity" => Some(Message::Activity),
                _ => None,
            };
            if let Some(message) = message {
                let _ = message_tx.send(message);
            }
            let _ = stream.write_all(
                format!(
                    "{{\"running\":true,\"keyboard_access\":{},\"feedback_visible\":{},\"ui\":\"rust-appkit\"}}\n",
                    ACCESS.load(Ordering::Relaxed), FEEDBACK_VISIBLE.load(Ordering::Relaxed)
                )
                .as_bytes(),
            );
        }
    });
    let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
    let _: bool = msg_send![app,setActivationPolicy:1isize];
    let controller = Controller::new(
        _mtm,
        State {
            window: None,
            input: None,
            detail: None,
            submit: None,
            rows: vec![],
            history: History::load(&storage_dir().join("history.json")),
            activity: Activity::load(&storage_dir().join("activity.json")),
            activity_window: None,
            activity_today: true,
            journal_summary: ActivitySummary::new(0, vec![], vec![], vec![], vec![]),
            journal_pending: false,
            journal_requested_scope: true,
            journal_results,
            cycle_results,
            tracking_results,
            tracking_pending: false,
            activity_saved: Instant::now(),
            dashboard_saved: Instant::now() - Duration::from_secs(61),
            prediction_tx,
            predictions,
            prediction_cache: None,
            prediction_status:
                "Learning from app order. Background prediction starts when you use an app.".into(),
            prediction_pending: false,
            prediction_requested_key: None,
            prediction_attempt: Instant::now() - Duration::from_secs(10),
            front_changed: Instant::now(),
            pending: None,
            original: String::new(),
            generation: 0,
            busy: false,
            active_request: None,
            tap_started: Instant::now(),
            toast: None,
            cycle: None,
            queued_cycle_taps: 0,
            cycle_commit_pending: false,
            last_poll: Instant::now(),
            work: work_tx,
            results: result_rx,
            messages: message_rx,
            status: None,
            timer: None,
        },
    );
    install_edit_menu(app);
    controller.build();
    let bar: *mut AnyObject = msg_send![class!(NSStatusBar), systemStatusBar];
    let item: *mut AnyObject = msg_send![bar,statusItemWithLength:-1.0f64];
    let button: *mut AnyObject = msg_send![item, button];
    let initial_title = if controller.ivars().borrow().activity.enabled {
        "⇄"
    } else {
        "⇄ ⏸"
    };
    let _: () = msg_send![button,setTitle:&*ns(initial_title)];
    let _: () = msg_send![button,setToolTip:&*ns("Smart Switch")];
    let menu_raw: *mut AnyObject = msg_send![class!(NSMenu), alloc];
    let menu = owned(msg_send![menu_raw, init]);
    for (title, action) in [
        ("Ask where to go…", sel!(prompt:)),
        ("Switch using workflow", sel!(tap:)),
        ("Activity & switch reasons…", sel!(activity:)),
        ("Keyboard access settings…", sel!(access:)),
        ("Quit Smart Switch", sel!(quit:)),
    ] {
        let raw: *mut AnyObject = msg_send![class!(NSMenuItem), alloc];
        let entry =
            owned(msg_send![raw,initWithTitle:&*ns(title), action:action, keyEquivalent:&*ns("")]);
        let _: () = msg_send![&*entry,setTarget:&*controller];
        let _: () = msg_send![&*menu,addItem:&*entry];
    }
    let tracking = tracking_menu(&controller);
    let raw: *mut AnyObject = msg_send![class!(NSMenuItem), alloc];
    let entry = owned(
        msg_send![raw,initWithTitle:&*ns("Tracking"), action:Option::<Sel>::None, keyEquivalent:&*ns("")],
    );
    let _: () = msg_send![&*entry,setSubmenu:&*tracking];
    let _: () = msg_send![&*menu,insertItem:&*entry,atIndex:3isize];
    let _: () = msg_send![item,setMenu:&*menu];
    controller.ivars().borrow_mut().status = Retained::retain(item);
    ACCESS.store(install_tap(), Ordering::Relaxed);
    let timer: *mut AnyObject = msg_send![class!(NSTimer),scheduledTimerWithTimeInterval:0.016f64, target:&*controller, selector:sel!(tick:), userInfo:NIL, repeats:true];
    controller.ivars().borrow_mut().timer = Retained::retain(timer);
    if prompt || !ACCESS.load(Ordering::Relaxed) {
        controller.show_prompt();
    }
    let _: () = msg_send![app, run];
    let _ = fs::remove_file(&socket);
    Ok(())
}
