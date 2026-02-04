use crate::recorder::{
    canvas_renderer::CanvasRenderer,
    media_recorder::Recorder,
    sensors::SensorManager,
    types::{SourceType, PipPosition},
};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MediaStream, HtmlVideoElement};

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
    camera_video: Option<HtmlVideoElement>,
    screen_video: Option<HtmlVideoElement>,
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
            camera_video: None,
            screen_video: None,
            renderer: None,
            recorder: None,
            animation_frame_id: None,
        }
    }

    pub async fn start_recording(&mut self) -> Result<(), JsValue> {
        use crate::recorder::media_streams::{get_camera_stream, get_screen_stream, add_screen_stop_listener};
        use crate::recorder::types::CameraFacing;
        use crate::recorder::utils::log;

        log("[Recorder] Starting recording");

        let document = web_sys::window()
            .ok_or("No window")?
            .document()
            .ok_or("No document")?;

        // Initialize start time (Issue 007)
        let start_time = js_sys::Date::now();
        self.start_time = start_time;

        // Get media streams and create video elements based on source type
        match self.source_type {
            SourceType::Camera => {
                let stream = get_camera_stream().await?;
                let video: HtmlVideoElement = document.create_element("video")?.dyn_into()?;
                video.set_autoplay(true);
                video.set_muted(true);
                video.set_src_object(Some(&stream));
                self.camera_stream = Some(stream);
                self.camera_video = Some(video);
            }
            SourceType::Screen => {
                let stream = get_screen_stream().await?;
                let video: HtmlVideoElement = document.create_element("video")?.dyn_into()?;
                video.set_autoplay(true);
                video.set_muted(true);
                video.set_src_object(Some(&stream));
                self.screen_stream = Some(stream.clone());
                self.screen_video = Some(video);

                // Issue 009: Add screen stop listener
                add_screen_stop_listener(&stream, Box::new(|| {
                    log("[Recorder] Screen sharing stopped by user");
                    // Note: We can't directly call stop_recording from here due to ownership
                    // The JavaScript callback onScreenShareStopped will need to trigger it
                    if let Some(window) = web_sys::window() {
                        if let Ok(callback) = js_sys::Reflect::get(&window, &"onScreenShareStopped".into()) {
                            if callback.is_function() {
                                let func: js_sys::Function = callback.dyn_into().unwrap();
                                let _ = func.call0(&window);
                            }
                        }
                    }
                }))?;
            }
            SourceType::Combined => {
                let camera_stream = get_camera_stream().await?;
                let screen_stream = get_screen_stream().await?;

                let camera_video: HtmlVideoElement = document.create_element("video")?.dyn_into()?;
                camera_video.set_autoplay(true);
                camera_video.set_muted(true);
                camera_video.set_src_object(Some(&camera_stream));

                let screen_video: HtmlVideoElement = document.create_element("video")?.dyn_into()?;
                screen_video.set_autoplay(true);
                screen_video.set_muted(true);
                screen_video.set_src_object(Some(&screen_stream));

                self.camera_stream = Some(camera_stream);
                self.screen_stream = Some(screen_stream.clone());
                self.camera_video = Some(camera_video);
                self.screen_video = Some(screen_video);

                // Issue 009: Add screen stop listener for combined mode
                add_screen_stop_listener(&screen_stream, Box::new(|| {
                    log("[Recorder] Screen sharing stopped by user");
                    if let Some(window) = web_sys::window() {
                        if let Ok(callback) = js_sys::Reflect::get(&window, &"onScreenShareStopped".into()) {
                            if callback.is_function() {
                                let func: js_sys::Function = callback.dyn_into().unwrap();
                                let _ = func.call0(&window);
                            }
                        }
                    }
                }))?;
            }
        }

        // Initialize sensor manager (Issue 007, 010)
        let camera_facing = match self.source_type {
            SourceType::Camera | SourceType::Combined => CameraFacing::User,
            SourceType::Screen => CameraFacing::Unknown,
        };

        let mut sensor_manager = SensorManager::new(start_time, camera_facing);

        // Check if sensor overlay checkbox is checked
        if let Some(checkbox) = document.get_element_by_id("show-sensors-overlay") {
            if let Ok(input) = checkbox.dyn_into::<web_sys::HtmlInputElement>() {
                sensor_manager.set_overlay_enabled(input.checked());
            }
        }

        // Update global sensor manager
        if let Ok(mut global_mgr) = crate::SENSOR_MANAGER.lock() {
            *global_mgr = Some(sensor_manager);
        }

        // Initialize canvas renderer
        let canvas: web_sys::HtmlCanvasElement = document
            .get_element_by_id("recordingCanvas")
            .ok_or("recordingCanvas not found")?
            .dyn_into()?;

        self.renderer = Some(CanvasRenderer::new(canvas.clone())?);

        // Start MediaRecorder with canvas stream
        let canvas_stream = canvas
            .capture_stream()
            .map_err(|_| "Failed to capture canvas stream")?;

        // Add audio track from source stream (Issue 004)
        let source_stream = match self.source_type {
            SourceType::Camera | SourceType::Combined => self.camera_stream.as_ref(),
            SourceType::Screen => self.screen_stream.as_ref(),
        };

        if let Some(stream) = source_stream {
            let audio_tracks = stream.get_audio_tracks();
            log(&format!("[Recorder] Found {} audio tracks", audio_tracks.length()));
            if audio_tracks.length() > 0 {
                let audio_track = web_sys::MediaStreamTrack::from(audio_tracks.get(0));
                canvas_stream.add_track(&audio_track);
                log("[Recorder] Added audio track to recording");
            }
        }

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
        use crate::recorder::utils::log;

        log("[Recorder] Starting render loop");

        // We'll use a simple approach: export a render function and call it from JavaScript
        // For now, return Ok - the actual rendering will be driven by a JavaScript setInterval
        // This is simpler than managing Rust closures with requestAnimationFrame

        Ok(())
    }

    pub fn render_frame(&mut self) -> Result<(), JsValue> {
        if !self.recording {
            return Ok(());
        }

        if let Some(renderer) = &self.renderer {
            // Convert PipPosition to string
            let pip_pos_str = match self.pip_position {
                PipPosition::TopLeft => "top-left",
                PipPosition::TopRight => "top-right",
                PipPosition::BottomLeft => "bottom-left",
                PipPosition::BottomRight => "bottom-right",
            };

            // Render main video frame
            renderer.render_frame(
                self.source_type,
                self.screen_video.as_ref(),
                self.camera_video.as_ref(),
                pip_pos_str,
                self.pip_size * 100.0,
            )?;

            // Render chart overlay if enabled
            if self.chart_enabled {
                // Get canvas element to determine dimensions
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        if let Some(canvas_element) = document.get_element_by_id("recordingCanvas") {
                            if let Ok(canvas) = canvas_element.dyn_into::<web_sys::HtmlCanvasElement>() {
                                let canvas_width = canvas.width() as f64;
                                let canvas_height = canvas.height() as f64;

                                // Calculate chart dimensions (Issue 016)
                                // chart_size is a percentage (0.0 - 1.0), use it as percentage of canvas width
                                let chart_width = canvas_width * self.chart_size;
                                // Maintain 4:3 aspect ratio (common for charts)
                                let chart_height = chart_width * 0.75;
                                let margin = 20.0;

                                // Calculate position based on chart position
                                let (chart_x, chart_y) = match self.chart_position {
                                    PipPosition::TopLeft => (margin, margin),
                                    PipPosition::TopRight => (canvas_width - chart_width - margin, margin),
                                    PipPosition::BottomLeft => (margin, canvas_height - chart_height - margin),
                                    PipPosition::BottomRight => (canvas_width - chart_width - margin, canvas_height - chart_height - margin),
                                };

                                let _ = renderer.render_chart_overlay(
                                    &self.chart_type,
                                    chart_x,
                                    chart_y,
                                    chart_width,
                                    chart_height,
                                );
                            }
                        }
                    }
                }
            }

            // Render sensor overlay if we have sensor data
            if let Ok(manager_guard) = crate::SENSOR_MANAGER.lock() {
                if let Some(ref mgr) = *manager_guard {
                    let motion_data = mgr.get_motion_data();
                    if let Some(latest) = motion_data.last() {
                        let _ = renderer.render_sensor_overlay(
                            &latest.timestamp_utc,
                            &latest.gps,
                            &latest.magnetometer,
                            &latest.orientation,
                            &Some(latest.acceleration.clone()),
                            &latest.camera_direction,
                        );

                        // Render compass if we have camera direction
                        let _ = renderer.render_compass(latest.camera_direction);
                    }
                }
            }

            self.frame_count += 1;

            // Update metrics display (Issue 013)
            let elapsed = (js_sys::Date::now() - self.start_time) / 1000.0;
            // Estimate size based on frame count (rough estimate)
            let estimated_size = (self.frame_count as u64) * 50000; // ~50KB per frame estimate
            crate::recorder::ui::update_recording_metrics(elapsed, self.frame_count, estimated_size);
        }

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

        // Get motion data and camera facing from global SENSOR_MANAGER (Issue 010)
        let (motion_data, camera_facing) = if let Ok(manager_guard) = crate::SENSOR_MANAGER.lock() {
            if let Some(ref mgr) = *manager_guard {
                (mgr.get_motion_data().clone(), mgr.get_camera_facing())
            } else {
                (Vec::new(), CameraFacing::Unknown)
            }
        } else {
            (Vec::new(), CameraFacing::Unknown)
        };

        // Calculate duration
        let duration = (js_sys::Date::now() - self.start_time) / 1000.0;

        // Create recording metadata
        let end_time = js_sys::Date::now();
        
        // Issue 005: Get test metadata from the main measurement state
        let test_metadata_js = crate::get_test_metadata();
        let test_metadata = if !test_metadata_js.is_null() && !test_metadata_js.is_undefined() {
            serde_wasm_bindgen::from_value(test_metadata_js).ok()
        } else {
            None
        };
        
        let metadata = RecordingMetadata {
            frame_count: self.frame_count,
            duration,
            mime_type: "video/webm".to_string(),
            start_time_utc: crate::recorder::utils::format_timestamp(self.start_time),
            end_time_utc: crate::recorder::utils::format_timestamp(end_time),
            source_type: self.source_type,
            camera_facing,  // Now uses actual camera facing from sensor manager
            chart_included: self.chart_enabled,
            chart_type: if self.chart_enabled {
                Some(self.chart_type.clone())
            } else {
                None
            },
            test_metadata,
        };

        // Generate unique ID
        let id = format!("rec_{}", end_time as u64);

        // Save to IndexedDB
        let db = IndexedDbWrapper::open().await?;
        db.save_recording(&id, &blob, &metadata, &motion_data).await?;

        // Issue 011: Refresh recordings list in UI
        if let Some(window) = web_sys::window() {
            if let Ok(refresh_fn) = js_sys::Reflect::get(&window, &"refreshRecordingsList".into()) {
                if refresh_fn.is_function() {
                    let func: js_sys::Function = refresh_fn.dyn_into()?;
                    let _ = func.call0(&window);
                }
            }
        }

        // Cleanup
        self.camera_stream = None;
        self.screen_stream = None;
        self.camera_video = None;
        self.screen_video = None;
        self.recorder = None;
        self.renderer = None;
        self.recording = false;

        // Clear global sensor manager (Issue 007)
        if let Ok(mut manager_guard) = crate::SENSOR_MANAGER.lock() {
            if let Some(ref mut mgr) = *manager_guard {
                mgr.clear();
            }
        }

        log("[Recorder] Recording saved");
        Ok(())
    }
}
