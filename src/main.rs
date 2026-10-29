#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use gpui::{
    AnyView, AnyWindowHandle, App, Bounds, Entity, WindowBounds, WindowOptions, prelude::*, px, size,
};
use gpui_component::Root;
use mangue::assets::Assets;
use mangue::canvas::{Canvas, MenuShell, actions};
use mangue::model::Definitions;

/// Loaded startup document and the path and error when it failed to load.
type Startup = (Option<(Definitions, PathBuf)>, Option<(PathBuf, String)>);

/// Returns the document to show for an optional path (none when no path is
/// given or it cannot be loaded) and the load error, if any.
fn load_definitions(arg: Option<PathBuf>) -> Startup {
    let Some(path) = arg else {
        return (None, None);
    };

    match mangue::canvas::persist::open(&path) {
        Ok(defs) => (Some((defs, path)), None),
        Err(err) => (None, Some((path, err))),
    }
}

/// The canvas a window's gpui-component `Root` wraps, directly or inside the
/// in-window `MenuShell`, if it wraps one.
fn canvas_of(window: AnyWindowHandle, cx: &App) -> Option<Entity<Canvas>> {
    let view = window.downcast::<Root>()?.read(cx).ok()?.view().clone();

    match view.clone().downcast::<Canvas>() {
        Ok(canvas) => Some(canvas),
        Err(_) => Some(view.downcast::<MenuShell>().ok()?.read(cx).canvas.clone()),
    }
}

/// Opens `path` in a new tab of the frontmost canvas window; returns whether
/// such a window existed.
fn open_in_frontmost(path: &Path, cx: &mut App) -> bool {
    let windows = cx.window_stack().unwrap_or_else(|| cx.windows());
    let Some((window, canvas)) = windows
        .into_iter()
        .find_map(|w| canvas_of(w, cx).map(|canvas| (w, canvas)))
    else {
        return false;
    };

    let _ = window.update(cx, |_, window, cx| {
        canvas.update(cx, |canvas, cx| canvas.load(path.to_path_buf(), window, cx));
        window.activate_window();
    });
    cx.activate(true);

    true
}

/// Opens a path in a tab of the frontmost canvas window, or a new canvas
/// window (with no document when `None`) when none exists, and brings the app to the
/// foreground so it shows up in the current Space.
fn open_canvas(path: Option<PathBuf>, cx: &mut App) {
    if let Some(path) = &path
        && open_in_frontmost(path, cx)
    {
        return;
    }

    let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
    let (document, failure) = load_definitions(path);
    if let Some((_, path)) = &document {
        mangue::settings::recent::record(path, cx);
    }

    let window = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let canvas = cx.new(|cx| match document {
                    Some((defs, path)) => Canvas::new(defs, Some(path), window, cx),
                    None => Canvas::empty(window, cx),
                });
                let view: AnyView = if cfg!(target_os = "macos") {
                    canvas.into()
                } else {
                    cx.new(|cx| MenuShell::new(canvas, cx)).into()
                };

                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .unwrap();

    if let Some((path, err)) = failure
        && let Some(canvas) = canvas_of(window.into(), cx)
    {
        let _ = window.update(cx, |_, window, cx| {
            canvas.update(cx, |canvas, cx| {
                canvas.alert_open_failure(&path, &err, window, cx)
            })
        });
    }

    let _ = window.update(cx, |_, window, _| window.activate_window());
    cx.activate(true);
}

/// Boots the GPUI application: embedded assets, key bindings, menus, the main
/// canvas window and the Open Documents (Finder / `open file.bpmn`) handler.
fn main() {
    let app = gpui_platform::application().with_assets(Assets);
    let (tx, rx) = mpsc::channel::<PathBuf>();

    app.on_open_urls(move |urls| {
        for path in urls
            .iter()
            .filter_map(|url| mangue::files::url_to_path(url))
        {
            let _ = tx.send(path);
        }
    });

    app.run(move |cx: &mut App| {
        mangue::canvas::init_editor(cx);
        mangue::settings::install(cx);
        actions::install(cx);

        open_canvas(std::env::args().nth(1).map(PathBuf::from), cx);

        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;

                while let Ok(path) = rx.try_recv() {
                    let _ = cx.update(|cx| open_canvas(Some(path), cx));
                }
            }
        })
        .detach();
    });
}
