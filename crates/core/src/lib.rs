//! openclicky core: data model (BUILD-SPEC §4), UI events, and resource slots (§8).

pub mod event;
pub mod model;
pub mod resources;

pub use event::Event;
pub use resources::{ResourceError, ResourceKind, ResourceStore, SlotEntry};
