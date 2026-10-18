//! The `chosen` mark: the tick on the row, card or toggle whose value is in
//! force.
//!
//! The toolkit's (`Ui::chosen`): the shell's tick cut through a bead, in the
//! glossy material every mark of the set wears, ringed in the panel's dark
//! glass. The ring is there because the bead on its own all but vanishes on
//! a lit row — white water on the light's bright violet with the same violet
//! through its tick — and it is the set's material rather than a flat disc
//! under a grey bead: the ring is a bead of that material round the mark,
//! never under it, so the mark shows the row through it as Review's marks
//! do.
//!
//! The ring and the mark are counted with the words (`Ui::words_between`), so
//! a page coming in brings them up with its words rather than with its glass.

use lxb_app::lxb_render::Ui;
use lxb_app::lxb_toolkit::settings::IconStyle;

/// Draw the `chosen` mark in the square `rect`, at `alpha`.
pub fn chosen(ui: &mut Ui, rect: [f32; 4], icons: IconStyle, alpha: f32) {
    let from = ui.drawn();
    ui.chosen(rect, icons, alpha);
    let to = ui.drawn();
    ui.words_between(from, to);
}

#[cfg(test)]
mod tests {

    #[test]
    fn the_mark_is_one_the_toolkit_has() {
        assert!(lxb_app::lxb_toolkit::assets::glyph("chosen").is_some());
    }
}
