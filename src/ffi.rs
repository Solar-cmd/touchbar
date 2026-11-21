//! Raw bindings. Public frameworks plus the private ones that TouchBarServer uses.
#![allow(non_upper_case_globals, non_snake_case, dead_code)]

use objc2::encode::{Encode, Encoding, RefEncode};
use std::ffi::{c_char, c_void};

pub type CFTypeRef = *const c_void;
pub type CFStringRef = *const c_void;
pub type CFArrayRef = *const c_void;
pub type CFRunLoopRef = *mut c_void;
pub type CFRunLoopTimerRef = *mut c_void;
pub type CFRunLoopSourceRef = *mut c_void;
pub type CFMachPortRef = *mut c_void;
pub type CGColorSpaceRef = *mut c_void;
pub type CGImageRef = *mut CGImage;
pub type IOSurfaceRef = *mut c_void;
pub type CGEventRef = *mut CGEvent;
pub type CGContextRef = *mut CGContext;

/// Opaque CoreGraphics types with correct ObjC encodings (needed for msg_send! verification).
#[repr(C)]
pub struct CGContext {
    _p: [u8; 0],
}
unsafe impl RefEncode for CGContext {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("CGContext", &[]));
}
#[repr(C)]
pub struct CGImage {
    _p: [u8; 0],
}
unsafe impl RefEncode for CGImage {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("CGImage", &[]));
}
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

/// Version-0 run loop source context; only `perform` is used.
#[repr(C)]
pub struct CFRunLoopSourceContext {
    pub version: isize,
    pub info: *mut c_void,
    pub retain: *const c_void,
    pub release: *const c_void,
    pub copy_description: *const c_void,
    pub equal: *const c_void,
    pub hash: *const c_void,
    pub schedule: *const c_void,
    pub cancel: *const c_void,
    pub perform: extern "C" fn(*mut c_void),
}

pub const kCGImageAlphaPremultipliedFirst: u32 = 2;
pub const kCGBitmapByteOrder32Little: u32 = 2 << 12;
pub const BGRA_INFO: u32 = kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little;
pub const kCFStringEncodingUTF8: u32 = 0x0800_0100;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub static kCFRunLoopCommonModes: CFStringRef;
    pub fn CFRelease(cf: CFTypeRef);
    pub fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    pub fn CFShow(cf: CFTypeRef);
    pub fn CFRunLoopGetMain() -> CFRunLoopRef;
    pub fn CFRunLoopRun();
    pub fn CFRunLoopAddSource(rl: CFRunLoopRef, src: CFRunLoopSourceRef, mode: CFStringRef);
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
    pub fn CFRunLoopSourceCreate(a: *const c_void, order: isize, ctx: *mut CFRunLoopSourceContext) -> CFRunLoopSourceRef;
    pub fn CFRunLoopSourceSignal(s: CFRunLoopSourceRef);
    pub fn CFRunLoopWakeUp(rl: CFRunLoopRef);
    pub fn CFAbsoluteTimeGetCurrent() -> f64;
    pub fn CFArrayGetCount(a: CFArrayRef) -> isize;
    pub fn CFArrayGetValueAtIndex(a: CFArrayRef, i: isize) -> *const c_void;
    pub fn CFMachPortCreateRunLoopSource(a: *const c_void, p: CFMachPortRef, o: isize) -> CFRunLoopSourceRef;
    pub fn CFStringCreateWithCString(a: *const c_void, s: *const c_char, enc: u32) -> CFStringRef;
    pub fn CFURLCreateFromFileSystemRepresentation(a: *const c_void, b: *const u8, len: isize, dir: bool) -> CFTypeRef;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    pub fn CGColorSpaceCreateDeviceRGB() -> CGColorSpaceRef;
    pub fn CGBitmapContextCreate(
        data: *mut c_void,
        w: usize,
        h: usize,
        bpc: usize,
        bpr: usize,
        cs: CGColorSpaceRef,
        info: u32,
    ) -> CGContextRef;
    pub fn CGBitmapContextCreateImage(c: CGContextRef) -> CGImageRef;
    pub fn CGContextRelease(c: CGContextRef);
    pub fn CGContextSaveGState(c: CGContextRef);
    pub fn CGContextRestoreGState(c: CGContextRef);
    pub fn CGContextScaleCTM(c: CGContextRef, sx: f64, sy: f64);
    pub fn CGContextTranslateCTM(c: CGContextRef, tx: f64, ty: f64);
    pub fn CGContextSetRGBFillColor(c: CGContextRef, r: f64, g: f64, b: f64, a: f64);
    pub fn CGContextFillRect(c: CGContextRef, r: CGRect);
    pub fn CGContextDrawImage(c: CGContextRef, r: CGRect, img: CGImageRef);
    pub fn CGContextSetAlpha(c: CGContextRef, a: f64);
    pub fn CGContextSetInterpolationQuality(c: CGContextRef, q: i32);

    pub fn CGEventCreateKeyboardEvent(src: *const c_void, key: u16, down: bool) -> CGEventRef;
    pub fn CGEventPost(tap: u32, e: CGEventRef);
    pub fn CGEventGetFlags(e: CGEventRef) -> u64;
    pub fn CGEventTapCreate(
        tap: u32,
        place: u32,
        opts: u32,
        mask: u64,
        cb: extern "C" fn(*mut c_void, u32, CGEventRef, *mut c_void) -> CGEventRef,
        refcon: *mut c_void,
    ) -> CFMachPortRef;
    pub fn CGEventTapEnable(tap: CFMachPortRef, on: bool);
    pub fn CGEventSourceFlagsState(state: i32) -> u64;
    pub fn CGPreflightListenEventAccess() -> bool;
    pub fn CGRequestListenEventAccess() -> bool;
    pub fn CGPreflightPostEventAccess() -> bool;
    pub fn CGRequestPostEventAccess() -> bool;
    pub fn CGGetOnlineDisplayList(max: u32, ids: *mut u32, count: *mut u32) -> i32;
    pub fn CGDisplayIsBuiltin(id: u32) -> bool;
    pub fn CGMainDisplayID() -> u32;
}

#[link(name = "ImageIO", kind = "framework")]
extern "C" {
    pub fn CGImageDestinationCreateWithURL(url: CFTypeRef, ty: CFStringRef, n: usize, o: CFTypeRef) -> CFTypeRef;
    pub fn CGImageDestinationAddImage(d: CFTypeRef, i: CGImageRef, o: CFTypeRef);
    pub fn CGImageDestinationFinalize(d: CFTypeRef) -> bool;
}

#[link(name = "IOSurface", kind = "framework")]
extern "C" {
    pub fn IOSurfaceLock(s: IOSurfaceRef, opts: u32, seed: *mut u32) -> i32;
    pub fn IOSurfaceUnlock(s: IOSurfaceRef, opts: u32, seed: *mut u32) -> i32;
    pub fn IOSurfaceGetBaseAddress(s: IOSurfaceRef) -> *mut u8;
    pub fn IOSurfaceGetBytesPerRow(s: IOSurfaceRef) -> usize;
    pub fn IOSurfaceGetWidth(s: IOSurfaceRef) -> usize;
    pub fn IOSurfaceGetHeight(s: IOSurfaceRef) -> usize;
    pub fn IOSurfaceGetPixelFormat(s: IOSurfaceRef) -> u32;
}

// --- Private: SkyLight "HMD" API, the path TouchBarServer uses on Apple Silicon.
// Requires the com.apple.private.SkyLight.touchbar entitlement.
#[link(name = "SkyLight", kind = "framework")]
extern "C" {
    pub fn SLSGetTouchBar() -> CFTypeRef;
    pub fn SLSHMDAcquire(hmd: CFTypeRef, cfg: CFTypeRef, err: *mut CFTypeRef) -> u64;
    pub fn SLSHMDRelinquish(hmd: CFTypeRef);
    pub fn SLSHMDCopyDisplayInfoDictionary(hmd: CFTypeRef) -> CFTypeRef;
    pub fn SLSHMDGetDrawable(hmd: CFTypeRef, flags: u32, err: *mut CFTypeRef) -> CFTypeRef;
    pub fn SLSHMDDrawableGetIOSurface(d: CFTypeRef) -> IOSurfaceRef;
    pub fn SLSHMDDrawablePresentWithTime(d: CFTypeRef, t: f64) -> i32;
}

// --- Private: MultitouchSupport (Touch Bar digitizer).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MTPoint {
    pub x: f32,
    pub y: f32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MTVector {
    pub pos: MTPoint,
    pub vel: MTPoint,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MTTouch {
    pub frame: i32,
    pub timestamp: f64,
    pub identifier: i32,
    pub state: i32,
    pub finger_id: i32,
    pub hand_id: i32,
    pub normalized: MTVector,
    pub size: f32,
    pub zero1: i32,
    pub angle: f32,
    pub major_axis: f32,
    pub minor_axis: f32,
    pub absolute: MTVector,
    pub zero2: [i32; 2],
    pub z_density: f32,
}
pub type MTContactCallback = extern "C" fn(*mut c_void, *const MTTouch, i32, f64, i32) -> i32;

#[link(name = "MultitouchSupport", kind = "framework")]
extern "C" {
    pub fn MTDeviceCreateList() -> CFArrayRef;
    pub fn MTDeviceGetSensorSurfaceDimensions(d: *const c_void, w: *mut i32, h: *mut i32) -> i32;
    pub fn MTRegisterContactFrameCallback(d: *const c_void, cb: MTContactCallback);
    pub fn MTDeviceStart(d: *const c_void, mode: i32) -> i32;
}

// --- Private: display brightness and the Touch Bar panel power.
#[link(name = "DisplayServices", kind = "framework")]
extern "C" {
    pub fn DisplayServicesGetBrightness(id: u32, b: *mut f32) -> i32;
    pub fn DisplayServicesSetBrightness(id: u32, b: f32) -> i32;
}
#[link(name = "DFRBrightness", kind = "framework")]
extern "C" {
    pub fn DFRBrightnessClientCreate() -> *mut c_void;
    pub fn DFRBrightnessClientDisplayTurnOn(c: *mut c_void);
    pub fn DFRBrightnessClientDisplayTurnOff(c: *mut c_void);
}
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

// --- CoreAudio volume.
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

pub const fn fourcc(s: &[u8; 4]) -> u32 {
    ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | s[3] as u32
}
