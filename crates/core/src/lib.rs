//! openclicky core: data model (§4), UI events, resource slots (§8), the note DSL (§7),
//! the report contract and prompt assembly (§6).

pub mod event;
pub mod model;
pub mod note;
pub mod prompt;
pub mod report;
pub mod resources;

pub use event::Event;
pub use resources::{ResourceError, ResourceKind, ResourceStore, SlotEntry};
