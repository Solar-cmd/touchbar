# TouchBar

A small Touch Bar app written in Rust for Apple Silicon MacBook Pros. It shows F1–F12. A chevron at the right end expands to reveal brightness, volume and mute buttons. Tapping brightness or volume opens a slider.

It runs **on top of the stock, running `TouchBarServer`** — the same path Pock and MTMR use — so it needs **no boot-arg, no SIP change and no private entitlement**.

## How it works

The app builds ordinary AppKit controls and presents them as a *system-modal* `NSTouchBar`:

```
+[NSTouchBar presentSystemModalTouchBar:placement:systemTrayItemIdentifier:]
```

`placement: 1` makes our bar take over the **whole** panel, which hides Apple's Control Strip while we run. `TouchBarServer` renders the views onto the panel and delivers touches back to them as normal target/action events, so there is no custom drawing or digitizer code.

| Part | API |
|---|---|
| Panel + touch | Stock `TouchBarServer`, via system-modal `NSTouchBar` (`DFRFoundation`) |
| Keys | `CGEventPost` (F1–F12) |
| Brightness | DisplayServices |
| Volume / mute | CoreAudio |

The app is event-driven: it does nothing between taps except a brief 60 Hz timer while the expand/slider animation is in flight. The binary is about 340 KB.

Behaviour:
- Tap ‹/› to slide the brightness, volume and mute buttons in or out.
- Tap either button to open its slider, or press and slide to drag right away. The slider eases closed with × or after 6 s untouched.

## Requirements

The stock `TouchBarServer` and `ControlStrip` services must be running (they are, by default). This Mac keeps SIP off, but nothing here depends on that.

## Install

```bash
./install.sh
```

On first run, grant **Accessibility** to `TouchBar.app` in System Settings → Privacy & Security — it lets the app post F-key events. Ad-hoc re-signing changes the code identity, so you may have to grant it again after a rebuild.

## Calibration

Environment variables (set in `~/Library/LaunchAgents/local.touchbar.plist`):
- `TB_WIDTH`: bar width in points. Default `1004`, which fills the panel on these models. Lower it if the chevron runs off the right edge.
- `TB_HEIGHT`: bar height in points. Default `30`.

## Proof of concept

`examples/modal.rs` is a minimal standalone demo of the mechanism — one button and one slider presented full-bar, logging taps to `/tmp/tbpoc.log`:

```bash
cargo run --release --example modal
```
