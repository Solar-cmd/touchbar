//! The bar is drawn by real AppKit controls in the Touch Bar appearances that ship
//! with macOS (`NSAppearanceNameFunctionRow`, `NSAppearanceNameControlStrip`), using the
//! stock Control Strip grabber/platter views and glyphs, so it looks like the stock bar.
//! Only call into this module from the main thread.
use crate::ffi::*;
use crate::gfx::{Canvas, SCALE};
use objc2::rc::autoreleasepool;
use objc2::runtime::{AnyClass, AnyObject, Bool, Sel};
use objc2::{class, msg_send};
use std::ffi::{c_char, CString};
use std::ptr::null;

pub const GRABBER_W: f64 = 32.0; // stock is 16; wider is easier to hit
pub const PLATTER_ITEM_W: f64 = 72.0;
pub const PLATTER_W: f64 = PLATTER_ITEM_W * 3.0; // brightness, volume, mute
pub const CLOSE_W: f64 = 48.0;
pub const ACCESSORY_W: f64 = 30.0;
/// Width of the compact slider section that grows out of the Control Strip.
pub const SLIDER_W: f64 = 360.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SliderKind {
    Brightness,
    Volume,
}

/// Everything needed to draw one frame. Positions are in points.
pub struct Scene {
    pub fkeys: Vec<(f64, f64, bool)>, // x, width, pressed
    pub grabber_x: f64,
    pub grabber_pressed: bool,
    pub expanded: bool,  // chevron points right (collapse) when true
    pub platter_x: f64,  // platter is PLATTER_W wide; may be partly off the right edge
    pub bright_pressed: bool,
    pub volume_pressed: bool,
    pub mute_pressed: bool,
    pub volume: f32,
    pub muted: bool,
    pub slider: Option<(SliderKind, f32, bool)>, // kind, value, close pressed
    pub slider_x: f64, // left edge of the slider section (it runs to the right edge)
    pub slider_alpha: f64, // 0..1: slider fades in as the strip fades out
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
    unsafe { msg_send![v, setFrame: CGRect::new(x, y, w, h)] }
}
fn set_hidden(v: Id, hidden: bool) {
    unsafe { msg_send![v, setHidden: Bool::new(hidden)] }
}
fn set_alpha(v: Id, a: f64) {
    unsafe { msg_send![v, setAlphaValue: a] }
}
fn highlight(b: Id, on: bool) {
    unsafe { msg_send![b, highlight: Bool::new(on)] }
}
fn image_button(img: Id) -> Id {
    unsafe { msg_send![class!(NSButton), buttonWithImage: img, target: null::<AnyObject>(), action: None::<Sel>] }
}
fn add(parent: Id, child: Id) {
    unsafe { msg_send![parent, addSubview: child] }
}

/// Render a view hierarchy into a CGImage at 2x.
fn snapshot(view: Id, w: f64, h: f64) -> CGImageRef {
    unsafe {
        let rep: Id = msg_send![class!(NSBitmapImageRep), alloc];
        let rep: Id = msg_send![rep,
            initWithBitmapDataPlanes: null::<*mut c_char>() as *mut *mut c_char,
            pixelsWide: (w * SCALE) as isize,
            pixelsHigh: (h * SCALE) as isize,
            bitsPerSample: 8isize,
            samplesPerPixel: 4isize,
            hasAlpha: Bool::YES,
            isPlanar: Bool::NO,
            colorSpaceName: nsstring("NSDeviceRGBColorSpace"),
            bytesPerRow: 0isize,
            bitsPerPixel: 0isize];
        let _: () = msg_send![rep, setSize: CGSize { width: w, height: h }];
        let _: () = msg_send![view, cacheDisplayInRect: CGRect::new(0.0, 0.0, w, h), toBitmapImageRep: rep];
        let img: CGImageRef = msg_send![rep, CGImage];
        CFRetain(img as _);
        let _: () = msg_send![rep, release];
        img
    }
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
    chevrons: [Id; 2], // expand (‹), collapse (›)
}
unsafe impl Send for Ui {}

impl Ui {
    pub fn new(w: f64, h: f64) -> Ui {
        autoreleasepool(|_| unsafe {
            let _: Id = msg_send![class!(NSApplication), sharedApplication];
            let root: Id = msg_send![class!(NSView), alloc];
            let root: Id = msg_send![root, initWithFrame: CGRect::new(0.0, 0.0, w, h)];
            let strip = appearance("NSAppearanceNameControlStrip");
            let _: () = msg_send![root, setAppearance: strip];

            let fn_row = appearance("NSAppearanceNameFunctionRow");
            let fkeys = (1..=12)
                .map(|n| {
                    let b: Id = msg_send![class!(NSButton),
                        buttonWithTitle: nsstring(&format!("F{n}")), target: null::<AnyObject>(), action: None::<Sel>];
                    let _: () = msg_send![b, setAppearance: fn_row];
                    add(root, b);
                    b
                })
                .collect();

            let platter_cls = AnyClass::get(c"NSTouchBarControlStripPlatter").unwrap_or(class!(NSView));
            let platter: Id = msg_send![platter_cls, alloc];
            let platter: Id = msg_send![platter, initWithFrame: CGRect::new(w, 0.0, PLATTER_W, h)];
            add(root, platter);
            let bright = image_button(image("NSTouchBarControlStripDisplayBrightnessUpTemplate"));
            set_frame(bright, 0.0, 0.0, PLATTER_ITEM_W, h);
            add(platter, bright);
            let volume_glyphs = [
                image("NSTouchBarAudioOutputVolumeOffTemplate"),
                image("NSTouchBarAudioOutputVolumeLowTemplate"),
                image("NSTouchBarAudioOutputVolumeMediumTemplate"),
                image("NSTouchBarAudioOutputVolumeHighTemplate"),
            ];
            let volume = image_button(volume_glyphs[2]);
            set_frame(volume, PLATTER_ITEM_W, 0.0, PLATTER_ITEM_W, h);
            add(platter, volume);
            let mute_glyphs =
                [image("NSTouchBarAudioOutputMuteOffTemplate"), image("NSTouchBarAudioOutputMuteOnTemplate")];
            let mute = image_button(mute_glyphs[0]);
            set_frame(mute, PLATTER_ITEM_W * 2.0, 0.0, PLATTER_ITEM_W, h);
            add(platter, mute);

            let close = image_button(image("NSTouchBarCloseTemplate"));
            add(root, close);
            let slider: Id = msg_send![class!(NSSlider),
                sliderWithValue: 0.5f64, minValue: 0.0f64, maxValue: 1.0f64,
                target: null::<AnyObject>(), action: None::<Sel>];
            add(root, slider);
            let white: Id = msg_send![class!(NSColor), whiteColor];
            let accessory = || {
                let v: Id = msg_send![class!(NSImageView), alloc];
                let v: Id = msg_send![v, initWithFrame: CGRect::default()];
                let _: () = msg_send![v, setContentTintColor: white];
                add(root, v);
                v
            };
            let (slider_min, slider_max) = (accessory(), accessory());

            // Grabber: a rounded key with the stock chevrons, wider than stock to be easy to hit.
            let chevrons = [image("NSTouchBarGoBackTemplate"), image("NSTouchBarGoForwardTemplate")];
            let grabber = image_button(chevrons[0]);
            let _: () = msg_send![grabber, setAppearance: fn_row]; // rounded key bezel, like the F-keys
            add(root, grabber);

            Ui { w, h, root, fkeys, platter, bright, volume, volume_glyphs, mute, mute_glyphs, close, slider, slider_min, slider_max, grabber, chevrons }
        })
    }

    /// Close button x, in points. The section is [min glyph][slider][max glyph][close].
    pub fn close_x(&self) -> f64 {
        self.w - CLOSE_W
    }

    /// Slider track for a section starting at `section_x`: (start, end, knob thickness).
    pub fn slider_track(&self, section_x: f64) -> (f64, f64, f64) {
        let x0 = section_x + 4.0 + ACCESSORY_W + 4.0;
        let x1 = (self.close_x() - 6.0 - ACCESSORY_W - 4.0).max(x0);
        let knob: f64 = unsafe { msg_send![self.slider, knobThickness] };
        (x0, x1, knob)
    }

    pub fn draw(&self, scene: &Scene, canvas: &Canvas) {
        autoreleasepool(|_| unsafe {
            let h = self.h;
            let in_slider = scene.slider.is_some();
            for (b, &(x, w, pressed)) in self.fkeys.iter().zip(&scene.fkeys) {
                set_frame(*b, x, 0.0, w, h);
                highlight(*b, pressed);
            }
            // Hand-off: the strip fades out over the first half, the slider in over the second.
            let p = if in_slider { scene.slider_alpha.clamp(0.0, 1.0) } else { 0.0 };
            let a = ((p - 0.5) * 2.0).clamp(0.0, 1.0); // slider opacity
            let strip = ((0.5 - p) * 2.0).clamp(0.0, 1.0); // strip opacity
            for v in [self.platter, self.grabber] {
                set_hidden(v, strip <= 0.0);
                set_alpha(v, strip);
            }
            set_frame(self.grabber, scene.grabber_x, 0.0, GRABBER_W, h);
            highlight(self.grabber, scene.grabber_pressed);
            let _: () = msg_send![self.grabber, setImage: self.chevrons[scene.expanded as usize]];
            set_frame(self.platter, scene.platter_x, 0.0, PLATTER_W, h);
            highlight(self.bright, scene.bright_pressed);
            highlight(self.volume, scene.volume_pressed);
            highlight(self.mute, scene.mute_pressed);
            let _: () = msg_send![self.mute, setImage: self.mute_glyphs[scene.muted as usize]];
            let v = if scene.muted { 0.0 } else { scene.volume };
            let glyph = self.volume_glyphs[if v <= 0.0 { 0 } else if v < 0.34 { 1 } else if v < 0.67 { 2 } else { 3 }];
            let _: () = msg_send![self.volume, setImage: glyph];

            for v in [self.close, self.slider, self.slider_min, self.slider_max] {
                set_hidden(v, !in_slider);
                set_alpha(v, a);
            }
            if let Some((kind, value, close_pressed)) = scene.slider {
                let sx = scene.slider_x;
                let (x0, x1, _) = self.slider_track(sx);
                set_frame(self.close, self.close_x(), 0.0, CLOSE_W, h);
                highlight(self.close, close_pressed);
                set_frame(self.slider_min, sx + 4.0, 0.0, ACCESSORY_W, h);
                set_frame(self.slider, x0, 0.0, x1 - x0, h);
                set_frame(self.slider_max, x1 + 4.0, 0.0, ACCESSORY_W, h);
                let (lo, hi) = match kind {
                    SliderKind::Brightness => (
                        "NSTouchBarControlStripDisplayBrightnessDownTemplate",
                        "NSTouchBarControlStripDisplayBrightnessUpTemplate",
                    ),
                    SliderKind::Volume => ("NSTouchBarAudioOutputVolumeOffTemplate", "NSTouchBarAudioOutputVolumeHighTemplate"),
                };
                let _: () = msg_send![self.slider_min, setImage: image(lo)];
                let _: () = msg_send![self.slider_max, setImage: image(hi)];
                let _: () = msg_send![self.slider, setDoubleValue: value as f64];
            }

            canvas.clear_black();
            let img = snapshot(self.root, self.w, h);
            canvas.image(img, CGRect::new(0.0, 0.0, self.w, h), false, 1.0);
            CFRelease(img as _);
        })
    }
}
