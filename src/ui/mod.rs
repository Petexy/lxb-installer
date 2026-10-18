//! The controller-first interface. Owned by the interface work.
pub mod app;
pub mod asked;
pub mod board;
pub mod button;
pub mod cards;
pub mod change;
pub mod column;
pub mod demo;
pub mod field;
#[cfg(test)]
mod fit;
pub mod flags;
pub mod glyphs;
pub mod install;
pub mod legend;
pub mod log;
pub mod map;
pub mod mark;
pub mod menus;
pub mod motion;
pub mod pair;
pub mod panel;
pub mod partitions;
pub mod pictures;
pub mod progress;
pub mod screens;
pub mod search;
pub mod sheet;
pub mod spots;
pub mod text;
pub mod toggle;
pub mod trail;
pub mod words;

pub use app::{Machine, Screen, State, SCENES};
