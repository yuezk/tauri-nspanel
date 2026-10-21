# Native window conversion tests

Run from a macOS desktop session:

```sh
cargo test -p tauri-nspanel --features native-tests --test native_conversion
```

The opt-in test uses a real AppKit event loop and hidden WKWebViews serving a
local HTML fixture. It does not use the example frontend or contact a server.
It runs on the main thread with a custom test harness because AppKit cannot run
on a standard Rust test worker.

The test covers an already-loaded decorated window and a `PanelBuilder` panel.
Each performs ten window/panel round trips, including title-bar and style changes.
It checks native window/content identity, the original DOM input and its value,
style/frame restoration, panel registry removal, and rejection of conversion
from worker threads without changing the registry. A watchdog fails stalled runs.

This regression catches WebKit/title-bar KVO exceptions caused by replacing a
window's Objective-C class while those observers remain attached. It does not
replace manual focus, input-method, fullscreen Spaces, or multi-monitor testing.
