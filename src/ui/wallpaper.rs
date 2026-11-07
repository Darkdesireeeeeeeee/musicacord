//! Musicacord's wallpaper: the newest picture in the "Fond d'écran" folder
//! beside the executable, drawn behind the whole window. Dropping a picture in
//! the folder changes it within a few seconds, without restarting; emptying the
//! folder takes it away. While it shows, the window's large surfaces turn
//! see-through so the picture shows behind them.

use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use slint::{Color, ComponentHandle, Timer, TimerMode};
use yapfast_ui::YapfastWindow;

/// The folder's name, beside the executable.
const FOLDER: &str = "Fond d'écran";
/// How often the folder is looked at again.
const POLL: Duration = Duration::from_secs(5);
/// The picture's largest side once decoded: larger ones are scaled down, so a
/// 6000 px photo doesn't hold 140 MB of pixels for a 1080p window.
const MAX_SIDE: u32 = 2560;
/// The surfaces' opacity over the picture, out of 255.
const SEE_THROUGH: u8 = 0xc8;
/// Files the folder may hold that are pictures Musicacord can show.
const EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "jfif", "gif", "webp", "bmp"];

thread_local! {
    static TIMER: Timer = Timer::default();
    /// The picture shown, and when it was last written, to load it again only
    /// when it changes.
    static SHOWN: RefCell<Option<(PathBuf, SystemTime)>> = const { RefCell::new(None) };
    /// Whether a picture shows, which [`see_through`] follows.
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
}

/// Starts watching the folder for `window`: now, then every [`POLL`]. A window
/// made again, after closing to the tray, takes over the watch.
pub fn start(window: &YapfastWindow) {
    SHOWN.with(|shown| shown.borrow_mut().take());
    let Some(folder) = folder() else {
        return;
    };
    look(window, &folder);
    let weak = window.as_weak();
    TIMER.with(|timer| {
        timer.start(TimerMode::Repeated, POLL, move || {
            if let Some(window) = weak.upgrade() {
                look(&window, &folder);
            }
        });
    });
}

fn folder() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    Some(executable.parent()?.join(FOLDER))
}

/// Shows the folder's newest picture, or none, when that differs from what
/// shows.
fn look(window: &YapfastWindow, folder: &Path) {
    let newest = newest(folder);
    let unchanged = SHOWN.with(|shown| *shown.borrow() == newest);
    if unchanged {
        return;
    }
    SHOWN.with(|shown| *shown.borrow_mut() = newest.clone());
    let Some((path, _)) = newest else {
        show(window, None);
        return;
    };
    // Decoding a large photo takes a moment; the window keeps drawing meanwhile.
    let weak = window.as_weak();
    let decoding = std::thread::Builder::new()
        .name("wallpaper".into())
        .spawn(move || {
            let pixels = match decode(&path) {
                Ok(pixels) => Some(pixels),
                Err(error) => {
                    tracing::warn!(%error, path = %path.display(), "could not load the wallpaper");
                    None
                }
            };
            let shown = weak.upgrade_in_event_loop(move |window| {
                show(&window, pixels.map(slint::Image::from_rgba8));
            });
            if let Err(error) = shown {
                tracing::warn!(%error, "could not show the wallpaper");
            }
        });
    if let Err(error) = decoding {
        tracing::error!(%error, "could not start loading the wallpaper");
    }
}

/// The newest picture in the folder, by when it was written, as
/// `changer-fond.bat` marks the one it copies.
fn newest(folder: &Path) -> Option<(PathBuf, SystemTime)> {
    std::fs::read_dir(folder)
        .ok()?
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    EXTENSIONS.iter().any(|known| extension.eq_ignore_ascii_case(known))
                })
        })
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            metadata.is_file().then_some(())?;
            Some((entry.path(), metadata.modified().ok()?))
        })
        .max_by_key(|(_, written)| *written)
}

fn decode(path: &Path) -> anyhow::Result<slint::SharedPixelBuffer<slint::Rgba8Pixel>> {
    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let mut picture = reader.decode()?;
    if picture.width() > MAX_SIDE || picture.height() > MAX_SIDE {
        picture = picture.thumbnail(MAX_SIDE, MAX_SIDE);
    }
    let picture = picture.into_rgba8();
    Ok(slint::SharedPixelBuffer::clone_from_slice(
        picture.as_raw(),
        picture.width(),
        picture.height(),
    ))
}

fn show(window: &YapfastWindow, picture: Option<slint::Image>) {
    let active = picture.is_some();
    window.set_wallpaper(picture.unwrap_or_default());
    window.set_has_wallpaper(active);
    let was = ACTIVE.replace(active);
    if was != active {
        see_through(window, true);
    }
}

/// Makes the window's large surfaces see-through while a wallpaper shows.
/// `changed`: the wallpaper came or went, so surfaces go back to opaque when
/// it went; otherwise the theme was just set, opaque, and only a wallpaper
/// that shows changes anything.
pub fn see_through(window: &YapfastWindow, changed: bool) {
    let active = ACTIVE.get();
    if !active && !changed {
        return;
    }
    let alpha = if active { SEE_THROUGH } else { 0xff };
    let tokens = window.global::<yapfast_ui::Theme>();
    let with = |colour: Color| Color::from_argb_u8(alpha, colour.red(), colour.green(), colour.blue());
    tokens.set_rail(with(tokens.get_rail()));
    tokens.set_sidebar(with(tokens.get_sidebar()));
    tokens.set_panel(with(tokens.get_panel()));
    tokens.set_chat(with(tokens.get_chat()));
    tokens.set_stage(with(tokens.get_stage()));
}
