fn main() {
    // SkyLight, MultitouchSupport, DisplayServices, DFRBrightness and CoreBrightness are private.
    println!("cargo:rustc-link-search=framework=/System/Library/PrivateFrameworks");
}
