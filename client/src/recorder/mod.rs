//! Recording subsystem for capturing video with sensor overlays and chart compositing

pub mod types;
pub mod utils;
pub mod sensors;
pub mod canvas_renderer;
pub mod media_streams;
pub mod media_recorder;
pub mod storage;
pub mod state;
pub mod ui;

// Re-export main entry points
pub use state::RecorderState;
pub use ui::init_recorder_panel;
