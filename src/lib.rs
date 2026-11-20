//! The base of the EH3D machine-control program that will hold the drilling mode, the routing mode and the 2D and 3D CNC work. For now it
//! is the view of a job (the scene on the table, the themes and the words of the interface), which lives in `eh3d-view` of EH3D-COMMON
//! and is shared with the other programs of the suite; it is re-exported here so that this program uses it under its own name.

pub use eh3d_view::{crash, scene, texts, theme};

pub mod atc;
pub mod bookmarks;
pub mod boxes;
pub mod cli;
pub mod cli_tools;
pub mod cncroutes;
pub mod compare;
pub mod complete;
pub mod consumption;
pub mod ctltext;
pub mod cutcomp;
pub mod cycles;
pub mod dialect;
pub mod editor;
pub mod estimate;
pub mod faulty;
pub mod gcode;
pub mod gcodehelp;
pub mod guard;
#[cfg(feature = "gui")]
pub mod gui;
pub mod heightmap;
pub mod holder;
pub mod jobs;
pub mod jog;
pub mod life;
pub mod link;
pub mod lint;
pub mod logbook;
pub mod macrocheck;
pub mod macros;
pub mod materials;
pub mod motioncfg;
pub mod offsets;
pub mod plan;
pub mod postcheck;
pub mod probe;
pub mod problem;
pub mod profiles;
pub mod progcheck;
pub mod queue;
pub mod ranges;
pub mod recovery;
pub mod report;
pub mod restart;
pub mod server;
pub mod serverlink;
pub mod session;
pub mod settings;
pub mod sim;
pub mod startline;
pub mod stats;
pub mod templates;
pub mod timesplit;
pub mod tools;
pub mod transform;
pub mod tuning;
pub mod units;
pub mod view;
pub mod viewer;
pub mod wcs;
pub mod words;
pub mod workreport;

#[cfg(test)]
mod fuzz;
