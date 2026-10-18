//! Where a pointer can land, by number.
//!
//! Every control the installer draws writes its rectangle down under a number
//! with `Ui::spot`, and a click comes back as that number through
//! `Page::pressed`. The numbers are kept here, in one place, so two parts of a
//! screen can never claim the same one and a press be answered by whichever
//! wrote its target down last.
//!
//! All of them stand clear of the toolkit's own flow, which numbers the
//! controls it draws from nought upwards, and of the legend at `0x8000`.

/// The buttons along the foot of a screen's content (Continue, Install…):
/// `BUTTONS + index`.
pub const BUTTONS: u32 = 0x0300;

/// A screen's own one-off controls (a toggle row, a lone row, a field).
/// `ITEMS + index`, numbered by the screen.
pub const ITEMS: u32 = 0x0400;

/// The text fields: `FIELDS + FieldId::index`.
pub const FIELDS: u32 = 0x0500;

/// The cards of the desktop step: `CARDS + index`.
pub const CARDS: u32 = 0x0600;

/// The disks' chips over Partitions' map: `CHIPS + index`, the disk's place
/// among the machine's disks.
pub const CHIPS: u32 = 0x0700;

/// The tiles of Partitions' map: `TILES + index`, in the order the light
/// walks them — a block of its own, roomy enough for a gpt table's 128
/// partitions and the free space between every two of them.
pub const TILES: u32 = 0x0900;

/// The rows of a sheet over Partitions (New partition, Resize, Label…,
/// Mount point · Other…): `SHEET + index`, in the order the sheet lists
/// them; its buttons are the row of buttons' own (`BUTTONS`).
pub const SHEET: u32 = 0x0e00;

/// The track of a sheet's held size, which a pointer takes hold of and
/// drags along: a bar (`Page::dragging`).
pub const SHEET_TRACK: u32 = SHEET + 0xf0;

/// The room a sheet's size can take on the map — the tile it sizes and the
/// unallocated space beside it — which a pointer takes hold of to drag the
/// tile's edge, or clicks along: a bar too.
pub const SHEET_ROOM: u32 = SHEET + 0xf1;

/// The keys of the on-screen board: `BOARD + index`.
pub const BOARD: u32 = 0x0800;

/// The first of the scrolled columns. Each column has a block of its own:
/// see [`Column`].
pub const COLUMNS: u32 = 0x1000;

/// How many numbers one column's block holds: its rows, then the viewport
/// behind them and the bar beside them at the end of the block.
pub const COLUMN_BLOCK: u32 = 0x1000;

/// The legend's own block, kept where the siblings keep it.
pub const LEGEND: u32 = 0x8000;

/// The numbers one scrolled column answers to.
///
/// A row is `first + index`; the viewport behind the rows (so a wheel over
/// the gap between two rows still finds the column) and the scroll bar beside
/// it take the last two numbers of the block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    pub first: u32,
}

impl Column {
    /// The `slot`th column on a screen: nought for the left-hand one, one for
    /// the right-hand one of a pair.
    pub const fn nth(slot: u32) -> Self {
        Self {
            first: COLUMNS + slot * COLUMN_BLOCK,
        }
    }

    pub const fn row(self, index: usize) -> u32 {
        self.first + index as u32
    }

    pub const fn viewport(self) -> u32 {
        self.first + COLUMN_BLOCK - 2
    }

    pub const fn bar(self) -> u32 {
        self.first + COLUMN_BLOCK - 1
    }

    /// Whether a pointer over `id` is over this column at all: one of its
    /// rows, the space between them, or its bar.
    pub const fn owns(self, id: u32) -> bool {
        id >= self.first && id < self.first + COLUMN_BLOCK
    }

    /// The row a spot number is, if it is one of this column's rows.
    pub fn row_of(self, id: u32, rows: usize) -> Option<usize> {
        let index = id.checked_sub(self.first)? as usize;
        (index < rows).then_some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_blocks_overlap() {
        let left = Column::nth(0);
        let right = Column::nth(1);
        assert!(!left.owns(right.row(0)));
        assert!(!right.owns(left.bar()));
        assert!(left.owns(left.viewport()) && left.owns(left.bar()));
        assert!(
            Column::nth(6).bar() < LEGEND,
            "a column ran into the legend"
        );
        for fixed in [
            BUTTONS, ITEMS, FIELDS, CARDS, CHIPS, BOARD, TILES, SHEET_ROOM,
        ] {
            assert!(fixed < COLUMNS, "{fixed:#x} sits inside the columns");
        }
        // The keys of the board end before the tiles begin, the tiles
        // before a sheet's rows, and those before the columns.
        const { assert!(BOARD + 0x100 <= TILES && TILES + 0x0500 <= SHEET) };
        const { assert!(SHEET_ROOM < SHEET + 0x100 && SHEET + 0x100 <= COLUMNS) };
    }

    #[test]
    fn a_row_number_is_read_back_only_for_rows_that_exist() {
        let column = Column::nth(1);
        assert_eq!(column.row_of(column.row(3), 10), Some(3));
        assert_eq!(column.row_of(column.row(3), 3), None);
        assert_eq!(column.row_of(column.viewport(), 10), None);
        assert_eq!(column.row_of(Column::nth(0).row(0), 10), None);
    }
}
