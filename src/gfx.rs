//! The frame buffer: a landscape BGRA canvas that the AppKit-rendered bar is composited
//! into, then copied (rotated if needed) into the panel's IOSurface.
use crate::ffi::*;
use std::ffi::c_void;
use std::ptr::{null, null_mut};

pub const SCALE: f64 = 2.0; // pixels per point

pub struct Canvas {
    pub buf: Vec<u8>,
    pub w: usize,
    pub h: usize,
    pub ctx: CGContextRef,
}
unsafe impl Send for Canvas {}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Self {
        let mut buf = vec![0u8; w * h * 4];
        let ctx = unsafe {
            let cs = CGColorSpaceCreateDeviceRGB();
            let c = CGBitmapContextCreate(buf.as_mut_ptr() as *mut c_void, w, h, 8, w * 4, cs, BGRA_INFO);
            CGContextScaleCTM(c, SCALE, SCALE);
            CGContextSetInterpolationQuality(c, 3);
            c
        };
        Canvas { buf, w, h, ctx }
    }
    pub fn width_pt(&self) -> f64 {
        self.w as f64 / SCALE
    }
    pub fn height_pt(&self) -> f64 {
        self.h as f64 / SCALE
    }
    pub fn clear_black(&self) {
        self.fill(0.0, 1.0);
    }
    pub fn dim(&self, amount: f64) {
        self.fill(0.0, amount);
    }
    fn fill(&self, gray: f64, alpha: f64) {
        unsafe {
            CGContextSetRGBFillColor(self.ctx, gray, gray, gray, alpha);
            CGContextFillRect(self.ctx, CGRect::new(0.0, 0.0, self.width_pt(), self.height_pt()));
        }
    }
    /// Draw an image into `r` (points), optionally mirrored horizontally, with opacity.
    pub fn image(&self, img: CGImageRef, r: CGRect, mirror: bool, alpha: f64) {
        if img.is_null() {
            return;
        }
        unsafe {
            CGContextSaveGState(self.ctx);
            CGContextSetAlpha(self.ctx, alpha);
            if mirror {
                CGContextTranslateCTM(self.ctx, r.origin.x * 2.0 + r.size.width, 0.0);
                CGContextScaleCTM(self.ctx, -1.0, 1.0);
            }
            CGContextDrawImage(self.ctx, r, img);
            CGContextRestoreGState(self.ctx);
        }
    }

    /// Copy into an IOSurface, converting pixel format and rotating if the panel is portrait.
    pub fn blit(&self, surf: IOSurfaceRef, rotate: u32) {
        unsafe {
            IOSurfaceLock(surf, 0, null_mut());
            let base = IOSurfaceGetBaseAddress(surf);
            let bpr = IOSurfaceGetBytesPerRow(surf);
            let sw = IOSurfaceGetWidth(surf);
            let sh = IOSurfaceGetHeight(surf);
            let ten_bit = IOSurfaceGetPixelFormat(surf) == fourcc(b"l10r");
            let src = self.buf.as_ptr() as *const u32;
            let portrait = sh > sw;
            for sy in 0..sh {
                let row = base.add(sy * bpr) as *mut u32;
                for sx in 0..sw {
                    let (lx, ly) = if !portrait {
                        (sx, sy)
                    } else if rotate == 270 {
                        (self.w.saturating_sub(1 + sy), sx)
                    } else {
                        (sy, self.h.saturating_sub(1 + sx))
                    };
                    let p = if lx < self.w && ly < self.h { *src.add(ly * self.w + lx) } else { 0xff00_0000 };
                    *row.add(sx) = if ten_bit { to_l10r(p) } else { p };
                }
            }
            IOSurfaceUnlock(surf, 0, null_mut());
        }
    }

    pub fn write_png(&self, path: &str) {
        unsafe {
            let img = CGBitmapContextCreateImage(self.ctx);
            let url = CFURLCreateFromFileSystemRepresentation(null(), path.as_ptr(), path.len() as isize, false);
            let ty = CFStringCreateWithCString(null(), c"public.png".as_ptr(), kCFStringEncodingUTF8);
            let dest = CGImageDestinationCreateWithURL(url, ty, 1, null());
            CGImageDestinationAddImage(dest, img, null());
            CGImageDestinationFinalize(dest);
        }
    }
}

/// BGRA8 (little-endian 0xAARRGGBB) to ARGB2101010.
fn to_l10r(p: u32) -> u32 {
    let ex = |v: u32| (v << 2) | (v >> 6);
    let r = ex((p >> 16) & 0xff);
    let g = ex((p >> 8) & 0xff);
    let b = ex(p & 0xff);
    (3 << 30) | (r << 20) | (g << 10) | b
}
