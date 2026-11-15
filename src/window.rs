use crate::browser::web_url;
use std::{env, fmt::Write, path::PathBuf};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use webview2_com::SourceChangedEventHandler;
use wry::{dpi, NewWindowResponse, Rect, WebContext, WebViewBuilder, WebViewExtWindows};

const TOOLBAR_HEIGHT: f64 = 44.0;
const TOOLBAR: &str = include_str!("window_toolbar.html");

enum Action {
    Back,
    Forward,
    Reload,
    Navigate(String),
    LocationChanged,
    TitleChanged(String),
}

fn bounds(width: f64, height: f64, toolbar: bool) -> Rect {
    let bar = TOOLBAR_HEIGHT.min(height);
    let (top, extent) = if toolbar {
        (0.0, bar)
    } else {
        (bar, height - bar)
    };
    Rect {
        position: dpi::LogicalPosition::new(0.0, top).into(),
        size: dpi::LogicalSize::new(width, extent).into(),
    }
}

pub fn open(address: &str) -> Result<(), String> {
    let initial = web_url(address)?;
    let event_loop = EventLoopBuilder::<Action>::with_user_event().build();
    let notify = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("Surfbird")
        .with_inner_size(LogicalSize::new(1100.0, 720.0))
        .build(&event_loop)
        .map_err(|e| e.to_string())?;
    let profile = PathBuf::from(env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unset")?)
        .join("Surfbird")
        .join("WebView2");
    let mut context = WebContext::new(Some(profile));
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());

    // Only the bundled toolbar can issue browser commands. It has no external
    // subresources, accepts no navigation to web pages and shares no DOM with them.
    let toolbar_events = notify.clone();
    let mut toolbar_url = String::from("data:text/html;charset=utf-8,");
    for byte in TOOLBAR.bytes() {
        write!(toolbar_url, "%{byte:02X}").expect("writing into a String");
    }
    let bar = WebViewBuilder::new_with_web_context(&mut context)
        .with_bounds(bounds(size.width, size.height, true))
        .with_url(&toolbar_url)
        .with_autoplay(false)
        .with_download_started_handler(|_, _| false)
        .with_navigation_handler(move |url| url == toolbar_url)
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_ipc_handler(move |message| {
            let action = match message.body().as_str() {
                "back" => Action::Back,
                "forward" => Action::Forward,
                "reload" => Action::Reload,
                "ready" => Action::LocationChanged,
                url => match web_url(url) {
                    Ok(url) => Action::Navigate(url.into()),
                    Err(_) => return,
                },
            };
            let _ = toolbar_events.send_event(action);
        })
        .build_as_child(&window)
        .map_err(|e| format!("Creating the WebView2 toolbar: {e}"))?;

    let title_events = notify.clone();
    let page = WebViewBuilder::new_with_web_context(&mut context)
        .with_bounds(bounds(size.width, size.height, false))
        .with_url(initial.as_str())
        .with_autoplay(false)
        .with_hotkeys_zoom(true)
        .with_download_started_handler(|_, _| false)
        .with_navigation_handler(|url| web_url(&url).is_ok())
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_document_title_changed_handler(move |title| {
            let _ = title_events.send_event(Action::TitleChanged(title));
        })
        .build_as_child(&window)
        .map_err(|e| format!("Creating the WebView2 page: {e}"))?;
    // SourceChanged includes same-document history updates on single-page sites.
    // The controller retains the callback on this UI thread until the view closes.
    let changed = SourceChangedEventHandler::create(Box::new(move |_, _| {
        let _ = notify.send_event(Action::LocationChanged);
        Ok(())
    }));
    let mut token = 0;
    unsafe { page.webview().add_SourceChanged(&changed, &mut token) }.map_err(|e| e.to_string())?;

    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        let result = match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *flow = ControlFlow::Exit;
                return;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. },
                ..
            } => {
                let size = window.inner_size().to_logical::<f64>(window.scale_factor());
                bar.set_bounds(bounds(size.width, size.height, true))
                    .and_then(|_| page.set_bounds(bounds(size.width, size.height, false)))
            }
            Event::UserEvent(Action::Back) => page.go_back(),
            Event::UserEvent(Action::Forward) => page.go_forward(),
            Event::UserEvent(Action::Reload) => page.reload(),
            Event::UserEvent(Action::Navigate(url)) => page.load_url(&url),
            Event::UserEvent(Action::TitleChanged(title)) => {
                let title = crate::document::clean(&title);
                window.set_title(&if title.is_empty() {
                    "Surfbird".into()
                } else {
                    format!("Surfbird — {}", title.chars().take(160).collect::<String>())
                });
                return;
            }
            Event::UserEvent(Action::LocationChanged) => page.url().and_then(|url| {
                let quoted = serde_json::Value::String(url);
                bar.evaluate_script(&format!("window.showLocation({quoted})"))
            }),
            _ => return,
        };
        if let Err(error) = result {
            eprintln!("Surfbird: {}", crate::document::clean(&error.to_string()));
        }
    })
}
