//! Raw bindings. Only what the controls and the modal Touch Bar need — the stock
//! TouchBarServer now does the rendering and touch routing, so there is no SkyLight,
//! MultitouchSupport, IOSurface or panel-power code any more.
#![allow(non_upper_case_globals, non_snake_case, dead_code)]

use objc2::encode::{Encode, Encoding, RefEncode};
use std::ffi::{c_char, c_void};

pub type CFTypeRef = *const c_void;
pub type CFStringRef = *const c_void;
pub type CFRunLoopRef = *mut c_void;
pub type CFRunLoopTimerRef = *mut c_void;
pub type CGEventRef = *mut CGEvent;

/// Opaque CGEvent with the right ObjC encoding (for msg_send! verification in debug).
#[repr(C)]
pub struct CGEvent {
    _p: [u8; 0],
}
unsafe impl RefEncode for CGEvent {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CGEvent", &[]));
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}
impl CGRect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        CGRect { origin: CGPoint { x, y }, size: CGSize { width: w, height: h } }
    }
}
unsafe impl Encode for CGPoint {
    const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
}
unsafe impl Encode for CGSize {
    const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
}
unsafe impl Encode for CGRect {
    const ENCODING: Encoding = Encoding::Struct("CGRect", &[CGPoint::ENCODING, CGSize::ENCODING]);
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub static kCFRunLoopCommonModes: CFStringRef;
    pub fn CFRelease(cf: CFTypeRef);
    pub fn CFRunLoopGetMain() -> CFRunLoopRef;
    pub fn CFRunLoopAddTimer(rl: CFRunLoopRef, t: CFRunLoopTimerRef, mode: CFStringRef);
    pub fn CFRunLoopTimerCreate(
        alloc: *const c_void,
        fire: f64,
        interval: f64,
        flags: u64,
        order: isize,
        cb: extern "C" fn(CFRunLoopTimerRef, *mut c_void),
        ctx: *mut c_void,
    ) -> CFRunLoopTimerRef;
    pub fn CFRunLoopTimerSetNextFireDate(t: CFRunLoopTimerRef, date: f64);
    pub fn CFAbsoluteTimeGetCurrent() -> f64;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    pub fn CGEventCreateKeyboardEvent(src: *const c_void, key: u16, down: bool) -> CGEventRef;
    pub fn CGEventPost(tap: u32, e: CGEventRef);
    pub fn CGGetOnlineDisplayList(max: u32, ids: *mut u32, count: *mut u32) -> i32;
    pub fn CGDisplayIsBuiltin(id: u32) -> bool;
    pub fn CGMainDisplayID() -> u32;
}

// --- Private: present a bar through the running TouchBarServer. Not entitlement-gated.
#[link(name = "DFRFoundation", kind = "framework")]
extern "C" {
    pub fn DFRSystemModalShowsCloseBoxWhenFrontMost(flag: bool);
}

// --- Private: built-in display brightness.
#[link(name = "DisplayServices", kind = "framework")]
extern "C" {
    pub fn DisplayServicesGetBrightness(id: u32, b: *mut f32) -> i32;
    pub fn DisplayServicesSetBrightness(id: u32, b: f32) -> i32;
}

// AppKit is loaded for its classes (resolved at runtime, not link time).
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

// --- CoreAudio output volume / mute.
#[repr(C)]
pub struct AudioObjectPropertyAddress {
    pub selector: u32,
    pub scope: u32,
    pub element: u32,
}
#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    pub fn AudioObjectGetPropertyData(
        obj: u32,
        a: *const AudioObjectPropertyAddress,
        qs: u32,
        qd: *const c_void,
        size: *mut u32,
        data: *mut c_void,
    ) -> i32;
    pub fn AudioObjectSetPropertyData(
        obj: u32,
        a: *const AudioObjectPropertyAddress,
        qs: u32,
        qd: *const c_void,
        size: u32,
        data: *const c_void,
    ) -> i32;
}
#[link(name = "AudioToolbox", kind = "framework")]
extern "C" {
    pub fn AudioHardwareServiceGetPropertyData(
        obj: u32,
        a: *const AudioObjectPropertyAddress,
        qs: u32,
        qd: *const c_void,
        size: *mut u32,
        data: *mut c_void,
    ) -> i32;
    pub fn AudioHardwareServiceSetPropertyData(
        obj: u32,
        a: *const AudioObjectPropertyAddress,
        qs: u32,
        qd: *const c_void,
        size: u32,
        data: *const c_void,
    ) -> i32;
}

// Silence an unused-import style lint when c_char is only used behind cfg.
const _: *const c_char = std::ptr::null();

pub const fn fourcc(s: &[u8; 4]) -> u32 {
    ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | s[3] as u32
}
