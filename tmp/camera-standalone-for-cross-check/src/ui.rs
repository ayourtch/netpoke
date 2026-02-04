use wasm_bindgen::prelude::*;
use web_sys::{Document, HtmlButtonElement, HtmlElement, HtmlInputElement, HtmlSelectElement};

// Request sensor permissions from JavaScript (must be called from user gesture)
async fn request_sensor_permissions() -> Result<(), JsValue> {
    crate::utils::log("[RUST] request_sensor_permissions called");

    let window = web_sys::window().ok_or("No window")?;
    crate::utils::log("[RUST] Got window");

    let request_fn = js_sys::Reflect::get(&window, &"requestSensorPermissions".into())?;
    crate::utils::log("[RUST] Got requestSensorPermissions function");

    if !request_fn.is_function() {
        crate::utils::log("[RUST] requestSensorPermissions is NOT a function!");
        return Err(JsValue::from_str("requestSensorPermissions not found"));
    }
    crate::utils::log("[RUST] requestSensorPermissions IS a function");

    let request_fn: js_sys::Function = request_fn.dyn_into()?;
    crate::utils::log("[RUST] About to call requestSensorPermissions");
    let promise: js_sys::Promise = request_fn.call0(&window)?.dyn_into()?;
    crate::utils::log("[RUST] Got promise, awaiting...");
    let result = wasm_bindgen_futures::JsFuture::from(promise).await?;
    crate::utils::log("[RUST] Promise resolved");

    // Check if permissions were granted (result should be true)
    if result.is_truthy() {
        crate::utils::log("[RUST] Permissions granted (truthy result)");
        Ok(())
    } else {
        crate::utils::log("[RUST] Permissions denied (falsy result)");
        Err(JsValue::from_str("Sensor permissions denied"))
    }
}

#[derive(Clone)]
pub struct UiController {
    pub status_el: HtmlElement,
    pub start_camera_btn: HtmlButtonElement,
    pub start_screen_btn: HtmlButtonElement,
    pub start_combined_btn: HtmlButtonElement,
    pub stop_btn: HtmlButtonElement,
    pub metrics_div: HtmlElement,
    pub pip_controls_div: HtmlElement,
    pub frames_el: HtmlElement,
    pub duration_el: HtmlElement,
    pub video_size_el: HtmlElement,
    pub source_type_el: HtmlElement,
    pub recordings_list_el: HtmlElement,
    pub pip_position_el: HtmlSelectElement,
    pub pip_size_el: HtmlInputElement,
    pub pip_size_label_el: HtmlElement,
}

impl UiController {
    pub fn new() -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or("No window")?;
        let document = window.document().ok_or("No document")?;

        Ok(Self {
            status_el: get_element_by_id(&document, "status")?,
            start_camera_btn: get_element_by_id(&document, "startCameraBtn")?,
            start_screen_btn: get_element_by_id(&document, "startScreenBtn")?,
            start_combined_btn: get_element_by_id(&document, "startCombinedBtn")?,
            stop_btn: get_element_by_id(&document, "stopBtn")?,
            metrics_div: get_element_by_id(&document, "metrics")?,
            pip_controls_div: get_element_by_id(&document, "pipControls")?,
            frames_el: get_element_by_id(&document, "frames")?,
            duration_el: get_element_by_id(&document, "duration")?,
            video_size_el: get_element_by_id(&document, "videoSize")?,
            source_type_el: get_element_by_id(&document, "sourceType")?,
            recordings_list_el: get_element_by_id(&document, "recordingsList")?,
            pip_position_el: get_element_by_id(&document, "pipPosition")?,
            pip_size_el: get_element_by_id(&document, "pipSize")?,
            pip_size_label_el: get_element_by_id(&document, "pipSizeLabel")?,
        })
    }

    pub fn set_status(&self, text: &str) -> Result<(), JsValue> {
        self.status_el.set_text_content(Some(text));
        Ok(())
    }

    pub fn show_ready_state(&self) -> Result<(), JsValue> {
        // Remove inline display styles to let CSS control visibility
        // (CSS hides unsupported buttons via .no-screen-sharing class)
        self.start_camera_btn.style().remove_property("display")?;
        self.start_screen_btn.style().remove_property("display")?;
        self.start_combined_btn.style().remove_property("display")?;
        self.stop_btn.style().set_property("display", "none")?;
        self.metrics_div.style().set_property("display", "none")?;
        self.pip_controls_div.style().set_property("display", "none")?;
        self.status_el.set_text_content(Some("Ready to start"));
        Ok(())
    }

    pub fn show_recording_state(&self, source_type: crate::types::SourceType) -> Result<(), JsValue> {
        self.start_camera_btn.style().set_property("display", "none")?;
        self.start_screen_btn.style().set_property("display", "none")?;
        self.start_combined_btn.style().set_property("display", "none")?;
        self.stop_btn.style().set_property("display", "block")?;
        self.metrics_div.style().set_property("display", "block")?;

        let pip_display = if matches!(source_type, crate::types::SourceType::Combined) {
            "block"
        } else {
            "none"
        };
        self.pip_controls_div.style().set_property("display", pip_display)?;

        self.source_type_el.set_text_content(Some(source_type.display_name()));
        self.status_el.set_text_content(Some("Recording..."));
        Ok(())
    }

    pub fn update_metrics(&self, frames: u32, duration: f64, video_size_mb: f64) {
        self.frames_el.set_text_content(Some(&frames.to_string()));
        self.duration_el.set_text_content(Some(&format!("{:.1}s", duration)));
        self.video_size_el.set_text_content(Some(&format!("{:.2} MB", video_size_mb)));
    }

    pub fn render_recordings_list(&self, recordings: &[crate::types::Recording]) {
        if recordings.is_empty() {
            self.recordings_list_el.set_inner_html(
                "<p style=\"color:#888;\">No recordings yet</p>"
            );
            return;
        }

        let html = recordings.iter().map(|rec| {
            let date = js_sys::Date::new(&JsValue::from(rec.timestamp));
            let size_mb = rec.blob_size as f64 / (1024.0 * 1024.0);
            let source_class = format!("source-{}", rec.metadata.source_type.as_str());
            let source_label = rec.metadata.source_type.display_name();

            format!(
                r#"
                <div class="recording-item">
                    <div class="data">ID: {} <span class="source-label {}">{}</span></div>
                    <div class="data">Date: {}</div>
                    <div class="data">Duration: {:.1}s</div>
                    <div class="data">Frames: {}</div>
                    <div class="data">Size: {:.2} MB</div>
                    <button onclick="downloadVideo('{}')">Download Video</button>
                    <button onclick="downloadMotionData('{}')">Download Motion Data</button>
                    <button class="danger" onclick="deleteRecordingById('{}')">Delete</button>
                </div>
                "#,
                rec.id, source_class, source_label,
                date.to_locale_string("en-US", &JsValue::UNDEFINED).as_string().unwrap(),
                rec.metadata.duration, rec.metadata.frame_count, size_mb,
                rec.id, rec.id, rec.id
            )
        }).collect::<Vec<_>>().join("");

        self.recordings_list_el.set_inner_html(&html);
    }

    pub fn register_event_listeners(
        &self,
        app_state: std::rc::Rc<std::cell::RefCell<crate::app::AppState>>,
    ) -> Result<(), JsValue> {
        use crate::types::SourceType;

        // Start camera button
        {
            let app = app_state.clone();
            let closure = Closure::wrap(Box::new(move || {
                let app = app.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    // Request sensor permissions first (must be in user gesture context)
                    if let Err(e) = request_sensor_permissions().await {
                        crate::utils::log(&format!("Sensor permission error: {:?}", e));
                        return;
                    }

                    let result = app.borrow_mut().start_tracking(SourceType::Camera).await;
                    if let Err(e) = result {
                        crate::utils::log(&format!("Error: {:?}", e));
                    }
                });
            }) as Box<dyn Fn()>);
            self.start_camera_btn.add_event_listener_with_callback(
                "click",
                closure.as_ref().unchecked_ref(),
            )?;
            closure.forget();
        }

        // Start screen button
        {
            let app = app_state.clone();
            let closure = Closure::wrap(Box::new(move || {
                let app = app.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    // Request sensor permissions first (must be in user gesture context)
                    if let Err(e) = request_sensor_permissions().await {
                        crate::utils::log(&format!("Sensor permission error: {:?}", e));
                        return;
                    }

                    let result = app.borrow_mut().start_tracking(SourceType::Screen).await;
                    if let Err(e) = result {
                        crate::utils::log(&format!("Error: {:?}", e));
                    }
                });
            }) as Box<dyn Fn()>);
            self.start_screen_btn.add_event_listener_with_callback(
                "click",
                closure.as_ref().unchecked_ref(),
            )?;
            closure.forget();
        }

        // Start combined button
        {
            let app = app_state.clone();
            let closure = Closure::wrap(Box::new(move || {
                let app = app.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    // Request sensor permissions first (must be in user gesture context)
                    if let Err(e) = request_sensor_permissions().await {
                        crate::utils::log(&format!("Sensor permission error: {:?}", e));
                        return;
                    }

                    let result = app.borrow_mut().start_tracking(SourceType::Combined).await;
                    if let Err(e) = result {
                        crate::utils::log(&format!("Error: {:?}", e));
                    }
                });
            }) as Box<dyn Fn()>);
            self.start_combined_btn.add_event_listener_with_callback(
                "click",
                closure.as_ref().unchecked_ref(),
            )?;
            closure.forget();
        }

        // Stop button
        {
            let app = app_state.clone();
            let closure = Closure::wrap(Box::new(move || {
                let app = app.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let result = app.borrow_mut().stop_tracking().await;
                    if let Err(e) = result {
                        crate::utils::log(&format!("Error: {:?}", e));
                    }
                });
            }) as Box<dyn Fn()>);
            self.stop_btn.add_event_listener_with_callback(
                "click",
                closure.as_ref().unchecked_ref(),
            )?;
            closure.forget();
        }

        // PiP size slider
        {
            let label_el = self.pip_size_label_el.clone();
            let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
                if let Some(target) = event.target() {
                    if let Ok(input) = target.dyn_into::<HtmlInputElement>() {
                        let value = input.value();
                        label_el.set_text_content(Some(&format!("{}%", value)));
                    }
                }
            }) as Box<dyn Fn(web_sys::Event)>);
            self.pip_size_el.add_event_listener_with_callback(
                "input",
                closure.as_ref().unchecked_ref(),
            )?;
            closure.forget();
        }

        Ok(())
    }
}

fn get_element_by_id<T: wasm_bindgen::JsCast>(
    document: &Document,
    id: &str,
) -> Result<T, JsValue> {
    document
        .get_element_by_id(id)
        .ok_or_else(|| format!("Element #{} not found", id).into())
        .and_then(|el| el.dyn_into::<T>().map_err(|_| format!("Element #{} has wrong type", id).into()))
}
