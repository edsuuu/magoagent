use super::*;

/// The header (globe and name) and the address bar above the page, in points.
pub(crate) const PORTAL_HEADER: f32 = 32.0;
pub(crate) const PORTAL_BAR: f32 = 40.0;
pub(crate) const PORTAL_CHROME: f32 = PORTAL_HEADER + PORTAL_BAR;

/// The new / edit portal dialog.
#[derive(Clone, Debug)]
pub(crate) struct PortalDialog {
    /// The portal being edited; None creates one in `at`.
    pub(crate) target: Option<String>,
    pub(crate) at: Rect,
    pub(crate) spec: PortalSpec,
}

/// What pages report back (on the UI thread, from wry's handlers).
#[cfg(not(target_os = "linux"))]
pub(crate) enum PortalEvent {
    /// A tab finished loading an address (a link clicked inside the page, too).
    Loaded { portal: String, tab: u64, url: String },
    /// A link asked for a new window: the page goes there, since a portal shows one page.
    NewTab { portal: String, url: String },
    /// A tab started loading another page: what lived in the old one is gone.
    Navigating { portal: String, tab: u64 },
    /// A message from the Design overlay in a tab's page (design.rs).
    Design { portal: String, tab: u64, message: String },
    /// The picture of a region the stroke made: `seq` tells it from the later strokes.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))] // no picture on Windows yet
    Shot { portal: String, seq: u64, png: Option<Vec<u8>> },
}

/// An address the webview can load: "exemplo.com" → "https://exemplo.com". A scheme counts only
/// at the start: "exemplo.com/?r=http://x" is still a site.
pub(crate) fn normalize_url(url: &str) -> String {
    let url = url.trim();
    if url.is_empty() {
        "about:blank".into()
    } else if has_scheme(url) || url.starts_with("about:") {
        url.into()
    } else if url.starts_with("localhost") || url.starts_with("127.") {
        format!("http://{url}")
    } else {
        format!("https://{url}")
    }
}

/// Maps each new tab to a live page, preserving pages across closes and appends.
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub(super) fn match_portal_tabs(live: &[String], next: &[String]) -> Vec<Option<usize>> {
    if live.len() == next.len() {
        return (0..next.len()).map(Some).collect();
    }

    let mut lengths = vec![vec![0; next.len() + 1]; live.len() + 1];
    for i in (0..live.len()).rev() {
        for j in (0..next.len()).rev() {
            lengths[i][j] = if live[i] == next[j] { lengths[i + 1][j + 1] + 1 } else { lengths[i + 1][j].max(lengths[i][j + 1]) };
        }
    }

    let mut matched = vec![None; next.len()];
    let (mut i, mut j) = (0, 0);
    while i < live.len() && j < next.len() {
        if live[i] == next[j] {
            matched[j] = Some(i);
            i += 1;
            j += 1;
        } else if lengths[i + 1][j] >= lengths[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    matched
}

/// "https://…", "file:///…": a scheme (a letter, then letters, digits, `+`, `-` or `.`) and `://`.
pub(super) fn has_scheme(url: &str) -> bool {
    url.split_once("://").is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic()) && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    })
}

/// The web: what may go to the system's browser, or move a portal when a page asks for a window.
/// Other schemes (mailto:, ms-settings:, a custom app's) would launch programs.
pub(crate) fn is_web(url: &str) -> bool {
    let url = url.trim_start().to_ascii_lowercase();
    url.starts_with("http://") || url.starts_with("https://")
}

/// "https://docs.rs/egui/latest" → "docs.rs".
pub(crate) fn host_of(url: &str) -> &str {
    url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or_default()
}

/// The address as the bar shows it: no scheme.
pub(super) fn display_url(url: &str) -> &str {
    url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or(url)
}

/// Opens an address in the system's browser, or a folder in the file manager.
pub(crate) fn open_external(url: &str) {
    #[cfg(windows)]
    {
        // straight to the shell: `cmd /c start` parsed the address a second time
        #[link(name = "shell32")]
        unsafe extern "system" {
            fn ShellExecuteW(hwnd: isize, op: *const u16, file: *const u16, params: *const u16, dir: *const u16, show: i32) -> isize;
        }
        let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
        let (op, file) = (wide("open"), wide(url));
        const SW_SHOWNORMAL: i32 = 1;
        // SAFETY: both strings are NUL-terminated and outlive the call
        unsafe { ShellExecuteW(0, op.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL) };
    }
    #[cfg(not(windows))]
    {
        // waited on, off the UI thread, so no zombie stays
        let (program, url) = (if cfg!(target_os = "macos") { "open" } else { "xdg-open" }, url.to_owned());
        std::thread::spawn(move || std::process::Command::new(program).arg(url).status());
    }
}

/// One tab's live page.
#[cfg(not(target_os = "linux"))]
pub(crate) struct TabView {
    pub(crate) webview: wry::WebView,
    pub(crate) uid: u64,
    /// What it was last told to load (or reported loading), so the URL isn't re-sent.
    pub(super) url: String,
    /// From the page's start to its finish: the bar shows × to stop it meanwhile.
    pub(super) loading: bool,
    /// The secret the Design overlay in this tab's page was armed with (design.rs).
    pub(crate) secret: Option<String>,
}

#[cfg(not(target_os = "linux"))]
impl TabView {
    /// wry drops a page's `ipc.postMessage` when the page's URL isn't an `http::Uri`, as file:///C:/…
    /// on Windows: there Design's overlay keeps its messages and the app fetches them each frame.
    pub(crate) fn polls_design(&self) -> bool {
        cfg!(windows) && self.url.starts_with("file:")
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) struct PortalView {
    pub(crate) tabs: Vec<TabView>,
    /// The portal's own profile (cookies, logins), in a folder beside the drawing; its tabs share
    /// it. After `tabs`: the pages go before the profile they use.
    pub(super) context: wry::WebContext,
    /// Box, zoom, open tab and the holes cut in it last applied, so unchanged values aren't
    /// re-sent every frame.
    pub(super) shown: Option<(Rect, f32, usize, Vec<Rect>, f32)>,
}

#[cfg(not(target_os = "linux"))]
pub(super) fn new_tab_view(
    window: &impl wry::raw_window_handle::HasWindowHandle,
    portal: &str,
    url: &str,
    events: &std::sync::mpsc::Sender<PortalEvent>,
    ctx: &Context,
    web_context: &mut wry::WebContext,
    extras: &crate::app::browser::extras::TabExtras,
) -> wry::Result<TabView> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let uid = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let (loaded, new_window, note) = (events.clone(), events.clone(), events.clone());
    let (portal_loaded, portal_window, portal_note) = (portal.to_string(), portal.to_string(), portal.to_string());
    let (ctx_note, ctx_loaded) = (ctx.clone(), ctx.clone());
    let webview = crate::app::browser::extras::tune(wry::WebViewBuilder::new_with_web_context(web_context), extras)
        .with_url(url)
        .with_initialization_script(crate::app::browser::console::initialization_script())
        .with_visible(false)
        // a page made does not take the keyboard (a click on it does): who is typing in a terminal
        // keeps the keys when an agent opens a portal. On Windows it is more than manners: a page
        // taking the focus while the next one is being made (two tabs opened back to back) left
        // that one's making waiting forever, and the whole interface with it
        .with_focused(false)
        .with_on_page_load_handler(move |event, url| {
            let portal = portal_loaded.clone();
            let _ = loaded.send(match event {
                wry::PageLoadEvent::Started => PortalEvent::Navigating { portal, tab: uid },
                wry::PageLoadEvent::Finished => PortalEvent::Loaded { portal, tab: uid, url },
            });
            ctx_loaded.request_repaint();
        })
        .with_new_window_req_handler(move |url, _| {
            let _ = new_window.send(PortalEvent::NewTab { portal: portal_window.clone(), url });
            wry::NewWindowResponse::Deny
        })
        .with_ipc_handler(move |request| {
            // a page can post any size: past what Design sends, it stops here, before the queue
            let message = request.into_body();
            if message.len() <= crate::app::browser::design::MESSAGE_MAX {
                let _ = note.send(PortalEvent::Design { portal: portal_note.clone(), tab: uid, message });
                ctx_note.request_repaint();
            }
        })
        .build_as_child(window)?;
    // wry anchors a child page to the window's bottom edge (ViewMinYMargin), so a window that
    // settles at another height would move the page off its card; its place is set from the
    // canvas instead, so AppKit must leave it alone
    #[cfg(target_os = "macos")]
    wry::WebViewExtMacOS::webview(&webview).setAutoresizingMask(objc2_app_kit::NSAutoresizingMaskOptions::empty());
    Ok(TabView { webview, uid, url: url.into(), loading: true, secret: None })
}

/// Leaves `holes` (window points) out of a page's native view, so the app's floating UI over it
/// shows through while the page keeps its size; `origin` is where the view's box starts.
// ponytail: square holes, so a rounded menu's corners show the canvas instead of the page; a
// CreateRoundRectRgn per hole with the egui corner radius if that ever shows
#[cfg(windows)]
pub(super) fn cut_page(webview: &wry::WebView, origin: Pos2, holes: &[Rect], pixels_per_point: f32) {
    use wry::WebViewExtWindows;
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateRectRgn(left: i32, top: i32, right: i32, bottom: i32) -> isize;
        fn CombineRgn(target: isize, a: isize, b: isize, mode: i32) -> i32;
        fn DeleteObject(object: isize) -> i32;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn SetWindowRgn(hwnd: isize, region: isize, redraw: i32) -> i32;
    }
    const RGN_DIFF: i32 = 4;
    // wry's own container window, which holds the page's
    let hwnd = webview.hwnd().0 as isize;
    let px = |v: f32| (v * pixels_per_point).round() as i32;
    // SAFETY: plain GDI handles; the cut-outs are freed here, the shape goes to the window
    unsafe {
        if holes.is_empty() {
            SetWindowRgn(hwnd, 0, 1);
            return;
        }
        let shape = CreateRectRgn(0, 0, 1 << 20, 1 << 20);
        for hole in holes {
            let (min, max) = (hole.min - origin, hole.max - origin);
            let cut = CreateRectRgn(px(min.x), px(min.y), px(max.x), px(max.y));
            CombineRgn(shape, shape, cut, RGN_DIFF);
            DeleteObject(cut);
        }
        SetWindowRgn(hwnd, shape, 1);
    }
}

/// A clicked part of a portal's address bar.
pub(super) enum Chrome {
    Back,
    Forward,
    Reload,
    Stop,
    Navigate(String),
}

/// How long after the card last moved its page comes back (seconds).
#[cfg(windows)]
pub(super) const PAGE_SETTLE: f64 = 0.15;
/// WebView2's smallest page zoom (25%).
pub(super) const MIN_PAGE_ZOOM: f32 = 0.25;
