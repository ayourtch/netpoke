use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, HtmlVideoElement, MediaStream};

use crate::canvas_renderer::CanvasRenderer;
use crate::recorder::Recorder;
use crate::storage::IndexedDbWrapper;
use crate::types::{RecordingMetadata, SourceType};
use crate::ui::UiController;

pub struct AppState {
    ui: UiController,
    db: IndexedDbWrapper,
    canvas_renderer: CanvasRenderer,
    screen_video: HtmlVideoElement,
    camera_video: HtmlVideoElement,

    // Recording state
    recorder: Option<Recorder>,
    screen_stream: Option<MediaStream>,
    camera_stream: Option<MediaStream>,
    current_source_type: Option<SourceType>,
    frame_count: u32,
    render_interval_handle: Option<i32>,
    metrics_interval_handle: Option<i32>,
    sensor_manager: Option<crate::sensors::SensorManager>,
    start_time: f64,
}

impl AppState {
    pub async fn new() -> Result<Rc<RefCell<Self>>, JsValue> {
        let window = web_sys::window().ok_or("No window")?;
        let document = window.document().ok_or("No document")?;

        let ui = UiController::new()?;
        let db = IndexedDbWrapper::open().await?;

        let canvas: HtmlCanvasElement = document
            .get_element_by_id("preview")
            .ok_or("Canvas not found")?
            .dyn_into()?;

        let canvas_renderer = CanvasRenderer::new(canvas)?;

        let screen_video: HtmlVideoElement = document
            .get_element_by_id("screenVideo")
            .ok_or("Screen video not found")?
            .dyn_into()?;

        let camera_video: HtmlVideoElement = document
            .get_element_by_id("cameraVideo")
            .ok_or("Camera video not found")?
            .dyn_into()?;

        let state = Rc::new(RefCell::new(Self {
            ui,
            db,
            canvas_renderer,
            screen_video,
            camera_video,
            recorder: None,
            screen_stream: None,
            camera_stream: None,
            current_source_type: None,
            frame_count: 0,
            render_interval_handle: None,
            metrics_interval_handle: None,
            sensor_manager: None,
            start_time: 0.0,
        }));

        // Load recordings list
        state.borrow().refresh_recordings_list().await?;

        Ok(state)
    }

    pub fn get_ui(&self) -> &UiController {
        &self.ui
    }

    async fn refresh_recordings_list(&self) -> Result<(), JsValue> {
        let recordings = self.db.get_all_recordings().await?;
        self.ui.render_recordings_list(&recordings);
        Ok(())
    }

    pub async fn start_tracking(&mut self, source_type: SourceType) -> Result<(), JsValue> {
        crate::utils::log(&format!("Starting tracking: {:?}", source_type));

        // Reset state
        self.frame_count = 0;
        self.current_source_type = Some(source_type);

        // Initialize sensor manager
        self.start_time = js_sys::Date::now();

        // Determine camera facing mode based on source type
        let camera_facing = match source_type {
            SourceType::Camera => crate::types::CameraFacing::User,  // Front camera
            SourceType::Combined => crate::types::CameraFacing::User,  // Front camera for PiP
            SourceType::Screen => crate::types::CameraFacing::Unknown,  // No camera
        };

        let mut new_sensor_manager = crate::sensors::SensorManager::new(self.start_time, camera_facing);

        // Get checkbox state and set overlay enabled
        let window = web_sys::window().ok_or("No window")?;
        let document = window.document().ok_or("No document")?;
        if let Some(checkbox) = document.get_element_by_id("showSensorsOverlay") {
            if let Ok(input) = checkbox.dyn_into::<web_sys::HtmlInputElement>() {
                new_sensor_manager.set_overlay_enabled(input.checked());
            }
        }

        self.sensor_manager = Some(new_sensor_manager.clone());

        // Update global sensor manager
        if let Ok(mut global_mgr) = crate::SENSOR_MANAGER.lock() {
            *global_mgr = Some(new_sensor_manager);
        }

        // Get media streams based on source type
        // For Combined mode, request both in parallel to stay in user gesture context (Safari requirement)
        if source_type == SourceType::Combined {
            crate::utils::log("Requesting camera and screen in parallel for Combined mode");

            // Start both requests simultaneously
            let camera_future = crate::media_streams::get_camera_stream();
            let screen_future = crate::media_streams::get_screen_stream();

            // Await them together
            match futures::future::try_join(camera_future, screen_future).await {
                Ok((camera_stream, screen_stream)) => {
                    self.camera_video.set_src_object(Some(&camera_stream));
                    let _ = self.camera_video.play();
                    self.camera_stream = Some(camera_stream);

                    self.screen_video.set_src_object(Some(&screen_stream));
                    self.screen_video.set_muted(true);
                    let _ = self.screen_video.play();
                    self.screen_stream = Some(screen_stream);
                }
                Err(e) => {
                    let error_msg = format!("Permission denied or error: {:?}", e);
                    crate::utils::log(&error_msg);
                    let _ = self.ui.set_status(&error_msg);
                    self.current_source_type = None;
                    return Err(e);
                }
            }
        } else if source_type == SourceType::Camera {
            match crate::media_streams::get_camera_stream().await {
                Ok(camera_stream) => {
                    self.camera_video.set_src_object(Some(&camera_stream));
                    let _ = self.camera_video.play();
                    self.camera_stream = Some(camera_stream);
                }
                Err(e) => {
                    let error_msg = format!("Camera access denied or error: {:?}", e);
                    crate::utils::log(&error_msg);
                    let _ = self.ui.set_status(&error_msg);
                    self.current_source_type = None;
                    return Err(e);
                }
            }
        } else if source_type == SourceType::Screen {
            match crate::media_streams::get_screen_stream().await {
                Ok(screen_stream) => {
                    self.screen_video.set_src_object(Some(&screen_stream));
                    self.screen_video.set_muted(true);
                    let _ = self.screen_video.play();
                    self.screen_stream = Some(screen_stream);
                }
                Err(e) => {
                    let error_msg = format!("Screen share denied or error: {:?}", e);
                    crate::utils::log(&error_msg);
                    let _ = self.ui.set_status(&error_msg);
                    self.current_source_type = None;
                    return Err(e);
                }
            }
        }

        // Show stop button and recording UI now that we have streams
        // This ensures the user can always stop, even if setup fails later
        if let Err(e) = self.ui.show_recording_state(source_type) {
            crate::utils::log(&format!("Failed to update UI: {:?}", e));
        }

        // Start sensor tracking
        let window = web_sys::window().ok_or("No window")?;
        let start_sensors = js_sys::Reflect::get(&window, &"startSensorTracking".into())?;
        if start_sensors.is_function() {
            let start_fn: js_sys::Function = start_sensors.dyn_into()?;
            let promise: js_sys::Promise = start_fn.call0(&window)?.dyn_into()?;
            wasm_bindgen_futures::JsFuture::from(promise).await?;
        }

        // Wait for video to be ready
        let window = web_sys::window().ok_or("No window")?;
        wasm_bindgen_futures::JsFuture::from(
            js_sys::Promise::new(&mut |resolve, _reject| {
                window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 1000).unwrap();
            })
        ).await.map_err(|e| {
            crate::utils::log(&format!("Wait timeout error: {:?}", e));
            e
        })?;

        // Start render loop
        crate::utils::log("Starting render loop...");
        if let Err(e) = self.start_render_loop() {
            crate::utils::log(&format!("Render loop error: {:?}", e));
            let _ = self.ui.set_status("Error starting render loop");
            return Err(e);
        }

        // Wait for first frame to render
        wasm_bindgen_futures::JsFuture::from(
            js_sys::Promise::new(&mut |resolve, _reject| {
                window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 300).unwrap();
            })
        ).await.map_err(|e| {
            crate::utils::log(&format!("Frame wait error: {:?}", e));
            e
        })?;

        // Get canvas stream
        crate::utils::log("Getting canvas stream...");
        let canvas_stream = self.canvas_renderer.get_canvas_stream(30).map_err(|e| {
            crate::utils::log(&format!("Canvas stream error: {:?}", e));
            let _ = self.ui.set_status("Error capturing canvas stream");
            e
        })?;

        // Add audio track from source
        // For camera and combined modes, use camera audio; for screen mode, use screen audio
        let source_stream = match source_type {
            SourceType::Camera | SourceType::Combined => self.camera_stream.as_ref(),
            SourceType::Screen => self.screen_stream.as_ref(),
        };

        if let Some(stream) = source_stream {
            let audio_tracks = stream.get_audio_tracks();
            crate::utils::log(&format!("Found {} audio tracks", audio_tracks.length()));
            if audio_tracks.length() > 0 {
                let audio_track = web_sys::MediaStreamTrack::from(audio_tracks.get(0));
                canvas_stream.add_track(&audio_track);
                crate::utils::log("Added audio track to recording");
            } else {
                crate::utils::log("No audio tracks found in source stream");
            }
        } else {
            crate::utils::log("No source stream for audio");
        }

        // Create and start recorder
        crate::utils::log("Creating recorder...");
        let recorder = Recorder::new(&canvas_stream).map_err(|e| {
            crate::utils::log(&format!("Recorder creation error: {:?}", e));
            let _ = self.ui.set_status("Error creating recorder");
            e
        })?;

        crate::utils::log("Starting recorder...");
        recorder.start().map_err(|e| {
            crate::utils::log(&format!("Recorder start error: {:?}", e));
            let _ = self.ui.set_status("Error starting recorder");
            e
        })?;
        self.recorder = Some(recorder);

        // Start metrics update interval
        crate::utils::log("Starting metrics loop...");
        if let Err(e) = self.start_metrics_loop() {
            crate::utils::log(&format!("Metrics loop error: {:?}", e));
            // Don't fail on metrics error, recording can still work
        }

        crate::utils::log("Recording started successfully");
        let _ = self.ui.set_status("Recording...");

        Ok(())
    }

    fn start_render_loop(&mut self) -> Result<(), JsValue> {
        let window = web_sys::window().ok_or("No window")?;

        let screen_video = self.screen_video.clone();
        let camera_video = self.camera_video.clone();
        let source_type = self.current_source_type.ok_or("No source type")?;

        let canvas: HtmlCanvasElement = window
            .document()
            .ok_or("No document")?
            .get_element_by_id("preview")
            .ok_or("Canvas not found")?
            .dyn_into()?;

        let renderer = CanvasRenderer::new(canvas)?;

        let pip_position_el = self.ui.pip_position_el.clone();
        let pip_size_el = self.ui.pip_size_el.clone();

        let closure = Closure::wrap(Box::new(move || {
            let pip_position = pip_position_el.value();
            let pip_size: f64 = pip_size_el.value().parse().unwrap_or(25.0);

            let screen_ref = if matches!(source_type, SourceType::Screen | SourceType::Combined) {
                Some(&screen_video)
            } else {
                None
            };

            let camera_ref = if matches!(source_type, SourceType::Camera | SourceType::Combined) {
                Some(&camera_video)
            } else {
                None
            };

            let _ = renderer.render_frame(
                source_type,
                screen_ref,
                camera_ref,
                &pip_position,
                pip_size,
            );

            // Render sensor overlay if enabled
            if let Ok(manager_guard) = crate::SENSOR_MANAGER.lock() {
                if let Some(ref mgr) = *manager_guard {
                    if mgr.is_overlay_enabled() {
                        let timestamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap();
                        let camera_direction = mgr.get_current_camera_direction();
                        let _ = renderer.render_sensor_overlay(
                            &timestamp,
                            mgr.get_current_gps(),
                            mgr.get_current_magnetometer(),
                            mgr.get_current_orientation(),
                            mgr.get_current_acceleration(),
                            &camera_direction,
                        );

                        // Render compass indicator
                        let _ = renderer.render_compass(camera_direction);
                    }
                }
            }
        }) as Box<dyn FnMut()>);

        let handle = window.set_interval_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            33,
        )?;

        closure.forget();
        self.render_interval_handle = Some(handle);

        Ok(())
    }

    fn start_metrics_loop(&mut self) -> Result<(), JsValue> {
        let window = web_sys::window().ok_or("No window")?;

        let recorder_id = self.recorder.as_ref().ok_or("No recorder")?.recorder_id.clone();
        let start_time = self.recorder.as_ref().unwrap().start_time;

        let frames_el = self.ui.frames_el.clone();
        let duration_el = self.ui.duration_el.clone();
        let video_size_el = self.ui.video_size_el.clone();

        let mut frame_count = 0u32;

        let closure = Closure::wrap(Box::new(move || {
            use wasm_bindgen::prelude::*;

            #[wasm_bindgen(module = "/js/media_recorder.js")]
            extern "C" {
                #[wasm_bindgen(catch)]
                fn getChunksSize(id: &str) -> Result<f64, JsValue>;
            }

            frame_count += 1;

            let elapsed = (js_sys::Date::now() - start_time) / 1000.0;
            let chunks_size = getChunksSize(&recorder_id).unwrap_or(0.0);
            let size_mb = chunks_size / (1024.0 * 1024.0);

            frames_el.set_text_content(Some(&frame_count.to_string()));
            duration_el.set_text_content(Some(&format!("{:.1}s", elapsed)));
            video_size_el.set_text_content(Some(&format!("{:.2} MB", size_mb)));
        }) as Box<dyn FnMut()>);

        let handle = window.set_interval_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            100,
        )?;

        closure.forget();
        self.metrics_interval_handle = Some(handle);

        Ok(())
    }

    pub async fn stop_tracking(&mut self) -> Result<(), JsValue> {
        crate::utils::log("Stopping tracking");

        let window = web_sys::window().ok_or("No window")?;

        // Stop intervals
        if let Some(handle) = self.render_interval_handle.take() {
            window.clear_interval_with_handle(handle);
        }
        if let Some(handle) = self.metrics_interval_handle.take() {
            window.clear_interval_with_handle(handle);
        }

        // Stop recorder
        let recorder = self.recorder.take().ok_or("No recorder")?;
        let blob = recorder.stop().await?;

        // Stop streams
        if let Some(stream) = self.camera_stream.take() {
            crate::media_streams::stop_stream(&stream);
        }
        if let Some(stream) = self.screen_stream.take() {
            crate::media_streams::stop_stream(&stream);
        }

        self.ui.set_status("Saving recording...")?;

        // Wait a bit
        wasm_bindgen_futures::JsFuture::from(
            js_sys::Promise::new(&mut |resolve, _reject| {
                window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 500).unwrap();
            })
        ).await?;

        // Get motion data and camera facing from global sensor manager (receives callback updates)
        let (motion_data, camera_facing) = if let Ok(manager_guard) = crate::SENSOR_MANAGER.lock() {
            if let Some(ref mgr) = *manager_guard {
                (mgr.get_motion_data().clone(), mgr.get_camera_facing())
            } else {
                (Vec::new(), crate::types::CameraFacing::Unknown)
            }
        } else {
            (Vec::new(), crate::types::CameraFacing::Unknown)
        };

        // Create metadata
        let duration = (js_sys::Date::now() - recorder.start_time) / 1000.0;
        let metadata = RecordingMetadata {
            frame_count: self.frame_count,
            duration,
            mime_type: recorder.mime_type().to_string(),
            start_time_utc: recorder.start_time_utc.clone(),
            end_time_utc: crate::utils::current_timestamp_utc(),
            source_type: self.current_source_type.ok_or("No source type")?,
            camera_facing,
        };

        // Save to IndexedDB
        let recording_id = recorder.start_time.to_string();
        self.db.save_recording(&recording_id, &blob, &metadata, &motion_data).await?;

        // Stop sensors
        let stop_sensors = js_sys::Reflect::get(&window, &"stopSensorTracking".into())?;
        if stop_sensors.is_function() {
            let stop_fn: js_sys::Function = stop_sensors.dyn_into()?;
            stop_fn.call0(&window)?;
        }

        // Clear global sensor manager
        if let Ok(mut manager_guard) = crate::SENSOR_MANAGER.lock() {
            if let Some(ref mut mgr) = *manager_guard {
                mgr.clear();
            }
        }

        // Update UI
        self.ui.show_ready_state()?;
        self.ui.set_status("Recording saved!")?;
        self.refresh_recordings_list().await?;

        self.current_source_type = None;

        Ok(())
    }
}
