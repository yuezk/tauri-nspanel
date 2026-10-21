# Native window conversion tests

Run from a macOS desktop session:

```sh
cargo test -p tauri-nspanel --features native-tests --test native_conversion
```

The opt-in test uses a real AppKit event loop and WKWebViews serving a
local HTML fixture. It does not use the example frontend or contact a server.
It runs on the main thread with a custom test harness because AppKit cannot run
on a standard Rust test worker.

The test covers an already-loaded decorated window and a `PanelBuilder` panel.
Each performs ten window/panel round trips, including title-bar and style changes.
It checks native window/content identity, the original DOM input and its value,
style/frame restoration, panel registry removal, and rejection of conversion
from worker threads without changing the registry. A watchdog fails stalled runs.

After the hidden cases, a visible window takes keyboard focus. The test sends
native `NSEvent` key presses through `NSWindow.sendEvent`, starts marked-text
composition through `NSTextInputClient`, converts in both directions, and commits
the composed text. It checks first-responder identity, DOM focus and caret,
composition events and resulting text, visibility, key-window status, and frame
and style preservation. No Accessibility permission or installed IME is needed.
Avoid interacting with the test window while it runs.

The composition assertions use the real document's events and committed value:
modern WebKit's synchronous `hasMarkedText` method returns `NO` even while marked
text is active. Native input-context candidate windows and specific installed
input methods still need manual testing.

## macOS coverage

CI explicitly targets macOS 14, 15, and 26 instead of a moving `macos-latest`.
It can also be started manually with `workflow_dispatch`. macOS 14 runners are
scheduled for retirement in November 2026; remove that job when unavailable and
use a desktop machine for that version. See the
[GitHub runner inventory](https://github.com/actions/runner-images#available-images).

The library does not declare a separate minimum OS version; Tauri documents a
[default deployment minimum of macOS 10.13](https://v2.tauri.app/distribute/macos-application-bundle/).
That deployment target is not proof of runtime compatibility. Older releases,
including gpgui's macOS 13 minimum, need a separate desktop host because GitHub
no longer provides those runners. Local coverage for this change is macOS 26.7.1
on Apple Silicon; CI results must be checked separately.

This regression catches WebKit/title-bar KVO exceptions caused by replacing a
window's Objective-C class while those observers remain attached. It does not
replace manual input-method candidate UI, fullscreen Spaces, or multi-monitor testing.
