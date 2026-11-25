//! A small, light Touch Bar for MacBook Pros, running on top of the stock TouchBarServer.
//!
//! It presents a *system-modal* NSTouchBar through the running TouchBarServer — the same
//! private path Pock/MTMR use — so it needs no SkyLight entitlement, no `amfi` boot-arg
//! and no SIP changes. TouchBarServer does the rendering and delivers touches to our
//! AppKit controls as ordinary target/action events.
//!
//! Shows F1–F12 with a chevron key on the right that expands to brightness, volume and
//! mute; tapping brightness or volume opens a slider. Event-driven: idle until a tap.
mod ffi;
mod system;
mod ui;

use ffi::*;
use objc2::runtime::{AnyObject, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};
use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::Mutex;
use ui::{SliderKind, Ui};

const SLIDER_CLOSE_AFTER: f64 = 6.0; // an untouched slider closes itself
const NEVER: f64 = 1e12;
const ANIM_HZ: f64 = 1.0 / 60.0; // animation tick interval
const ANIM_STEP: f64 = 0.16; // progress per tick (~0.1s open/close)
const IDENT: &str = "local.touchbar.bar";

type Id = *mut AnyObject;

struct App {
    ui: Ui,
    expanded: bool,
    slider: Option<(SliderKind, f32)>,
    t: f64,        // current expand progress
    s: f64,        // current slider progress
    t_target: f64, // where expand is heading
    s_target: f64, // where the slider is heading
    close_timer: CFRunLoopTimerRef,
    anim_timer: CFRunLoopTimerRef,
    _delegate: Id,
    _tb: Id,
}
unsafe impl Send for App {}

static APP: Mutex<Option<App>> = Mutex::new(None);

fn now() -> f64 {
    unsafe { CFAbsoluteTimeGetCurrent() }
}

fn step(cur: f64, target: f64) -> f64 {
    if (cur - target).abs() <= ANIM_STEP {
        target
    } else if cur < target {
        cur + ANIM_STEP
    } else {
        cur - ANIM_STEP
    }
}

impl App {
    fn draw(&self) {
        self.ui.apply(self.t, self.s, self.expanded, self.slider);
    }
    /// Kick the animation timer so it drives t/s toward their targets.
    fn animate(&self) {
        unsafe { CFRunLoopTimerSetNextFireDate(self.anim_timer, now()) };
    }
    /// Pause the animation timer once settled (stay idle until the next tap).
    fn disarm_anim(&self) {
        unsafe { CFRunLoopTimerSetNextFireDate(self.anim_timer, NEVER) };
    }
    /// Advance one frame; returns true while still animating.
    fn tick(&mut self) -> bool {
        self.t = step(self.t, self.t_target);
        self.s = step(self.s, self.s_target);
        // Slider fully closed: drop its data once it's off-screen.
        if self.s == 0.0 && self.s_target == 0.0 {
            self.slider = None;
        }
        self.draw();
        self.t != self.t_target || self.s != self.s_target
    }
    fn arm_close(&self) {
        unsafe { CFRunLoopTimerSetNextFireDate(self.close_timer, now() + SLIDER_CLOSE_AFTER) };
    }
    fn disarm_close(&self) {
        unsafe { CFRunLoopTimerSetNextFireDate(self.close_timer, NEVER) };
    }
    fn set_expanded(&mut self, on: bool) {
        self.expanded = on;
        self.t_target = if on { 1.0 } else { 0.0 };
        self.animate();
    }
    fn open_slider(&mut self, kind: SliderKind) {
        let v = match kind {
            SliderKind::Brightness => system::brightness(),
            SliderKind::Volume => system::volume(),
        };
        self.slider = Some((kind, v));
        self.s_target = 1.0;
        self.animate();
        self.arm_close();
    }
    fn close_slider(&mut self) {
        self.s_target = 0.0;
        self.disarm_close();
        self.animate();
    }
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.lock().unwrap().as_mut().map(f)
}

// ---------------------------------------------------------------- delegate callbacks

extern "C" fn make_item(_this: Id, _sel: Sel, _bar: Id, ident: Id) -> Id {
    with_app(|app| unsafe {
        let item: Id = msg_send![class!(NSCustomTouchBarItem), alloc];
        let item: Id = msg_send![item, initWithIdentifier: ident];
        let _: () = msg_send![item, setView: app.ui.root()];
        let _: Id = msg_send![item, autorelease];
        item
    })
    .unwrap_or(null_mut())
}

extern "C" fn fkey(_this: Id, _sel: Sel, sender: Id) {
    let tag: isize = unsafe { msg_send![sender, tag] };
    if let Some(&code) = system::KC_F.get(tag as usize) {
        system::key(code, true);
        system::key(code, false);
    }
}

extern "C" fn chevron(_this: Id, _sel: Sel, _sender: Id) {
    with_app(|app| {
        let on = !app.expanded;
        app.set_expanded(on);
    });
}

extern "C" fn open_bright(_this: Id, _sel: Sel, _sender: Id) {
    with_app(|app| app.open_slider(SliderKind::Brightness));
}

extern "C" fn open_volume(_this: Id, _sel: Sel, _sender: Id) {
    with_app(|app| app.open_slider(SliderKind::Volume));
}

extern "C" fn mute(_this: Id, _sel: Sel, _sender: Id) {
    with_app(|app| {
        system::set_muted(!system::muted());
        app.draw();
    });
}

extern "C" fn slider_moved(_this: Id, _sel: Sel, sender: Id) {
    let v: f64 = unsafe { msg_send![sender, doubleValue] };
    with_app(|app| {
        if let Some((kind, value)) = app.slider.as_mut() {
            *value = v as f32;
            match kind {
                SliderKind::Brightness => system::set_brightness(*value),
                SliderKind::Volume => system::set_volume(*value),
            }
        }
        app.arm_close();
    });
}

extern "C" fn close_slider(_this: Id, _sel: Sel, _sender: Id) {
    with_app(|app| app.close_slider());
}

extern "C" fn on_close_timer(_t: CFRunLoopTimerRef, _ctx: *mut c_void) {
    with_app(|app| {
        if app.slider.is_some() {
            app.close_slider();
        }
    });
}

extern "C" fn on_anim_timer(_t: CFRunLoopTimerRef, _ctx: *mut c_void) {
    with_app(|app| {
        if !app.tick() {
            app.disarm_anim();
        }
    });
}

// ---------------------------------------------------------------- setup

fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn nsstring(s: &str) -> Id {
    let c = std::ffi::CString::new(s).unwrap();
    unsafe { msg_send![class!(NSString), stringWithUTF8String: c.as_ptr()] }
}

fn build_delegate() -> Id {
    unsafe {
        let mut b = ClassBuilder::new(c"TBDelegate", class!(NSObject)).expect("register TBDelegate");
        b.add_method(
            sel!(touchBar:makeItemForIdentifier:),
            make_item as extern "C" fn(Id, Sel, Id, Id) -> Id,
        );
        b.add_method(sel!(fkey:), fkey as extern "C" fn(Id, Sel, Id));
        b.add_method(sel!(chevron:), chevron as extern "C" fn(Id, Sel, Id));
        b.add_method(sel!(openBright:), open_bright as extern "C" fn(Id, Sel, Id));
        b.add_method(sel!(openVolume:), open_volume as extern "C" fn(Id, Sel, Id));
        b.add_method(sel!(mute:), mute as extern "C" fn(Id, Sel, Id));
        b.add_method(sel!(sliderMoved:), slider_moved as extern "C" fn(Id, Sel, Id));
        b.add_method(sel!(closeSlider:), close_slider as extern "C" fn(Id, Sel, Id));
        let cls = b.register();
        msg_send![cls, new]
    }
}

fn main() {
    let w = env_f64("TB_WIDTH", 1004.0);
    let h = env_f64("TB_HEIGHT", 30.0);

    unsafe {
        let app_obj: Id = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app_obj, setActivationPolicy: 1isize]; // accessory

        let delegate = build_delegate();
        let ui = Ui::new(w, h, delegate);

        let close_timer = CFRunLoopTimerCreate(
            std::ptr::null(),
            NEVER,
            NEVER,
            0,
            0,
            on_close_timer,
            null_mut(),
        );
        CFRunLoopAddTimer(CFRunLoopGetMain(), close_timer, kCFRunLoopCommonModes);

        // Repeating ~60 Hz timer, parked at NEVER; a tap kicks it to drive one transition.
        let anim_timer = CFRunLoopTimerCreate(
            std::ptr::null(),
            NEVER,
            ANIM_HZ,
            0,
            0,
            on_anim_timer,
            null_mut(),
        );
        CFRunLoopAddTimer(CFRunLoopGetMain(), anim_timer, kCFRunLoopCommonModes);

        // Build the touch bar and point it at our one item.
        let tb: Id = msg_send![class!(NSTouchBar), new];
        let _: () = msg_send![tb, setDelegate: delegate];
        let ident = nsstring(IDENT);
        let ids: Id = msg_send![class!(NSArray), arrayWithObject: ident];
        let _: () = msg_send![tb, setDefaultItemIdentifiers: ids];

        let app = App {
            ui,
            expanded: false,
            slider: None,
            t: 0.0,
            s: 0.0,
            t_target: 0.0,
            s_target: 0.0,
            close_timer,
            anim_timer,
            _delegate: delegate,
            _tb: tb,
        };
        app.draw();
        *APP.lock().unwrap() = Some(app);

        // Present it through TouchBarServer. No close box: it's our persistent bar.
        DFRSystemModalShowsCloseBoxWhenFrontMost(false);
        // `placement: 1` takes over the *whole* bar and hides the stock Control Strip
        // (the brightness/volume/Siri region on the right). Fall back to the 2-arg form,
        // then the legacy name, on older systems that lack the placement selector.
        let tb_cls: Id = class!(NSTouchBar) as *const _ as Id;
        let with_placement = sel!(presentSystemModalTouchBar:placement:systemTrayItemIdentifier:);
        let no_placement = sel!(presentSystemModalTouchBar:systemTrayItemIdentifier:);
        let has_placement: bool = msg_send![tb_cls, respondsToSelector: with_placement];
        let has_no_placement: bool = msg_send![tb_cls, respondsToSelector: no_placement];
        if has_placement {
            let _: () = msg_send![class!(NSTouchBar),
                presentSystemModalTouchBar: tb, placement: 1i64,
                systemTrayItemIdentifier: null_mut::<AnyObject>()];
            eprintln!("touchbar: presented full-bar (placement:1) {w}x{h}");
        } else if has_no_placement {
            let _: () = msg_send![class!(NSTouchBar),
                presentSystemModalTouchBar: tb, systemTrayItemIdentifier: null_mut::<AnyObject>()];
            eprintln!("touchbar: presented system-modal bar {w}x{h}");
        } else {
            let _: () = msg_send![class!(NSTouchBar),
                presentSystemModalFunctionBar: tb, systemTrayItemIdentifier: null_mut::<AnyObject>()];
            eprintln!("touchbar: presented legacy function bar {w}x{h}");
        }

        let _: () = msg_send![app_obj, run];
    }
}
