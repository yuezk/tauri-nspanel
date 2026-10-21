// Requires a macOS desktop session. See docs/testing.md for the command.
#[cfg(target_os = "macos")]
#[path = "support/visible_conversion.rs"]
mod visible_conversion;

#[cfg(target_os = "macos")]
mod regression {
    use std::collections::HashMap;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };
    use tauri::{Manager, WebviewWindow};
    use tauri_nspanel::{tauri_panel, ManagerExt, PanelBuilder, StyleMask, WebviewWindowExt};

    tauri_panel! {
        panel!(RegressionPanel {
            config: { can_become_key_window: true, can_become_main_window: false, }
        })
    }

    struct OriginalWindow {
        address: usize,
        content_address: usize,
        style: tauri_nspanel::objc2_app_kit::NSWindowStyleMask,
        frame: NSRect,
    }

    type Originals = Mutex<HashMap<String, OriginalWindow>>;

    fn step(window: WebviewWindow, count: usize) {
        let handle = window.app_handle();
        let label = window.label();
        let native = unsafe { &*window.ns_window().unwrap().cast::<NSWindow>() };
        if count == 0 {
            // The builder case begins as a loaded borderless panel. Its document
            // must also survive restoring a decorated window before the loop.
            if let Ok(panel) = handle.get_webview_panel(label) {
                panel
                    .set_style_mask(StyleMask::empty().titled().closable().into())
                    .unwrap();
                panel.to_window().expect("Restore builder window");
            }
        }
        let originals = window.state::<Originals>();
        let mut originals = originals.lock().unwrap();
        let original = originals
            .entry(label.to_owned())
            .or_insert_with(|| OriginalWindow {
                address: window.ns_window().unwrap() as usize,
                content_address: Retained::as_ptr(&native.contentView().unwrap()) as usize,
                style: native.styleMask(),
                frame: native.frame(),
            });

        if count == 20 {
            eprintln!("PASS: {label}: 10 round trips preserve native content and DOM/input state");
            if handle.state::<AtomicUsize>().fetch_add(1, Ordering::SeqCst) == 1 {
                super::visible_conversion::start(handle);
            }
            return;
        }

        if count % 2 == 0 {
            let background_window = window.clone();
            assert!(std::thread::spawn(move || background_window
                .to_panel::<RegressionPanel>()
                .is_err())
            .join()
            .unwrap());
            assert!(handle.get_webview_panel(label).is_err());
            let panel = window.to_panel::<RegressionPanel>().unwrap();
            // Conversion itself preserves the incoming style and frame.
            assert_eq!(native.styleMask(), original.style);
            assert_eq!(native.frame(), original.frame);
            panel
                .set_style_mask(StyleMask::empty().borderless().nonactivating_panel().into())
                .unwrap();
            panel.set_corner_radius(14.0);
            panel.set_content_size(280.0, 438.0);
        } else {
            let panel = handle.get_webview_panel(label).unwrap();
            let background_panel = panel.clone();
            assert!(
                std::thread::spawn(move || background_panel.to_window().is_none())
                    .join()
                    .unwrap()
            );
            assert!(handle.get_webview_panel(label).is_ok());
            panel.set_corner_radius(0.0);
            // Exercise class restoration with a live title bar and WebKit KVO.
            panel.set_style_mask(original.style).unwrap();
            panel.to_window().expect("Restore regular window");
            native.setFrame_display(original.frame, false);
            assert_eq!(native.styleMask(), original.style);
            assert_eq!(native.frame(), original.frame);
            assert!(handle.get_webview_panel(label).is_err());
        }
        assert_eq!(window.ns_window().unwrap() as usize, original.address);
        assert_eq!(
            Retained::as_ptr(&native.contentView().unwrap()) as usize,
            original.content_address
        );
        window.eval(format!(
            "document.title = window.originalInput === document.getElementById('portal') && window.originalInput.value === 'vpn.example.com' ? 'checked-{}' : 'failed'",
            count + 1,
        )).unwrap();
    }

    fn title_changed(window: WebviewWindow, title: String) {
        if title == "failed" {
            eprintln!("FAIL: webview or form state was replaced");
            window.app_handle().exit(1);
            return;
        }
        let count = if title == "ready" {
            Some(0)
        } else {
            title
                .strip_prefix("checked-")
                .and_then(|value| value.parse().ok())
        };
        if let Some(count) = count {
            dispatch2::DispatchQueue::main().exec_async(move || step(window, count));
        }
    }

    pub(super) fn run() {
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(40));
            eprintln!("FAIL: native conversion or webview state check timed out");
            std::process::exit(1);
        });
        tauri::Builder::default()
            .plugin(tauri_nspanel::init())
            .manage(Originals::default())
            .manage(AtomicUsize::new(0))
            .register_uri_scheme_protocol("conversion-test", |_, _| {
                tauri::http::Response::builder()
                    .header("Content-Type", "text/html")
                    .body(b"<input id='portal'><script>window.originalInput=document.getElementById('portal');window.originalInput.value='vpn.example.com';document.title='ready';</script>".to_vec())
                    .unwrap()
            })
            .setup(|app| {
                let worker_app = app.handle().clone();
                assert!(std::thread::spawn(move || {
                    PanelBuilder::<_, RegressionPanel>::new(&worker_app, "worker-window")
                        .build().is_err()
                }).join().unwrap());
                assert!(app.get_webview_window("worker-window").is_none());
                let url = tauri::WebviewUrl::External("conversion-test://localhost".parse()?);
                WebviewWindow::builder(app, "existing-window", url.clone())
                    .visible(false).focused(false).transparent(true)
                    .resizable(false).inner_size(280.0, 420.0)
                    .on_document_title_changed(title_changed)
                    .build()?;
                PanelBuilder::<_, RegressionPanel>::new(app.handle(), "builder-window")
                    .url(url)
                    .style_mask(StyleMask::empty().borderless().nonactivating_panel())
                    .corner_radius(14.0)
                    .with_window(|window| window.visible(false).focused(false).transparent(true)
                        .resizable(false).inner_size(280.0, 420.0)
                        .on_document_title_changed(title_changed))
                    .build()?;
                Ok(())
            })
            .run(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Native panel conversion regression");
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    regression::run();
}
