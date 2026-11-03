//! A single caption window for the lifetime of the app. Native close is async:
//! destroying and immediately reusing its label can leave an orphan HWND.
//! Serialize callers in Rust (including callers outside the main renderer),
//! and hide/reuse the window until the application itself exits.
use anyhow::Result;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

static GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn show(app: tauri::AppHandle) -> Result<()> {
    let _guard = GATE.lock().await;
    let window = if let Some(window) = app.get_webview_window("caption") {
        window
    } else {
        // build.dev_url remains present in production configuration. Only debug
        // builds may load Vite; installed captions must use bundled assets.
        let url = if let Some(dev_url) = app.config().build.dev_url.as_ref().filter(|_| cfg!(debug_assertions)) {
            WebviewUrl::External(tauri::Url::parse(&format!(
                "{}/caption.html",
                dev_url.as_str().trim_end_matches('/')
            ))?)
        } else {
            WebviewUrl::App("caption.html".into())
        };
        WebviewWindowBuilder::new(&app, "caption", url)
            .title("Innote — Phụ đề")
            .inner_size(420.0, 150.0)
            .min_inner_size(180.0, 90.0)
            .always_on_top(true)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .skip_taskbar(true)
            .resizable(true)
            .visible(false)
            .build()?
    };
    window.show()?;
    window.set_focus()?;
    app.emit("caption-window-visibility", true)?;
    Ok(())
}

pub async fn hide(app: tauri::AppHandle) -> Result<()> {
    let _guard = GATE.lock().await;
    if let Some(window) = app.get_webview_window("caption") {
        window.hide()?;
    }
    app.emit("caption-window-visibility", false)?;
    Ok(())
}

pub fn hide_later(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if hide(app.clone()).await.is_err() {
            let _ = app.emit("caption-layout-error", ());
        }
    });
}
