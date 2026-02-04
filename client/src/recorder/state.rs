use crate::recorder::{
    canvas_renderer::CanvasRenderer,
    media_recorder::Recorder,
    sensors::SensorManager,
    types::{SourceType, PipPosition},
};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::MediaStream;

pub struct RecorderState {
    pub source_type: SourceType,
    pub pip_position: PipPosition,
    pub pip_size: f64,
    pub chart_enabled: bool,
    pub chart_type: String,
    pub chart_position: PipPosition,
    pub chart_size: f64,
    pub recording: bool,
    pub start_time: f64,
    pub frame_count: u32,

    camera_stream: Option<MediaStream>,
    screen_stream: Option<MediaStream>,
    renderer: Option<CanvasRenderer>,
    recorder: Option<Recorder>,
    animation_frame_id: Option<i32>,
}

impl RecorderState {
    pub fn new() -> Self {
        Self {
            source_type: SourceType::Combined,
            pip_position: PipPosition::TopLeft,
            pip_size: 0.25,
            chart_enabled: true,
            chart_type: "metrics-chart".to_string(),
            chart_position: PipPosition::BottomRight,
            chart_size: 0.20,
            recording: false,
            start_time: 0.0,
            frame_count: 0,
            camera_stream: None,
            screen_stream: None,
            renderer: None,
            recorder: None,
            animation_frame_id: None,
        }
    }

    pub async fn start_recording(&mut self) -> Result<(), JsValue> {
        // Implementation in next task
        todo!()
    }

    pub async fn stop_recording(&mut self) -> Result<(), JsValue> {
        // Implementation in next task
        todo!()
    }
}
