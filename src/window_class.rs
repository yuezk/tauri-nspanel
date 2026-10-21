use objc2::{rc::Retained, MainThreadMarker};
use objc2_app_kit::{NSView, NSWindow, NSWindowStyleMask};
use objc2_foundation::NSObject;
use tauri::{Runtime, WebviewWindow};

/// Run a native class conversion without leaving WebKit or title-bar KVO observers
/// attached to the old class. Public only because panel macros expand in callers.
#[doc(hidden)]
pub fn convert_window_class<R: Runtime, T>(
    window: &WebviewWindow<R>,
    convert: impl FnOnce(*mut NSObject) -> tauri::Result<T>,
) -> tauri::Result<T> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| std::io::Error::other("Panel conversion must run on the main thread"))?;
    let pointer = window.ns_window()?;
    // SAFETY: Tauri owns this native window, and AppKit access is confined to the
    // main thread. The conversion changes its class, not its address or lifetime.
    let native = unsafe { &*pointer.cast::<NSWindow>() };
    let content = native.contentView();
    let style = native.styleMask();
    let placeholder = NSView::new(mtm);

    // WebKit and the title bar must unregister while their original KVO class is
    // still installed. A non-null placeholder also supports NSPanel conversion.
    native.setContentView(Some(&placeholder));
    native.setStyleMask(NSWindowStyleMask::Borderless);
    let restore = RestoreContent {
        window: native,
        content,
        style,
    };
    let result = convert(pointer.cast());
    drop(restore);
    result
}

struct RestoreContent<'a> {
    window: &'a NSWindow,
    content: Option<Retained<NSView>>,
    style: NSWindowStyleMask,
}

impl Drop for RestoreContent<'_> {
    fn drop(&mut self) {
        // Reattach only after restoring the style on the final class. Keep the
        // original content alive on both successful and rejected conversions.
        self.window.setStyleMask(self.style);
        self.window.setContentView(self.content.as_deref());
    }
}
