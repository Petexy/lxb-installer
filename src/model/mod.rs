//! What the interface and the worker agree on. Everything in `model` is
//! plain data and pure functions: nothing here writes to the machine, so all
//! of it can be tested on a development host.
pub mod catalogs;
pub mod config;
pub mod disks;
pub mod firmware;
pub mod keyboards;
pub mod layout;
pub mod live;
pub mod measure;
pub mod places;
pub mod protocol;
pub mod secret;
