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
        use crate::recorder::media_streams::{get_camera_stream, get_screen_stream};
        use crate::recorder::utils::log;

        log("[Recorder] Starting recording");

        // Get media streams based on source type
        match self.source_type {
            SourceType::Camera => {
                self.camera_stream = Some(get_camera_stream().await?);
            }
            SourceType::Screen => {
                self.screen_stream = Some(get_screen_stream().await?);
            }
            SourceType::Combined => {
                self.camera_stream = Some(get_camera_stream().await?);
                self.screen_stream = Some(get_screen_stream().await?);
            }
        }

        // Initialize canvas renderer
        let document = web_sys::window()
            .ok_or("No window")?
            .document()
            .ok_or("No document")?;

        let canvas: web_sys::HtmlCanvasElement = document
            .get_element_by_id("recordingCanvas")
            .ok_or("recordingCanvas not found")?
            .dyn_into()?;

        self.renderer = Some(CanvasRenderer::new(canvas.clone())?);

        // Start MediaRecorder with canvas stream
        let canvas_stream = canvas
            .capture_stream()
            .map_err(|_| "Failed to capture canvas stream")?;

        self.recorder = Some(Recorder::new(&canvas_stream)?);
        if let Some(recorder) = &self.recorder {
            recorder.start()?;
        }

        self.recording = true;
        self.start_time = js_sys::Date::now();
        self.frame_count = 0;

        // Start render loop
        self.start_render_loop()?;

        log("[Recorder] Recording started");
        Ok(())
    }

    fn start_render_loop(&mut self) -> Result<(), JsValue> {
        // This will be implemented with render_frame callback
        // For now, placeholder
        Ok(())
    }

    pub async fn stop_recording(&mut self) -> Result<(), JsValue> {
        use crate::recorder::storage::IndexedDbWrapper;
        use crate::recorder::types::{RecordingMetadata, CameraFacing};
        use crate::recorder::utils::log;

        log("[Recorder] Stopping recording");

        // Stop render loop
        if let Some(id) = self.animation_frame_id {
            web_sys::window()
                .ok_or("No window")?
                .cancel_animation_frame(id)?;
            self.animation_frame_id = None;
        }

        // Stop MediaRecorder and get blob
        let blob = if let Some(recorder) = &self.recorder {
            recorder.stop().await?
        } else {
            return Err("No recorder".into());
        };

        // Get motion data from global SENSOR_MANAGER
        let motion_data = if let Ok(manager_guard) = crate::SENSOR_MANAGER.lock() {
            if let Some(ref mgr) = *manager_guard {
                mgr.get_motion_data().clone()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Calculate duration
        let duration = (js_sys::Date::now() - self.start_time) / 1000.0;

        // Create recording metadata
        let end_time = js_sys::Date::now();
        let metadata = RecordingMetadata {
            frame_count: self.frame_count,
            duration,
            mime_type: "video/webm".to_string(),
            start_time_utc: crate::recorder::utils::format_timestamp(self.start_time),
            end_time_utc: crate::recorder::utils::format_timestamp(end_time),
            source_type: self.source_type,
            camera_facing: CameraFacing::Unknown,
            chart_included: self.chart_enabled,
            chart_type: if self.chart_enabled {
                Some(self.chart_type.clone())
            } else {
                None
            },
            test_metadata: None,
        };

        // Generate unique ID
        let id = format!("rec_{}", end_time as u64);

        // Save to IndexedDB
        let db = IndexedDbWrapper::open().await?;
        db.save_recording(&id, &blob, &metadata, &motion_data).await?;

        // Cleanup
        self.camera_stream = None;
        self.screen_stream = None;
        self.recorder = None;
        self.renderer = None;
        self.recording = false;

        log("[Recorder] Recording saved");
        Ok(())
    }
}
