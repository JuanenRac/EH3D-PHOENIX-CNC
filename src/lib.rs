//! The base of the EH3D machine-control program that will hold the drilling mode, the routing mode and the 2D and 3D CNC work. For now it
//! is the view of a job (the scene on the table, the themes and the words of the interface), which lives in `eh3d-view` of EH3D-COMMON
//! and is shared with the other programs of the suite; it is re-exported here so that this program uses it under its own name.

pub use eh3d_view::{crash, scene, texts, theme};

pub mod boxes;
pub mod cli;
pub mod complete;
pub mod estimate;
pub mod gcode;
#[cfg(feature = "gui")]
pub mod gui;
pub mod heightmap;
pub mod jobs;
pub mod jog;
pub mod link;
pub mod lint;
pub mod logbook;
pub mod macros;
pub mod plan;
pub mod probe;
pub mod progcheck;
pub mod queue;
pub mod ranges;
pub mod recovery;
pub mod report;
pub mod server;
pub mod session;
pub mod settings;
pub mod sim;
pub mod stats;
pub mod tuning;
pub mod view;
pub mod viewer;
pub mod words;
pub mod workreport;
