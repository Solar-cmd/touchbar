//! The bar's controls, built from real AppKit views in the Touch Bar appearances that
//! ship with macOS (`NSAppearanceNameFunctionRow`, `NSAppearanceNameControlStrip`) so
//! they look stock. The running TouchBarServer renders these views onto the panel and
//! routes touches back to them as normal target/action events — we only lay them out.
//! Main thread only.
use crate::system;
use objc2::rc::autoreleasepool;
use objc2::runtime::{AnyClass, AnyObject, Bool};
use objc2::{class, msg_send, sel};
use std::ffi::CString;

pub const GRABBER_W: f64 = 32.0; // chevron key; stock is 16, wider is easier to hit
pub const PLATTER_ITEM_W: f64 = 72.0;
pub const PLATTER_W: f64 = PLATTER_ITEM_W * 3.0; // brightness, volume, mute
pub const CLOSE_W: f64 = 48.0;
pub const ACCESSORY_W: f64 = 30.0;
pub const SLIDER_W: f64 = 360.0; // the compact slider section grown from the right
pub const FKEY_GAP: f64 = 6.0;
pub const STRIP_GAP: f64 = 12.0; // gap between the F-keys and the chevron/slider

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SliderKind {
    Brightness,
    Volume,
}

type Id = *mut AnyObject;

fn nsstring(s: &str) -> Id {
    let c = CString::new(s).unwrap();
    unsafe { msg_send![class!(NSString), stringWithUTF8String: c.as_ptr()] }
}
fn appearance(name: &str) -> Id {
    unsafe { msg_send![class!(NSAppearance), appearanceNamed: nsstring(name)] }
}
fn image(name: &str) -> Id {
    unsafe { msg_send![class!(NSImage), imageNamed: nsstring(name)] }
}
fn set_frame(v: Id, x: f64, y: f64, w: f64, h: f64) {
    unsafe { msg_send![v, setFrame: crate::ffi::CGRect::new(x, y, w, h)] }
}
fn set_hidden(v: Id, hidden: bool) {
    unsafe { msg_send![v, setHidden: Bool::new(hidden)] }
}
fn frameable(v: Id) {
    // Our convenience-created controls default to autolayout; we position by frame.
    unsafe { msg_send![v, setTranslatesAutoresizingMaskIntoConstraints: Bool::YES] }
}
fn add(parent: Id, child: Id) {
    unsafe { msg_send![parent, addSubview: child] }
}

pub struct Ui {
    w: f64,
    h: f64,
    root: Id,
    fkeys: Vec<Id>,
    platter: Id,
    bright: Id,
    volume: Id,
    volume_glyphs: [Id; 4], // off, low, medium, high
    mute: Id,
    mute_glyphs: [Id; 2], // sound on, muted
    close: Id,
    slider: Id,
    slider_min: Id,
    slider_max: Id,
    grabber: Id,
    chevrons: [Id; 2], // collapse (‹), expand (›)
}
unsafe impl Send for Ui {}

impl Ui {
    /// Build the controls, wiring each to `delegate` (which implements the action methods).
    pub fn new(w: f64, h: f64, delegate: Id) -> Ui {
        autoreleasepool(|_| unsafe {
            let root: Id = msg_send![class!(NSView), alloc];
            let root: Id = msg_send![root, initWithFrame: crate::ffi::CGRect::new(0.0, 0.0, w, h)];
            let strip = appearance("NSAppearanceNameControlStrip");
            let _: () = msg_send![root, setAppearance: strip];
            // The item sizes `root` by constraints; its subviews are positioned by frame.
            let _: () = msg_send![root, setTranslatesAutoresizingMaskIntoConstraints: Bool::NO];
            let wc: Id = msg_send![root, widthAnchor];
            let wc: Id = msg_send![wc, constraintEqualToConstant: w];
            let _: () = msg_send![wc, setActive: Bool::YES];
            let hc: Id = msg_send![root, heightAnchor];
            let hc: Id = msg_send![hc, constraintEqualToConstant: h];
            let _: () = msg_send![hc, setActive: Bool::YES];

            let fn_row = appearance("NSAppearanceNameFunctionRow");
            let fkeys: Vec<Id> = (1..=12)
                .map(|n| {
                    let b: Id = msg_send![class!(NSButton),
                        buttonWithTitle: nsstring(&format!("F{n}")), target: delegate, action: sel!(fkey:)];
                    let _: () = msg_send![b, setAppearance: fn_row];
                    let _: () = msg_send![b, setTag: (n - 1) as isize];
                    frameable(b);
                    add(root, b);
                    b
                })
                .collect();

            let platter_cls = AnyClass::get(c"NSTouchBarControlStripPlatter").unwrap_or(class!(NSView));
            let platter: Id = msg_send![platter_cls, alloc];
            let platter: Id = msg_send![platter, initWithFrame: crate::ffi::CGRect::new(w, 0.0, PLATTER_W, h)];
            frameable(platter);
            add(root, platter);
            let btn = |img: Id, action: objc2::runtime::Sel| -> Id {
                let b: Id = msg_send![class!(NSButton), buttonWithImage: img, target: delegate, action: action];
                frameable(b);
                b
            };
            let bright = btn(image("NSTouchBarControlStripDisplayBrightnessUpTemplate"), sel!(openBright:));
            set_frame(bright, 0.0, 0.0, PLATTER_ITEM_W, h);
            add(platter, bright);
            let volume_glyphs = [
                image("NSTouchBarAudioOutputVolumeOffTemplate"),
                image("NSTouchBarAudioOutputVolumeLowTemplate"),
                image("NSTouchBarAudioOutputVolumeMediumTemplate"),
                image("NSTouchBarAudioOutputVolumeHighTemplate"),
            ];
            let volume = btn(volume_glyphs[2], sel!(openVolume:));
            set_frame(volume, PLATTER_ITEM_W, 0.0, PLATTER_ITEM_W, h);
            add(platter, volume);
            let mute_glyphs =
                [image("NSTouchBarAudioOutputMuteOffTemplate"), image("NSTouchBarAudioOutputMuteOnTemplate")];
            let mute = btn(mute_glyphs[0], sel!(mute:));
            set_frame(mute, PLATTER_ITEM_W * 2.0, 0.0, PLATTER_ITEM_W, h);
            add(platter, mute);

            let close = btn(image("NSTouchBarCloseTemplate"), sel!(closeSlider:));
            add(root, close);
            let slider: Id = msg_send![class!(NSSlider),
                sliderWithValue: 0.5f64, minValue: 0.0f64, maxValue: 1.0f64,
                target: delegate, action: sel!(sliderMoved:)];
            let _: () = msg_send![slider, setContinuous: Bool::YES];
            frameable(slider);
            add(root, slider);
            let white: Id = msg_send![class!(NSColor), whiteColor];
            let accessory = || {
                let v: Id = msg_send![class!(NSImageView), alloc];
                let v: Id = msg_send![v, initWithFrame: crate::ffi::CGRect::default()];
                let _: () = msg_send![v, setContentTintColor: white];
                frameable(v);
                add(root, v);
                v
            };
            let (slider_min, slider_max) = (accessory(), accessory());

            // Chevron: a rounded key (function-row bezel) with the stock chevron glyphs.
            let chevrons = [image("NSTouchBarGoBackTemplate"), image("NSTouchBarGoForwardTemplate")];
            let grabber = btn(chevrons[0], sel!(chevron:));
            let _: () = msg_send![grabber, setAppearance: fn_row];
            add(root, grabber);

            // A swipe from the right edge expands/collapses the platter without having to
            // hit the small chevron. Direct touch types let the recognizer see finger drags
            // on the bar; the delegate confines it to the right edge (see shouldReceiveTouch).
            let _: () = msg_send![root, setAllowedTouchTypes: 1usize]; // NSTouchTypeMaskDirect
            let pan: Id = msg_send![class!(NSPanGestureRecognizer), alloc];
            let pan: Id = msg_send![pan, initWithTarget: delegate, action: sel!(swipe:)];
            let _: () = msg_send![pan, setAllowedTouchTypes: 1usize];
            let _: () = msg_send![pan, setDelegate: delegate];
            let _: () = msg_send![root, addGestureRecognizer: pan];

            Ui {
                w, h, root, fkeys, platter, bright, volume, volume_glyphs, mute, mute_glyphs,
                close, slider, slider_min, slider_max, grabber, chevrons,
            }
        })
    }

    pub fn root(&self) -> Id {
        self.root
    }

    pub fn width(&self) -> f64 {
        self.w
    }

    fn close_x(&self) -> f64 {
        self.w - CLOSE_W
    }

    /// Slider track for a section starting at `section_x`: (start, end).
    pub fn slider_track(&self, section_x: f64) -> (f64, f64) {
        let x0 = section_x + 4.0 + ACCESSORY_W + 4.0;
        let x1 = (self.close_x() - 6.0 - ACCESSORY_W - 4.0).max(x0);
        (x0, x1)
    }

    /// Lay out the controls for an animation frame and let AppKit redraw them.
    /// `t` is expand progress (0 collapsed → 1 platter shown), `s` is slider progress
    /// (0 hidden → 1 slider section fully grown). `expanded` only picks the chevron glyph.
    pub fn apply(&self, t: f64, s: f64, expanded: bool, slider: Option<(SliderKind, f32)>) {
        autoreleasepool(|_| unsafe {
            let (w, h) = (self.w, self.h);
            let showing_slider = s > 0.001 && slider.is_some();

            let platter_x = w - t * PLATTER_W;
            let grabber_x = platter_x - GRABBER_W;
            let slider_x = grabber_x + (w - SLIDER_W - grabber_x) * s;
            let end = slider_x - STRIP_GAP;
            let kw = (end - FKEY_GAP * 11.0) / 12.0;
            for (i, &b) in self.fkeys.iter().enumerate() {
                set_frame(b, i as f64 * (kw + FKEY_GAP), 0.0, kw, h);
            }

            // Chevron and platter: present while not (yet) in the slider section.
            set_hidden(self.grabber, showing_slider);
            set_frame(self.grabber, grabber_x, 0.0, GRABBER_W, h);
            let _: () = msg_send![self.grabber, setImage: self.chevrons[expanded as usize]];
            set_hidden(self.platter, showing_slider || t <= 0.001);
            set_frame(self.platter, platter_x, 0.0, PLATTER_W, h);

            let muted = system::muted();
            let _: () = msg_send![self.mute, setImage: self.mute_glyphs[muted as usize]];
            let v = if muted { 0.0 } else { system::volume() };
            let gi = if v <= 0.0 { 0 } else if v < 0.34 { 1 } else if v < 0.67 { 2 } else { 3 };
            let _: () = msg_send![self.volume, setImage: self.volume_glyphs[gi]];

            // Slider section: [min glyph][slider][max glyph][close].
            for v in [self.close, self.slider, self.slider_min, self.slider_max] {
                set_hidden(v, !showing_slider);
            }
            if let (true, Some((kind, value))) = (showing_slider, slider) {
                let (x0, x1) = self.slider_track(slider_x);
                set_frame(self.close, self.close_x(), 0.0, CLOSE_W, h);
                set_frame(self.slider_min, slider_x + 4.0, 0.0, ACCESSORY_W, h);
                set_frame(self.slider, x0, 0.0, x1 - x0, h);
                set_frame(self.slider_max, x1 + 4.0, 0.0, ACCESSORY_W, h);
                let (lo, hi) = match kind {
                    SliderKind::Brightness => (
                        "NSTouchBarControlStripDisplayBrightnessDownTemplate",
                        "NSTouchBarControlStripDisplayBrightnessUpTemplate",
                    ),
                    SliderKind::Volume => {
                        ("NSTouchBarAudioOutputVolumeOffTemplate", "NSTouchBarAudioOutputVolumeHighTemplate")
                    }
                };
                let _: () = msg_send![self.slider_min, setImage: image(lo)];
                let _: () = msg_send![self.slider_max, setImage: image(hi)];
                let _: () = msg_send![self.slider, setDoubleValue: value as f64];
            }
        })
    }
}
