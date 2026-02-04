use wasm_bindgen::prelude::*;
use std::sync::Mutex;
use once_cell::sync::Lazy;

mod utils;
mod types;
mod ui;
mod storage;
mod media_streams;
mod canvas_renderer;
mod recorder;
mod app;
mod sensors;

pub static SENSOR_MANAGER: Lazy<Mutex<Option<crate::sensors::SensorManager>>> =
    Lazy::new(|| Mutex::new(None));

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub async fn start() -> Result<(), JsValue> {
    utils::log("Initializing Camera WASM...");

    let app_state = app::AppState::new().await?;

    {
        let app = app_state.borrow();
        let ui = app.get_ui();
        ui.register_event_listeners(app_state.clone())?;
        ui.show_ready_state()?;
    }

    utils::log("Camera WASM initialized successfully");

    Ok(())
}

#[wasm_bindgen]
pub async fn download_video(id: String) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;

    let recording_js = storage::getRecording(&id).await?;

    let obj = js_sys::Object::from(recording_js);
    let video_blob_js = js_sys::Reflect::get(&obj, &"videoBlob".into())?;
    let video_blob: web_sys::Blob = video_blob_js.dyn_into()?;

    let metadata_js = js_sys::Reflect::get(&obj, &"metadata".into())?;
    let mime_type = js_sys::Reflect::get(&metadata_js, &"mimeType".into())?
        .as_string()
        .unwrap_or("video/webm".to_string());

    let url = web_sys::Url::create_object_url_with_blob(&video_blob)?;

    let a: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
    a.set_href(&url);

    let extension = if mime_type.contains("mp4") { "mp4" } else { "webm" };
    a.set_download(&format!("video_{}.{}", id, extension));
    a.click();

    let url_clone = url.clone();
    let closure = Closure::wrap(Box::new(move || {
        let _ = web_sys::Url::revoke_object_url(&url_clone);
    }) as Box<dyn Fn()>);
    window.set_timeout_with_callback_and_timeout_and_arguments_0(
        closure.as_ref().unchecked_ref(),
        1000,
    )?;
    closure.forget();

    Ok(())
}

#[wasm_bindgen]
pub async fn delete_recording_by_id(id: String) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("No window")?;

    let confirmed = window.confirm_with_message("Delete this recording?")?;
    if !confirmed {
        return Ok(());
    }

    let db = storage::IndexedDbWrapper::open().await?;
    db.delete_recording(&id).await?;

    // Refresh the list
    let document = window.document().ok_or("No document")?;
    let recordings_list_el: web_sys::HtmlElement = document
        .get_element_by_id("recordingsList")
        .ok_or("recordingsList not found")?
        .dyn_into()?;

    let recordings = db.get_all_recordings().await?;

    if recordings.is_empty() {
        recordings_list_el.set_inner_html("<p style=\"color:#888;\">No recordings yet</p>");
    } else {
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

        recordings_list_el.set_inner_html(&html);
    }

    Ok(())
}

#[wasm_bindgen]
pub fn on_gps_update(
    latitude: f64,
    longitude: f64,
    altitude: Option<f64>,
    accuracy: f64,
    altitude_accuracy: Option<f64>,
    heading: Option<f64>,
    speed: Option<f64>,
) {
    let gps = crate::types::GpsData {
        latitude,
        longitude,
        altitude,
        accuracy,
        altitude_accuracy,
        heading,
        speed,
    };

    if let Ok(mut manager) = SENSOR_MANAGER.lock() {
        if let Some(mgr) = manager.as_mut() {
            mgr.update_gps(gps);
        }
    }
}

#[wasm_bindgen]
pub fn on_orientation(
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
    absolute: bool,
) {
    utils::log(&format!(
        "[COMPASS] on_orientation called: alpha={:?}, absolute={}",
        alpha, absolute
    ));

    let orientation = crate::types::OrientationData {
        alpha,
        beta,
        gamma,
        absolute,
    };

    if let Ok(mut manager) = SENSOR_MANAGER.lock() {
        if let Some(mgr) = manager.as_mut() {
            mgr.update_orientation(orientation);
        } else {
            utils::log("[COMPASS] No sensor manager in global state");
        }
    } else {
        utils::log("[COMPASS] Failed to lock SENSOR_MANAGER");
    }
}

#[wasm_bindgen]
pub fn on_magnetometer(
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
) {
    let magnetometer = crate::types::OrientationData {
        alpha,
        beta,
        gamma,
        absolute: true,
    };

    if let Ok(mut manager) = SENSOR_MANAGER.lock() {
        if let Some(mgr) = manager.as_mut() {
            mgr.update_magnetometer(magnetometer);
        }
    }
}

#[wasm_bindgen]
pub fn on_motion(
    accel_x: f64,
    accel_y: f64,
    accel_z: f64,
    accel_g_x: f64,
    accel_g_y: f64,
    accel_g_z: f64,
    rot_alpha: f64,
    rot_beta: f64,
    rot_gamma: f64,
) {
    let acceleration = crate::types::AccelerationData {
        x: accel_x,
        y: accel_y,
        z: accel_z,
    };

    let acceleration_g = crate::types::AccelerationData {
        x: accel_g_x,
        y: accel_g_y,
        z: accel_g_z,
    };

    let rotation = crate::types::RotationData {
        alpha: rot_alpha,
        beta: rot_beta,
        gamma: rot_gamma,
    };

    let timestamp_utc = js_sys::Date::new_0().to_iso_string().as_string().unwrap();
    let current_time = js_sys::Date::now();

    if let Ok(mut manager) = SENSOR_MANAGER.lock() {
        if let Some(mgr) = manager.as_mut() {
            mgr.add_motion_event(timestamp_utc, current_time, acceleration, acceleration_g, rotation);
        }
    }
}

#[wasm_bindgen]
pub fn set_sensor_overlay_enabled(enabled: bool) {
    if let Ok(mut manager) = SENSOR_MANAGER.lock() {
        if let Some(mgr) = manager.as_mut() {
            mgr.set_overlay_enabled(enabled);
        }
    }
}

#[wasm_bindgen]
pub async fn download_motion_data(id: String) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;

    let recording_js = storage::getRecording(&id).await?;
    let obj = js_sys::Object::from(recording_js);

    let metadata_js = js_sys::Reflect::get(&obj, &"metadata".into())?;
    let motion_data_js = js_sys::Reflect::get(&obj, &"motionData".into())?;

    // Create JSON object
    let json_obj = js_sys::Object::new();
    js_sys::Reflect::set(&json_obj, &"id".into(), &JsValue::from_str(&id))?;
    js_sys::Reflect::set(&json_obj, &"metadata".into(), &metadata_js)?;
    js_sys::Reflect::set(&json_obj, &"motionData".into(), &motion_data_js)?;

    let json_string = js_sys::JSON::stringify_with_replacer_and_space(
        &json_obj,
        &JsValue::NULL,
        &JsValue::from_f64(2.0),
    )?;

    // Create blob and download
    let array = js_sys::Array::new();
    array.push(&json_string);
    let blob = web_sys::Blob::new_with_str_sequence_and_options(
        &array,
        web_sys::BlobPropertyBag::new().type_("application/json"),
    )?;

    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let a: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
    a.set_href(&url);
    a.set_download(&format!("motion_{}.json", id));
    a.click();

    let url_clone = url.clone();
    let closure = Closure::wrap(Box::new(move || {
        let _ = web_sys::Url::revoke_object_url(&url_clone);
    }) as Box<dyn Fn()>);
    window.set_timeout_with_callback_and_timeout_and_arguments_0(
        closure.as_ref().unchecked_ref(),
        1000,
    )?;
    closure.forget();

    Ok(())
}
