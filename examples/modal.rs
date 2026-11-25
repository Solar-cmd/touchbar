//! Proof of concept: present a *system-modal* NSTouchBar through the running
//! TouchBarServer — the Pock/MTMR path. No SkyLight, no private entitlement, no
//! boot-arg. Shows one button and one slider; taps and slider moves are logged
//! to /tmp/tbpoc.log so we can confirm input is routed back to us.
//!
//!   cargo run --release --example modal
//!
//! Requires the stock TouchBarServer to be running (reboot after re-enabling it).
use objc2::runtime::{AnyObject, Bool, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};
use std::ffi::CString;
use std::fs::OpenOptions;
use std::io::Write;
use std::ptr::null_mut;

type Id = *mut AnyObject;

#[link(name = "AppKit", kind = "framework")]
#[link(name = "DFRFoundation", kind = "framework")]
extern "C" {
    fn DFRSystemModalShowsCloseBoxWhenFrontMost(flag: Bool);
}

const IDENT: &str = "local.touchbar.poc";

fn log(s: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open("/tmp/tbpoc.log") {
        let _ = writeln!(f, "{s}");
    }
}

unsafe fn nsstring(s: &str) -> Id {
    let c = CString::new(s).unwrap();
    msg_send![class!(NSString), stringWithUTF8String: c.as_ptr()]
}

// Delegate callback: build the item the bar asks for.
extern "C" fn make_item(this: Id, _sel: Sel, _bar: Id, ident: Id) -> Id {
    unsafe {
        let item: Id = msg_send![class!(NSCustomTouchBarItem), alloc];
        let item: Id = msg_send![item, initWithIdentifier: ident];

        // A horizontal stack with a button and a slider.
        let btn: Id = msg_send![class!(NSButton),
            buttonWithTitle: nsstring("PoC F1"),
            target: this,
            action: sel!(tap:)];
        let slider: Id = msg_send![class!(NSSlider),
            sliderWithValue: 0.5f64, minValue: 0.0f64, maxValue: 1.0f64,
            target: this, action: sel!(slid:)];
        let _: () = msg_send![slider, setContinuous: Bool::YES];
        let views: [Id; 2] = [btn, slider];
        let items: Id = msg_send![class!(NSArray), arrayWithObjects: views.as_ptr(), count: 2usize];
        let stack: Id = msg_send![class!(NSStackView), stackViewWithViews: items];

        let _: () = msg_send![item, setView: stack];
        let _: Id = msg_send![item, autorelease];
        log("make_item: built item");
        item
    }
}

extern "C" fn tap(_this: Id, _sel: Sel, _sender: Id) {
    log("tap: button pressed");
}

extern "C" fn slid(_this: Id, _sel: Sel, sender: Id) {
    let v: f64 = unsafe { msg_send![sender, doubleValue] };
    log(&format!("slid: {v:.3}"));
}

fn main() {
    let _ = std::fs::remove_file("/tmp/tbpoc.log");
    log("poc starting");
    unsafe {
        // Register a tiny delegate class.
        let mut b = ClassBuilder::new(c"TBPoCDelegate", class!(NSObject)).expect("class");
        b.add_method(
            sel!(touchBar:makeItemForIdentifier:),
            make_item as extern "C" fn(Id, Sel, Id, Id) -> Id,
        );
        b.add_method(sel!(tap:), tap as extern "C" fn(Id, Sel, Id));
        b.add_method(sel!(slid:), slid as extern "C" fn(Id, Sel, Id));
        let cls = b.register();
        let delegate: Id = msg_send![cls, new];

        let app: Id = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, setActivationPolicy: 1isize]; // Accessory

        let tb: Id = msg_send![class!(NSTouchBar), new];
        let _: () = msg_send![tb, setDelegate: delegate];
        let ident = nsstring(IDENT);
        let ids: Id = msg_send![class!(NSArray), arrayWithObject: ident];
        let _: () = msg_send![tb, setDefaultItemIdentifiers: ids];

        DFRSystemModalShowsCloseBoxWhenFrontMost(Bool::NO);

        // Selector name differs across releases; try the modern one, then the old.
        let tb_cls: &AnyObject = &*(class!(NSTouchBar) as *const _ as *const AnyObject);
        let modern = sel!(presentSystemModalTouchBar:systemTrayItemIdentifier:);
        let legacy = sel!(presentSystemModalFunctionBar:systemTrayItemIdentifier:);
        let has_modern: Bool = msg_send![tb_cls, respondsToSelector: modern];
        if has_modern.as_bool() {
            log("present: using presentSystemModalTouchBar:");
            let _: () = msg_send![class!(NSTouchBar),
                presentSystemModalTouchBar: tb, systemTrayItemIdentifier: null_mut::<AnyObject>()];
        } else {
            let has_legacy: Bool = msg_send![tb_cls, respondsToSelector: legacy];
            log(&format!("present: modern absent, legacy present={}", has_legacy.as_bool()));
            let _: () = msg_send![class!(NSTouchBar),
                presentSystemModalFunctionBar: tb, systemTrayItemIdentifier: null_mut::<AnyObject>()];
        }
        log("presented; entering run loop");

        // Keep tb + delegate alive forever.
        std::mem::forget(delegate as usize);
        std::mem::forget(tb as usize);

        let _: () = msg_send![app, run];
    }
}
