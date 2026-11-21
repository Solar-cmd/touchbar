# TouchBar

A small Touch Bar server written in Rust, for Apple Silicon MacBook Pros with the stock `TouchBarServer` disabled. It shows F1–F12. A chevron at the right end expands to reveal brightness, volume and mute buttons. Tapping brightness or volume opens a slider.

## How it works

| Part | API |
|---|---|
| Panel | SkyLight HMD (`SLSGetTouchBar`, `SLSHMDAcquire`, `SLSHMDGetDrawable`, `…PresentWithTime`), the same path TouchBarServer uses |
| Touch | MultitouchSupport contact frames from the Touch Bar digitizer |
| Panel power | DFRBrightness (on/off after idle) |
| Keys | `CGEventPost` (F1–F12) |
| Sliders | DisplayServices (brightness), CoreAudio (volume) |

The app is event-driven: it does no work between touches, keypresses and its timers. The binary is about 340 KB.

Behaviour:
- Tap ‹/› to show or hide the brightness, volume and mute buttons.
- Tap either button to open its slider, or press and slide on it to drag right away. The slider closes with × or after 6 s untouched.
- The bar dims after 60 s idle and turns off after 75 s. A touch or a keypress wakes it.

## Requirements

WindowServer only hands over the Touch Bar to a process with `com.apple.private.SkyLight.touchbar`. macOS kills an ad-hoc-signed binary that claims this entitlement unless AMFI is relaxed. SIP is already off on this Mac, so the boot-arg can be set from macOS:

```bash
sudo nvram boot-args="-v amfi_get_out_of_my_way=1"
```

Then reboot. Warning: this turns off code-signature enforcement for the whole system.

## Install

```bash
./install.sh
```

On first run, grant **Accessibility** and **Input Monitoring** to `TouchBar.app` in System Settings → Privacy & Security. Ad-hoc re-signing changes the code identity, so you have to grant them again after each rebuild. Accessibility lets the app send F-keys; Input Monitoring lets typing wake the bar. Logs go to `/tmp/touchbar.log`.

## Calibration

These are environment variables in `~/Library/LaunchAgents/local.touchbar.plist`:
- `TB_ROTATE`: `90` or `270`. Use this if the panel reports a portrait surface and the image is upside down.
- `TB_TOUCH_SCALE` / `TB_TOUCH_OFFSET`: map digitizer x (0–1) to panel x. Use these if taps land beside the keys.

## Preview without hardware

```bash
cargo run --release -- --snapshot /tmp
```

This writes `collapsed.png`, `expanded.png` and `slider.png`.
