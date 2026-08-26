#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    use tao::{
        dpi::LogicalSize,
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
    };
    use wry::WebViewBuilder;

    let mut args = std::env::args().skip(1);
    let url = args
        .next()
        .unwrap_or_else(|| "http://127.0.0.1:48750".to_owned());
    let title = args
        .next()
        .unwrap_or_else(|| "Ghost & Guild".to_owned());
    let width = args
        .next()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(1180.0);
    let height = args
        .next()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(820.0);

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(LogicalSize::new(width, height))
        .build(&event_loop)
        .expect("failed to create Ghost webview window");
    let _webview = WebViewBuilder::new()
        .with_url(&url)
        .build(&window)
        .expect("failed to create Ghost WebView2 view");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
}

#[cfg(not(windows))]
fn main() {
    eprintln!("ghost-fl-runtime-webview is only supported on Windows");
}
