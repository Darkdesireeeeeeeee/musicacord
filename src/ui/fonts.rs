//! Musicacord: the typeface of the whole window, chosen in Appearance among
//! the fonts installed on the computer, or Musicacord's own. Text size is
//! Messages' text size, and text colour a custom theme's.

use slint::{Model, ModelRc, SharedString, VecModel, fontique_011::shared_collection};

use super::*;

/// The first choice, which keeps the typeface `ui/yapfast.slint` names.
const OWN: &str = "(Police de Musicacord)";

/// The installed families, by name, without case, once each.
fn families() -> Vec<String> {
    let mut names: Vec<String> = shared_collection()
        .family_names()
        .map(str::to_owned)
        .collect();
    names.sort_unstable_by_key(|name| name.to_lowercase());
    names.dedup();
    names
}

/// Lists the fonts in Appearance and shows the window in `chosen`, or in
/// Musicacord's own typeface for none or one no longer installed.
pub(super) fn show(window: &YapfastWindow, chosen: Option<&str>) {
    let names = families();
    let index = chosen
        .and_then(|chosen| names.iter().position(|name| name == chosen))
        .map_or(0, |found| found + 1);
    let family = if index == 0 { "" } else { &names[index - 1] };
    window.set_font_family(family.into());
    let choices: Vec<SharedString> = std::iter::once(OWN.into())
        .chain(names.iter().map(|name| SharedString::from(name.as_str())))
        .collect();
    window.set_fonts(ModelRc::new(VecModel::from(choices)));
    window.set_font_index(index as i32);
}

pub(super) fn bind(window: &YapfastWindow, adapter: &Rc<RefCell<Adapter>>) {
    let adapter = Rc::downgrade(adapter);
    let weak = window.as_weak();
    window.on_font_chosen(move |index| {
        let (Some(adapter), Some(window)) = (adapter.upgrade(), weak.upgrade()) else {
            return;
        };
        let chosen = usize::try_from(index)
            .ok()
            .filter(|index| *index > 0)
            .and_then(|index| window.get_fonts().row_data(index))
            .map(|name| name.to_string());
        window.set_font_family(chosen.as_deref().unwrap_or_default().into());
        window.set_font_index(index.max(0));
        let mut adapter = adapter.borrow_mut();
        adapter.layout.font_family = chosen;
        adapter.layout.write(&adapter.root);
    });
}
