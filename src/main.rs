//! A small Touch Bar server for Apple Silicon MacBook Pros.
//!
//! Takes over the Touch Bar panel from WindowServer through SkyLight's HMD API, the
//! same path the stock TouchBarServer uses, and reads touches from the digitizer
//! through MultitouchSupport. Shows F1–F12 plus the stock Control Strip grabber on
//! the right; the grabber slides out a brightness and a volume button, and tapping
//! either opens the stock slider. The bar is drawn with AppKit's own Touch Bar
//! appearances so it looks like the stock one.
//!
//! Event-driven: it only wakes for touches, keyboard activity and short animations.
mod ffi;
mod gfx;
mod system;
mod ui;

use ffi::*;
use gfx::Canvas;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;
use ui::{Scene, SliderKind, Ui, GRABBER_W, PLATTER_ITEM_W, PLATTER_W, SLIDER_W};

const DIM_AFTER: f64 = 60.0; // seconds idle before dimming (stock behaviour)
const OFF_AFTER: f64 = 75.0; // seconds idle before the panel turns off
const SLIDER_CLOSE_AFTER: f64 = 6.0; // an untouched slider closes itself
const SLIDE_START: f64 = 10.0; // points of horizontal travel that turn a press into a slide
const ANIM_TIME: f64 = 0.25; // expand/collapse and slider open/close duration
const NEVER: f64 = 1e12;

const FKEY_GAP: f64 = 6.0;
const STRIP_GAP: f64 = 12.0; // between F12 and the grabber

// ---------------------------------------------------------------- model

#[derive(Clone, Copy)]
enum Target {
    Swallow,
    F(usize),
    Grabber,
    Open { kind: SliderKind, start_x: f64 },
    Mute,
    Slide { start_x: f64, start_v: f32 },
    Track,
    Close,
}

#[derive(Clone, Copy, PartialEq)]
enum Power {
    On,
    Dim,
    Off,
}

struct App {
    canvas: Canvas,
    ui: Option<Ui>,
    hmd: CFTypeRef,
    panel: *mut c_void,
    rotate: u32,
    touch_scale: f64,
    touch_offset: f64,
    expanded: bool,
    anim_from: f64,
    anim_start: f64,
    touches: HashMap<i32, Target>,
    slider: Option<(SliderKind, f32)>, // kept until the close animation finishes
    slider_open: bool,
    slider_from: f64,
    slider_start: f64,
    power: Power,
    last_activity: f64,
    idle_timer: CFRunLoopTimerRef,
    slider_timer: CFRunLoopTimerRef,
    anim_timer: CFRunLoopTimerRef,
    render_source: CFRunLoopSourceRef,
}
unsafe impl Send for App {}

static APP: Mutex<Option<App>> = Mutex::new(None);

fn now() -> f64 {
    unsafe { CFAbsoluteTimeGetCurrent() }
}

fn set_timer(t: CFRunLoopTimerRef, at: f64) {
    if !t.is_null() {
        unsafe { CFRunLoopTimerSetNextFireDate(t, at) };
    }
}

fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Geometry for an expansion amount `t` and a slider amount `s`, both 0..=1.
/// The slider section grows out of the Control Strip; the F-keys shrink to make room.
struct Layout {
    fkeys: Vec<(f64, f64)>,
    grabber_x: f64,
    platter_x: f64,
    slider_x: f64,
}

fn layout(width: f64, t: f64, s: f64) -> Layout {
    let platter_x = width - t * PLATTER_W;
    let grabber_x = platter_x - GRABBER_W;
    let slider_x = grabber_x + (width - SLIDER_W - grabber_x) * s;
    let end = slider_x - STRIP_GAP;
    let kw = (end - FKEY_GAP * 11.0) / 12.0;
    let fkeys = (0..12).map(|i| (i as f64 * (kw + FKEY_GAP), kw)).collect();
    Layout { fkeys, grabber_x, platter_x, slider_x }
}

/// Animated value moving from `from` toward `to` since `start`.
fn tween(from: f64, to: f64, start: f64) -> f64 {
    from + (to - from) * ease((now() - start) / ANIM_TIME)
}

impl App {
    fn width(&self) -> f64 {
        self.canvas.width_pt()
    }

    /// Current expansion amount, following the animation.
    fn expand_t(&self) -> f64 {
        tween(self.anim_from, if self.expanded { 1.0 } else { 0.0 }, self.anim_start)
    }

    fn slider_s(&self) -> f64 {
        tween(self.slider_from, if self.slider_open { 1.0 } else { 0.0 }, self.slider_start)
    }

    fn layout(&self) -> Layout {
        layout(self.width(), self.expand_t(), self.slider_s())
    }

    fn animating(&self) -> bool {
        let t = now();
        t - self.anim_start < ANIM_TIME || t - self.slider_start < ANIM_TIME
    }

    fn start_animation(&self) {
        set_timer(self.anim_timer, now());
    }

    fn toggle(&mut self) {
        self.anim_from = self.expand_t();
        self.expanded = !self.expanded;
        self.anim_start = now();
        self.start_animation();
    }

    fn close_slider(&mut self) {
        if self.slider_open {
            self.slider_from = self.slider_s();
            self.slider_open = false;
            self.slider_start = now();
            self.start_animation();
        }
    }

    /// Called every animation frame; drops the slider once it has slid shut.
    fn finish_animation(&mut self) {
        if !self.slider_open && now() - self.slider_start >= ANIM_TIME {
            self.slider = None;
        }
    }

    fn scene(&self) -> Scene {
        let l = self.layout();
        let held = |f: &dyn Fn(&Target) -> bool| self.touches.values().any(f);
        Scene {
            fkeys: l
                .fkeys
                .iter()
                .enumerate()
                .map(|(i, &(x, w))| (x, w, held(&|t| matches!(t, Target::F(n) if *n == i))))
                .collect(),
            grabber_x: l.grabber_x,
            grabber_pressed: held(&|t| matches!(t, Target::Grabber)),
            expanded: self.expanded,
            platter_x: l.platter_x,
            bright_pressed: held(&|t| matches!(t, Target::Open { kind: SliderKind::Brightness, .. })),
            volume_pressed: held(&|t| matches!(t, Target::Open { kind: SliderKind::Volume, .. })),
            mute_pressed: held(&|t| matches!(t, Target::Mute)),
            volume: system::volume(),
            muted: system::muted(),
            slider: self.slider.map(|(k, v)| (k, v, held(&|t| matches!(t, Target::Close)))),
            slider_x: l.slider_x,
            slider_alpha: self.slider_s(),
        }
    }

    /// Main thread only (AppKit draws the frame).
    fn render(&mut self) {
        let Some(ui) = self.ui.as_ref() else { return };
        if self.power == Power::Off {
            self.canvas.clear_black();
        } else {
            ui.draw(&self.scene(), &self.canvas);
            if self.power == Power::Dim {
                self.canvas.dim(0.6);
            }
        }
        if self.hmd.is_null() {
            return;
        }
        unsafe {
            let mut err: CFTypeRef = std::ptr::null();
            let d = SLSHMDGetDrawable(self.hmd, 0, &mut err);
            if d.is_null() {
                eprintln!("touchbar: no drawable");
                return;
            }
            self.canvas.blit(SLSHMDDrawableGetIOSurface(d), self.rotate);
            SLSHMDDrawablePresentWithTime(d, 0.0);
        }
    }

    /// Ask the main thread to draw (safe from any thread).
    fn request_render(&self) {
        if !self.render_source.is_null() {
            unsafe {
                CFRunLoopSourceSignal(self.render_source);
                CFRunLoopWakeUp(CFRunLoopGetMain());
            }
        }
    }

    // ------------------------------------------------------------ power

    fn activity(&mut self) {
        self.last_activity = now();
        set_timer(self.idle_timer, self.last_activity + DIM_AFTER);
        if self.power != Power::On {
            if self.power == Power::Off && !self.panel.is_null() {
                unsafe { DFRBrightnessClientDisplayTurnOn(self.panel) };
            }
            self.power = Power::On;
        }
    }

    fn idle_tick(&mut self) {
        let idle = now() - self.last_activity;
        if idle >= OFF_AFTER {
            if self.power != Power::Off {
                self.power = Power::Off;
                self.slider = None;
                self.slider_open = false;
                self.render();
                if !self.panel.is_null() {
                    unsafe { DFRBrightnessClientDisplayTurnOff(self.panel) };
                }
            }
        } else if idle >= DIM_AFTER {
            self.power = Power::Dim;
            self.render();
            set_timer(self.idle_timer, self.last_activity + OFF_AFTER);
        }
    }

    // ------------------------------------------------------------ touches

    /// Slider track at its fully open position: (start, end, knob).
    fn track(&self) -> (f64, f64, f64) {
        match self.ui.as_ref() {
            Some(ui) => ui.slider_track(self.width() - SLIDER_W),
            None => (0.0, 1.0, 0.0),
        }
    }

    fn slider_value_at(&self, x: f64) -> f32 {
        let (x0, x1, knob) = self.track();
        ((x - x0 - knob / 2.0) / (x1 - x0 - knob)) as f32
    }

    fn open_slider(&mut self, kind: SliderKind) -> f32 {
        let v = match kind {
            SliderKind::Brightness => system::brightness(),
            SliderKind::Volume => system::volume(),
        };
        self.slider = Some((kind, v));
        if !self.slider_open {
            self.slider_from = self.slider_s();
            self.slider_open = true;
            self.slider_start = now();
            self.start_animation();
        }
        v
    }

    fn touch_down(&mut self, id: i32, x: f64) {
        let was_off = self.power == Power::Off;
        self.activity();
        set_timer(self.slider_timer, NEVER);
        if was_off {
            self.touches.insert(id, Target::Swallow);
            return;
        }
        let l = self.layout();
        let close_x = self.ui.as_ref().map(|u| u.close_x()).unwrap_or(self.width());
        let target = if self.slider_open && x >= l.slider_x - 2.0 {
            if x >= close_x - 2.0 {
                Target::Close
            } else {
                let v = self.slider_value_at(x);
                self.set_slider_value(v);
                Target::Track
            }
        } else if !self.slider_open && x >= l.grabber_x - 4.0 {
            if x < l.platter_x.min(self.width()) {
                Target::Grabber
            } else {
                match ((x - l.platter_x) / PLATTER_ITEM_W) as usize {
                    0 => Target::Open { kind: SliderKind::Brightness, start_x: x },
                    1 => Target::Open { kind: SliderKind::Volume, start_x: x },
                    _ => Target::Mute,
                }
            }
        } else if let Some(i) =
            l.fkeys.iter().position(|&(kx, kw)| x >= kx - FKEY_GAP / 2.0 && x < kx + kw + FKEY_GAP / 2.0)
        {
            system::key(system::KC_F[i], true);
            Target::F(i)
        } else {
            Target::Swallow
        };
        self.touches.insert(id, target);
    }

    fn touch_move(&mut self, id: i32, x: f64) {
        let Some(t) = self.touches.get(&id).copied() else { return };
        let track = self.track();
        let span = track.1 - track.0 - track.2;
        match t {
            // Press-and-slide on brightness/volume drags the slider straight away (stock gesture).
            Target::Open { kind, start_x } if (x - start_x).abs() > SLIDE_START => {
                let start_v = self.open_slider(kind);
                self.touches.insert(id, Target::Slide { start_x: x, start_v });
            }
            Target::Slide { start_x, start_v } => self.set_slider_value(start_v + ((x - start_x) / span) as f32),
            Target::Track => {
                let v = self.slider_value_at(x);
                self.set_slider_value(v);
            }
            _ => {}
        }
    }

    fn touch_up(&mut self, id: i32) {
        let Some(t) = self.touches.remove(&id) else { return };
        match t {
            Target::F(i) => system::key(system::KC_F[i], false),
            Target::Grabber => self.toggle(),
            Target::Mute => system::set_muted(!system::muted()),
            Target::Open { kind, .. } => {
                self.open_slider(kind);
            }
            Target::Close => self.close_slider(),
            _ => {}
        }
        if self.slider_open && self.touches.is_empty() {
            set_timer(self.slider_timer, now() + SLIDER_CLOSE_AFTER);
        }
    }

    fn set_slider_value(&mut self, v: f32) {
        let Some((kind, value)) = self.slider.as_mut() else { return };
        *value = v.clamp(0.0, 1.0);
        match kind {
            SliderKind::Brightness => system::set_brightness(*value),
            SliderKind::Volume => system::set_volume(*value),
        }
    }
}

// ---------------------------------------------------------------- callbacks

extern "C" fn on_touches(_dev: *mut c_void, data: *const MTTouch, n: i32, _ts: f64, _frame: i32) -> i32 {
    let touches = unsafe { std::slice::from_raw_parts(data, n.max(0) as usize) };
    let mut guard = APP.lock().unwrap();
    let Some(app) = guard.as_mut() else { return 0 };
    let width = app.width();
    let mut seen = Vec::with_capacity(touches.len());
    for t in touches {
        // 3 = make touch, 4 = touching
        if t.state != 3 && t.state != 4 {
            continue;
        }
        let x = (t.normalized.pos.x as f64 * app.touch_scale + app.touch_offset) * width;
        seen.push(t.identifier);
        if app.touches.contains_key(&t.identifier) {
            app.touch_move(t.identifier, x);
        } else {
            app.touch_down(t.identifier, x);
        }
    }
    let gone: Vec<i32> = app.touches.keys().filter(|id| !seen.contains(id)).copied().collect();
    for id in gone {
        app.touch_up(id);
    }
    app.request_render();
    0
}

extern "C" fn on_render(_info: *mut c_void) {
    if let Some(app) = APP.lock().unwrap().as_mut() {
        app.render();
    }
}

extern "C" fn on_anim_timer(_t: CFRunLoopTimerRef, _ctx: *mut c_void) {
    if let Some(app) = APP.lock().unwrap().as_mut() {
        // Drop a fully closed slider before drawing, so the last frame doesn't show it.
        app.finish_animation();
        app.render();
        let next = if app.animating() { now() + 1.0 / 60.0 } else { NEVER };
        set_timer(app.anim_timer, next);
    }
}

static TAP: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

extern "C" fn on_event(_proxy: *mut c_void, ty: u32, event: CGEventRef, _refcon: *mut c_void) -> CGEventRef {
    if ty == 0xFFFF_FFFE || ty == 0xFFFF_FFFF {
        // The system disabled the tap (timeout or user input); turn it back on.
        unsafe { CGEventTapEnable(TAP.load(Ordering::Relaxed), true) };
        return event;
    }
    if let Some(app) = APP.lock().unwrap().as_mut() {
        let was = app.power;
        app.activity();
        if was != Power::On {
            app.render();
        }
    }
    event
}

extern "C" fn on_idle_timer(_t: CFRunLoopTimerRef, _ctx: *mut c_void) {
    if let Some(app) = APP.lock().unwrap().as_mut() {
        app.idle_tick();
    }
}

extern "C" fn on_slider_timer(_t: CFRunLoopTimerRef, _ctx: *mut c_void) {
    if let Some(app) = APP.lock().unwrap().as_mut() {
        if app.slider_open && app.touches.is_empty() {
            app.close_slider();
        }
    }
}

// ---------------------------------------------------------------- setup

fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn acquire_panel() -> Option<(CFTypeRef, usize, usize)> {
    unsafe {
        let hmd = SLSGetTouchBar();
        if hmd.is_null() {
            eprintln!(
                "touchbar: WindowServer did not hand over the Touch Bar. The binary needs the \
                 com.apple.private.SkyLight.touchbar entitlement (and AMFI must allow it)."
            );
            return None;
        }
        CFRetain(hmd);
        let mut err: CFTypeRef = std::ptr::null();
        SLSHMDAcquire(hmd, std::ptr::null(), &mut err);
        if !err.is_null() {
            eprintln!("touchbar: SLSHMDAcquire failed:");
            CFShow(err);
            return None;
        }
        let d = SLSHMDGetDrawable(hmd, 0, &mut err);
        if d.is_null() {
            eprintln!("touchbar: no drawable after acquire");
            return None;
        }
        let s = SLSHMDDrawableGetIOSurface(d);
        let (w, h) = (IOSurfaceGetWidth(s), IOSurfaceGetHeight(s));
        let pf = IOSurfaceGetPixelFormat(s).to_be_bytes();
        eprintln!("touchbar: panel {w}x{h} format '{}'", String::from_utf8_lossy(&pf));
        Some((hmd, w.max(h), w.min(h)))
    }
}

fn start_touch() {
    unsafe {
        let list = MTDeviceCreateList();
        for i in 0..CFArrayGetCount(list) {
            let dev = CFArrayGetValueAtIndex(list, i);
            let (mut w, mut h) = (0, 0);
            MTDeviceGetSensorSurfaceDimensions(dev, &mut w, &mut h);
            // The Touch Bar digitizer is a long thin strip (~232 x 8 mm).
            if h > 0 && w / h >= 10 {
                MTRegisterContactFrameCallback(dev, on_touches);
                MTDeviceStart(dev, 0);
                return;
            }
        }
        eprintln!("touchbar: no Touch Bar digitizer found");
    }
}

fn start_event_tap() {
    unsafe {
        if !CGPreflightListenEventAccess() {
            CGRequestListenEventAccess();
        }
        if !CGPreflightPostEventAccess() {
            CGRequestPostEventAccess();
        }
        // keyDown, flagsChanged, left/right mouse down: listen only (wakes the bar).
        let mask: u64 = (1 << 10) | (1 << 12) | (1 << 1) | (1 << 3);
        let tap = CGEventTapCreate(1, 0, 1, mask, on_event, null_mut());
        if tap.is_null() {
            eprintln!("touchbar: event tap unavailable; grant Input Monitoring (needed for wake-on-typing).");
            return;
        }
        TAP.store(tap, Ordering::Relaxed);
        let src = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        CFRunLoopAddSource(CFRunLoopGetMain(), src, kCFRunLoopCommonModes);
    }
}

fn new_app(w: usize, h: usize) -> App {
    let canvas = Canvas::new(w, h);
    let ui = Ui::new(canvas.width_pt(), canvas.height_pt());
    App {
        canvas,
        ui: Some(ui),
        hmd: std::ptr::null(),
        panel: null_mut(),
        rotate: env_f64("TB_ROTATE", 90.0) as u32,
        touch_scale: env_f64("TB_TOUCH_SCALE", 1.0),
        touch_offset: env_f64("TB_TOUCH_OFFSET", 0.0),
        expanded: false,
        anim_from: 0.0,
        anim_start: 0.0,
        touches: HashMap::new(),
        slider: None,
        slider_open: false,
        slider_from: 0.0,
        slider_start: 0.0,
        power: Power::On,
        last_activity: now(),
        idle_timer: null_mut(),
        slider_timer: null_mut(),
        anim_timer: null_mut(),
        render_source: null_mut(),
    }
}

fn snapshot(dir: &str) {
    let mut app = new_app(2008, 60);
    let shot = |app: &mut App, name: &str| {
        app.render();
        app.canvas.write_png(&format!("{dir}/{name}.png"));
    };
    shot(&mut app, "collapsed");
    app.touches.insert(1, Target::F(4));
    app.touches.insert(2, Target::Grabber);
    shot(&mut app, "pressed");
    app.touches.clear();
    app.expanded = true;
    app.anim_from = 0.0;
    app.anim_start = now() - ANIM_TIME / 2.0;
    shot(&mut app, "animating");
    app.anim_start = 0.0;
    shot(&mut app, "expanded");
    app.touches.insert(1, Target::Mute);
    shot(&mut app, "expanded-pressed");
    app.touches.clear();
    app.slider = Some((SliderKind::Brightness, 0.6));
    app.slider_open = true;
    app.slider_start = now() - ANIM_TIME / 2.0;
    shot(&mut app, "slider-opening");
    app.slider_start = 0.0;
    shot(&mut app, "slider");
    app.expanded = false;
    shot(&mut app, "slider-collapsed-strip");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 && args[1] == "--snapshot" {
        snapshot(&args[2]);
        return;
    }

    let Some((hmd, w, h)) = acquire_panel() else { std::process::exit(78) };
    let panel = unsafe { DFRBrightnessClientCreate() };
    if !panel.is_null() {
        unsafe { DFRBrightnessClientDisplayTurnOn(panel) };
    }
    let make_timer = |first: f64, cb: extern "C" fn(CFRunLoopTimerRef, *mut c_void)| unsafe {
        let t = CFRunLoopTimerCreate(std::ptr::null(), first, NEVER, 0, 0, cb, null_mut());
        CFRunLoopAddTimer(CFRunLoopGetMain(), t, kCFRunLoopCommonModes);
        t
    };
    let render_source = unsafe {
        let mut ctx = CFRunLoopSourceContext {
            version: 0,
            info: null_mut(),
            retain: std::ptr::null(),
            release: std::ptr::null(),
            copy_description: std::ptr::null(),
            equal: std::ptr::null(),
            hash: std::ptr::null(),
            schedule: std::ptr::null(),
            cancel: std::ptr::null(),
            perform: on_render,
        };
        let src = CFRunLoopSourceCreate(std::ptr::null(), 0, &mut ctx);
        CFRunLoopAddSource(CFRunLoopGetMain(), src, kCFRunLoopCommonModes);
        src
    };

    let mut app = new_app(w, h);
    app.hmd = hmd;
    app.panel = panel;
    app.idle_timer = make_timer(now() + DIM_AFTER, on_idle_timer);
    app.slider_timer = make_timer(NEVER, on_slider_timer);
    app.anim_timer = make_timer(NEVER, on_anim_timer);
    app.render_source = render_source;
    app.render();
    *APP.lock().unwrap() = Some(app);

    start_touch();
    start_event_tap();
    unsafe { CFRunLoopRun() };
    unsafe { SLSHMDRelinquish(hmd) };
}
