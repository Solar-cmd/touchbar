//! Side effects: key events, display brightness, output volume.
use crate::ffi::*;
use std::ffi::c_void;
use std::ptr::null;

pub const KC_F: [u16; 12] = [122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111];

// NX_SECONDARYFNMASK: marks the event as if `fn` were held, so F1–F12 arrive as the
// real function keys rather than the media actions they default to on these keyboards.
const FLAG_FN: u64 = 0x80_0000;

pub fn key(code: u16, down: bool) {
    unsafe {
        let e = CGEventCreateKeyboardEvent(null(), code, down);
        if !e.is_null() {
            CGEventSetFlags(e, FLAG_FN);
            CGEventPost(0, e);
            CFRelease(e as _);
        }
    }
}

// ---------- display brightness ----------
fn builtin_display() -> u32 {
    unsafe {
        let mut ids = [0u32; 16];
        let mut n = 0u32;
        if CGGetOnlineDisplayList(16, ids.as_mut_ptr(), &mut n) == 0 {
            for &id in &ids[..n as usize] {
                if CGDisplayIsBuiltin(id) {
                    return id;
                }
            }
        }
        CGMainDisplayID()
    }
}
pub fn brightness() -> f32 {
    let mut b = 0.5f32;
    unsafe { DisplayServicesGetBrightness(builtin_display(), &mut b) };
    b
}
pub fn set_brightness(v: f32) {
    unsafe { DisplayServicesSetBrightness(builtin_display(), v.clamp(0.0, 1.0)) };
}

// ---------- output volume (CoreAudio) ----------
fn output_device() -> u32 {
    let addr = AudioObjectPropertyAddress { selector: fourcc(b"dOut"), scope: fourcc(b"glob"), element: 0 };
    let mut dev = 0u32;
    let mut size = 4u32;
    unsafe { AudioObjectGetPropertyData(1, &addr, 0, null(), &mut size, &mut dev as *mut u32 as *mut c_void) };
    dev
}
const VMVC: AudioObjectPropertyAddress =
    AudioObjectPropertyAddress { selector: fourcc(b"vmvc"), scope: fourcc(b"outp"), element: 0 };
const MUTE: AudioObjectPropertyAddress =
    AudioObjectPropertyAddress { selector: fourcc(b"mute"), scope: fourcc(b"outp"), element: 0 };

pub fn volume() -> f32 {
    let mut v = 0.5f32;
    let mut size = 4u32;
    unsafe {
        AudioHardwareServiceGetPropertyData(output_device(), &VMVC, 0, null(), &mut size, &mut v as *mut f32 as *mut c_void)
    };
    v
}
pub fn muted() -> bool {
    let mut m = 0u32;
    let mut size = 4u32;
    unsafe { AudioObjectGetPropertyData(output_device(), &MUTE, 0, null(), &mut size, &mut m as *mut u32 as *mut c_void) };
    m != 0
}
pub fn set_muted(on: bool) {
    let m = on as u32;
    unsafe { AudioObjectSetPropertyData(output_device(), &MUTE, 0, null(), 4, &m as *const u32 as *const c_void) };
}
pub fn set_volume(v: f32) {
    let dev = output_device();
    let v = v.clamp(0.0, 1.0);
    let mute: u32 = (v == 0.0) as u32;
    unsafe {
        AudioHardwareServiceSetPropertyData(dev, &VMVC, 0, null(), 4, &v as *const f32 as *const c_void);
        AudioObjectSetPropertyData(dev, &MUTE, 0, null(), 4, &mute as *const u32 as *const c_void);
    }
}
