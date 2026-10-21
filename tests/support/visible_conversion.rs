//! Included by the main-thread native harness; not a separate Rust test target.
use objc2::{msg_send, sel};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType, NSWindow};
use objc2_foundation::{NSNotFound, NSObjectProtocol, NSPoint, NSRange, NSString};
use tauri::{Manager, WebviewWindow};
use tauri_nspanel::{ManagerExt, WebviewWindowExt};

pub fn start(app: &tauri::AppHandle) {
    WebviewWindow::builder(
        app,
        "visible-window",
        tauri::WebviewUrl::External("conversion-test://localhost".parse().unwrap()),
    )
    .title("Native input regression")
    .inner_size(360.0, 200.0)
    .on_document_title_changed(|window, title| {
        if title.starts_with("failed") {
            panic!("Visible input regression: {title}");
        }
        let phase = if title == "ready" {
            Some(0)
        } else {
            title.strip_prefix("visible-").and_then(|s| s.parse().ok())
        };
        if let Some(phase) = phase {
            dispatch2::DispatchQueue::main().exec_async(move || step(window, phase));
        }
    })
    .build()
    .unwrap();
}

fn check(window: &WebviewWindow, phase: usize, condition: &str) {
    // WebKit delivers text-input IPC asynchronously. Wait for the asserted state
    // rather than assuming an arbitrary sleep is enough on a busy CI runner.
    window.eval(format!(r#"
        (() => {{
            const deadline = Date.now() + 5000;
            function verify() {{
                if ({condition}) document.title = 'visible-{phase}';
                else if (Date.now() < deadline) setTimeout(verify, 20);
                else document.title = 'failed-{phase}:' + originalInput.value + ':' + document.activeElement.id + ':' + JSON.stringify(events);
            }}
            setTimeout(verify, 0);
        }})();
    "#)).unwrap();
}

fn key(native: &NSWindow, text: &str, code: u16) {
    let text = NSString::from_str(text);
    let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown, NSPoint::new(0.0, 0.0), NSEventModifierFlags::empty(), 0.0,
        native.windowNumber(), None, &text, &text, false, code,
    ).unwrap();
    native.sendEvent(&event);
}

fn step(window: WebviewWindow, phase: usize) {
    let native = unsafe { &*window.ns_window().unwrap().cast::<NSWindow>() };
    match phase {
        0 => {
            window.set_focus().unwrap();
            window.eval("window.events=[]; for (const name of ['compositionstart','compositionupdate','compositionend','keydown']) originalInput.addEventListener(name, e => events.push([e.type,e.data || e.key])); originalInput.value=''; originalInput.focus();").unwrap();
            check(&window, 1, "document.activeElement === originalInput");
        }
        1 => {
            assert!(native.isVisible());
            assert!(
                native.isKeyWindow(),
                "visible fixture must receive keyboard input"
            );
            key(native, "a", 0);
            check(&window, 2, "originalInput.value === 'a' && events.some(e => e[0] === 'keydown' && e[1] === 'a')");
        }
        2 | 5 => {
            let responder = native.firstResponder().unwrap();
            assert!(
                responder.respondsToSelector(sel!(setMarkedText:selectedRange:replacementRange:))
            );
            unsafe {
                let _: () = msg_send![&responder, setMarkedText: &*NSString::from_str("に"), selectedRange: NSRange::new(1, 0), replacementRange: NSRange::new(NSNotFound as usize, 0)];
            }
            check(
                &window,
                phase + 1,
                "originalInput.value.endsWith('に') && events[events.length - 1][0] === 'compositionupdate'",
            );
        }
        3 | 6 => {
            let responder = native.firstResponder().unwrap();
            let frame = native.frame();
            let style = native.styleMask();
            if phase == 3 {
                window
                    .to_panel::<super::regression::RegressionPanel>()
                    .unwrap();
            } else {
                window
                    .app_handle()
                    .get_webview_panel(window.label())
                    .unwrap()
                    .to_window()
                    .unwrap();
            }
            assert!(native.isVisible(), "conversion hid visible window");
            assert!(native.isKeyWindow(), "conversion lost key window");
            assert_eq!(native.frame(), frame);
            assert_eq!(native.styleMask(), style);
            assert_eq!(
                native.firstResponder().as_deref(),
                Some(&*responder),
                "conversion lost first responder"
            );
            // Modern WebKit's synchronous hasMarkedText returns NO even during
            // composition. Assert the real document's composition lifecycle and
            // commit result instead of relying on that unsupported query.
            check(&window, phase + 1, "document.activeElement === originalInput && originalInput.value.endsWith('に') && events[events.length - 1][0] === 'compositionupdate' && originalInput.selectionStart === originalInput.value.length");
        }
        4 | 7 => {
            let responder = native.firstResponder().unwrap();
            unsafe {
                let _: () = msg_send![&responder, insertText: &*NSString::from_str("日"), replacementRange: NSRange::new(NSNotFound as usize, 0)];
            }
            check(
                &window,
                if phase == 4 { 5 } else { 8 },
                if phase == 4 {
                    "originalInput.value === 'a日' && events[events.length - 1][0] === 'compositionend'"
                } else {
                    "originalInput.value === 'a日日' && events.filter(e => e[0] === 'compositionend').length === 2"
                },
            );
        }
        8 => {
            key(native, "b", 11);
            check(
                &window,
                9,
                "originalInput.value === 'a日日b' && events[events.length - 1][0] === 'keydown'",
            );
        }
        9 => {
            eprintln!("PASS: visible window: focus, native keyboard events and marked-text composition survive both conversions");
            window.app_handle().exit(0);
        }
        _ => unreachable!(),
    }
}
