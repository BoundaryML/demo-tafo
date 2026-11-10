//! Native sprite-sheet decoding and frame playback; no switching or learning policy.
use super::*;
use std::{cell::Cell, collections::HashMap, path::Path};

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGImageCreateWithImageInRect(image: *const c_void, rect: NSRect) -> *const c_void;
    fn CGImageGetWidth(image: *const c_void) -> usize;
    fn CGImageGetHeight(image: *const c_void) -> usize;
    fn CGImageRelease(image: *const c_void);
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Pose {
    Idle,
    Switch,
    Correct,
}
thread_local! {
    static IMAGES: RefCell<HashMap<&'static str, Vec<Retained<AnyObject>>>> = RefCell::new(HashMap::new());
}
unsafe fn frames(pose: Pose) -> Vec<Retained<AnyObject>> {
    let name = match pose {
        Pose::Idle => "ferret-idle-sprites.png",
        Pose::Switch => "ferret-switch-sprites.png",
        Pose::Correct => "ferret-correct-sprites.png",
    };
    IMAGES.with(|cache| {
        if let Some(images)=cache.borrow().get(name) { return images.clone(); }
        let loaded=(|| -> Option<Vec<Retained<AnyObject>>> {
            let bundled=std::env::current_exe().ok()?.parent()?.join("../Resources/mascot").join(name);
            let path=if bundled.exists() {bundled} else {Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/mascot").join(name)};
            let raw: *mut AnyObject=msg_send![class!(NSImage),alloc];
            let sheet: Retained<AnyObject>=Retained::from_raw(msg_send![raw,initWithContentsOfFile:&*ns(&path.to_string_lossy())])?;
            let bitmap: *const c_void=msg_send![&*sheet,CGImageForProposedRect:std::ptr::null_mut::<NSRect>(),context:NIL,hints:NIL];
            if bitmap.is_null() {return None;}
            let width=CGImageGetWidth(bitmap);
            let height=CGImageGetHeight(bitmap);
            if width<4 || height<2 {return None;}
            let mut images=Vec::with_capacity(8);
            for index in 0..8 {
                let col=index%4;
                let row=index/4;
                let x=col*width/4;
                let y=row*height/2;
                let w=(col+1)*width/4-x;
                let h=(row+1)*height/2-y;
                let cell=CGImageCreateWithImageInRect(bitmap,rect(x as f64,y as f64,w as f64,h as f64));
                if cell.is_null() {return None;}
                let raw: *mut AnyObject=msg_send![class!(NSImage),alloc];
                let frame=Retained::from_raw(msg_send![raw,initWithCGImage:cell,size:NSSize::new(256.,256.)]);
                CGImageRelease(cell);
                images.push(frame?);
            }
            Some(images)
        })().unwrap_or_default();
        cache.borrow_mut().insert(name,loaded.clone());
        loaded
    })
}

fn frame_index(pose: Pose, elapsed: Duration, reduced: bool) -> usize {
    if reduced {
        return match pose {
            Pose::Idle => 0,
            Pose::Switch => 4,
            Pose::Correct => 7,
        };
    }
    let ms = elapsed.as_millis();
    match pose {
        Pose::Idle => {
            let mut phase = ms % 4280;
            for (index, duration) in [3200, 80, 100, 120, 160, 160, 160, 300]
                .into_iter()
                .enumerate()
            {
                if phase < duration {
                    return index;
                }
                phase -= duration;
            }
            0
        }
        Pose::Switch => (ms / 85).min(7) as usize,
        Pose::Correct => (ms / 110).min(7) as usize,
    }
}

pub(super) struct Mascot {
    view: Retained<AnyObject>,
    frames: Vec<Retained<AnyObject>>,
    pose: Pose,
    started: Instant,
    frame: Cell<usize>,
}
impl Mascot {
    pub(super) unsafe fn new(parent: *mut AnyObject, frame: NSRect, pose: Pose) -> Self {
        let view = new_view("NSImageView", frame);
        let _: () = msg_send![&*view,setImageScaling:0usize];
        let _: () = msg_send![&*view,setAccessibilityLabel:&*ns("Tafo ferret")];
        let images = frames(pose);
        if let Some(image) = images.first() {
            let _: () = msg_send![&*view,setImage:&**image];
        }
        let _: () = msg_send![parent,addSubview:&*view];
        Self {
            view,
            frames: images,
            pose,
            started: Instant::now(),
            frame: Cell::new(0),
        }
    }
    pub(super) unsafe fn set_pose(&mut self, pose: Pose) {
        if self.pose == pose {
            return;
        }
        self.pose = pose;
        self.started = Instant::now();
        self.frames = frames(pose);
        self.frame.set(usize::MAX);
        self.animate();
    }
    pub(super) unsafe fn animate(&self) {
        let reduced: bool = msg_send![workspace(), accessibilityDisplayShouldReduceMotion];
        let index = frame_index(self.pose, self.started.elapsed(), reduced);
        if index == self.frame.get() {
            return;
        }
        if let Some(image) = self.frames.get(index) {
            let _: () = msg_send![&*self.view,setImage:&**image];
            self.frame.set(index);
        }
    }
}
impl Controller {
    pub(super) unsafe fn animate_mascots(&self) {
        let state = self.ivars().borrow();
        if let Some(setup) = state.setup.as_ref() {
            setup.mascot.animate();
        }
        if let Some(toast) = state.toast.as_ref() {
            toast.mascot.animate();
        }
        if let Some(cycle) = state.cycle.as_ref() {
            cycle.animate_mascot();
        }
    }
}
