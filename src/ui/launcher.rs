//! Musicacord: buttons in Settings that start the person's other apps, listed
//! in `Applis.txt` beside the executable, one `Name = path` a line. A script
//! `Lancer <Name>.vbs` beside an app's executable starts in its place, as the
//! apps' own shortcuts do. Explorer starts each, so an app that asks for an
//! administrator gets Windows' prompt.

use std::path::{Path, PathBuf};

use slint::{ModelRc, VecModel};

use super::*;

const FILE: &str = "Applis.txt";

/// What a new list says, without anyone's paths.
const TEMPLATE: &str = "\
# Les applis que la page Musicacord des réglages lance, une par ligne :
#   Nom = chemin de l'exe
# Si un script « Lancer Nom.vbs » est à côté de l'exe, il est lancé à sa place.
# Les lignes qui commencent par # sont ignorées. Exemple :
# Voicemeeter = C:\\Program Files (x86)\\VB\\Voicemeeter\\voicemeeter8x64_vmchroma.exe
";

fn file() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join(FILE))
}

/// The apps listed, by name and executable, in their order.
fn apps() -> Vec<(String, PathBuf)> {
    let Some(text) = file().and_then(|file| std::fs::read_to_string(file).ok()) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let (name, path) = line.split_once('=')?;
            let (name, path) = (name.trim(), path.trim().trim_matches('"'));
            (!name.is_empty() && !path.is_empty()).then(|| (name.to_owned(), PathBuf::from(path)))
        })
        .collect()
}

/// What starts for an app: its `Lancer <Name>.vbs`, or its executable.
fn target(name: &str, executable: &Path) -> PathBuf {
    executable
        .parent()
        .map(|folder| folder.join(format!("Lancer {name}.vbs")))
        .filter(|script| script.is_file())
        .unwrap_or_else(|| executable.to_owned())
}

fn start(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let child = std::process::Command::new("explorer.exe").arg(path).spawn();
    #[cfg(not(windows))]
    let child = std::process::Command::new("xdg-open").arg(path).spawn();
    child.map(drop)
}

/// Lists the apps on the Musicacord page.
pub(super) fn show(window: &YapfastWindow) {
    let rows: Vec<LaunchApp> = apps()
        .into_iter()
        .map(|(name, executable)| LaunchApp {
            found: executable.is_file(),
            name: name.into(),
        })
        .collect();
    window.set_launch_apps(ModelRc::new(VecModel::from(rows)));
    window.set_launch_file(
        file()
            .map(|file| file.display().to_string())
            .unwrap_or_default()
            .into(),
    );
}

pub(super) fn bind(window: &YapfastWindow) {
    let weak = window.as_weak();
    window.on_launch_app(move |index| {
        // Read again, so an edit to the list counts without restarting.
        let apps = apps();
        let Some((name, executable)) = usize::try_from(index).ok().and_then(|index| apps.get(index))
        else {
            return;
        };
        let started = if executable.is_file() {
            start(&target(name, executable))
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "introuvable"))
        };
        if let Err(error) = started {
            tracing::warn!(%error, app = %name, path = %executable.display(), "could not start an app");
        }
        if let Some(window) = weak.upgrade() {
            show(&window);
        }
    });
    let weak = window.as_weak();
    window.on_edit_launch_file(move || {
        let Some(file) = file() else { return };
        if !file.exists()
            && let Err(error) = std::fs::write(&file, TEMPLATE)
        {
            tracing::warn!(%error, "could not make the app list");
            return;
        }
        if let Err(error) = start(&file) {
            tracing::warn!(%error, "could not open the app list");
        }
        if let Some(window) = weak.upgrade() {
            show(&window);
        }
    });
}
