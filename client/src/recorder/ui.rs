use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use std::cell::RefCell;
use std::rc::Rc;

use crate::recorder::state::RecorderState;
use crate::recorder::types::{ChartPosition, PipPosition, SourceType};

thread_local! {
    static RECORDER_STATE: Rc<RefCell<RecorderState>> =
        Rc::new(RefCell::new(RecorderState::new()));
}

/// Request sensor permissions from JavaScript (must be called from user gesture context)
async fn request_sensor_permissions() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("Failed to get window object for sensor permissions")?;

    let request_fn = js_sys::Reflect::get(&window, &"requestSensorPermissions".into())?;

    if !request_fn.is_function() {
        crate::recorder::utils::log("[Recorder] requestSensorPermissions not found in window, skipping sensor permission request");
        return Ok(());  // Not a fatal error - sensors may not be available
    }

    let request_fn: js_sys::Function = request_fn.dyn_into()?;
    let promise: js_sys::Promise = request_fn.call0(&window)?.dyn_into()?;
    let result = wasm_bindgen_futures::JsFuture::from(promise).await?;

    if result.is_truthy() {
        crate::recorder::utils::log("[Recorder] Sensor permissions granted");
        Ok(())
    } else {
        crate::recorder::utils::log("[Recorder] Sensor permissions denied, continuing without sensors");
        Ok(())  // Not a fatal error - recording can continue without sensors
    }
}

/// Start sensor tracking from JavaScript
async fn start_sensor_tracking() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("Failed to get window object for sensor tracking")?;

    let start_fn = js_sys::Reflect::get(&window, &"startSensorTracking".into())?;

    if !start_fn.is_function() {
        crate::recorder::utils::log("[Recorder] startSensorTracking not found in window, skipping sensor tracking");
        return Ok(());
    }

    let start_fn: js_sys::Function = start_fn.dyn_into()?;
    start_fn.call0(&window)?;
    crate::recorder::utils::log("[Recorder] Sensor tracking started");
    Ok(())
}

/// Stop sensor tracking from JavaScript
fn stop_sensor_tracking() {
    if let Some(window) = web_sys::window() {
        if let Ok(stop_fn) = js_sys::Reflect::get(&window, &"stopSensorTracking".into()) {
            if stop_fn.is_function() {
                if let Ok(func) = stop_fn.dyn_into::<js_sys::Function>() {
                    let _ = func.call0(&window);
                    crate::recorder::utils::log("[Recorder] Sensor tracking stopped");
                }
            }
        }
    }
}

#[wasm_bindgen]
pub fn init_recorder_panel() {
    let document = match web_sys::window()
        .and_then(|w| w.document())
    {
        Some(d) => d,
        None => return,
    };

    // Set up recording panel controls
    setup_mode_selection(&document);
    setup_pip_controls(&document);
    setup_chart_controls(&document);
    setup_sensor_overlay_toggle(&document);  // Issue 015
    setup_recording_buttons(&document);

    crate::recorder::utils::log("[Recorder] Panel initialized");
}

#[wasm_bindgen]
pub fn recorder_render_frame() {
    RECORDER_STATE.with(|state| {
        // Use try_borrow_mut to avoid panic if the state is already borrowed
        // (e.g., during start_recording or stop_recording async operations)
        if let Ok(mut state_guard) = state.try_borrow_mut() {
            if let Err(e) = state_guard.render_frame() {
                crate::recorder::utils::log(&format!("[Recorder] render_frame error: {:?}", e));
            }
        }
        // If borrow fails, silently skip this frame - the next frame will render
    });
}

fn setup_mode_selection(document: &web_sys::Document) {
    // Camera mode radio
    if let Some(radio) = document.get_element_by_id("mode-camera") {
        let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            RECORDER_STATE.with(|state| {
                state.borrow_mut().source_type = SourceType::Camera;
            });
            update_pip_visibility(false);
        }) as Box<dyn FnMut(_)>);

        if let Ok(element) = radio.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }

    // Screen mode radio
    if let Some(radio) = document.get_element_by_id("mode-screen") {
        let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            RECORDER_STATE.with(|state| {
                state.borrow_mut().source_type = SourceType::Screen;
            });
            update_pip_visibility(false);
        }) as Box<dyn FnMut(_)>);

        if let Ok(element) = radio.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }

    // Combined mode radio
    if let Some(radio) = document.get_element_by_id("mode-combined") {
        let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            RECORDER_STATE.with(|state| {
                state.borrow_mut().source_type = SourceType::Combined;
            });
            update_pip_visibility(true);
        }) as Box<dyn FnMut(_)>);

        if let Ok(element) = radio.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }
}

fn setup_pip_controls(document: &web_sys::Document) {
    // PiP size slider
    if let Some(slider) = document.get_element_by_id("pip-size") {
        // Cast to HtmlInputElement once (it inherits from HtmlElement so can do both read value and add listener)
        if let Ok(input_element) = slider.dyn_into::<web_sys::HtmlInputElement>() {
            // Read initial value from slider to sync state with DOM
            // This handles browser form restoration and ensures state matches the displayed value
            if let Ok(value) = input_element.value().parse::<f64>() {
                RECORDER_STATE.with(|state| {
                    state.borrow_mut().pip_size = value / 100.0;
                });
                crate::recorder::utils::log(&format!("[Recorder] Initial PiP size: {}%", value));
            }

            let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
                if let Some(target) = event.target() {
                    if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                        let value_str = input.value();
                        if let Ok(value) = value_str.parse::<f64>() {
                            RECORDER_STATE.with(|state| {
                                state.borrow_mut().pip_size = value / 100.0;
                            });
                            // Update the label text next to the slider
                            if let Some(window) = web_sys::window() {
                                if let Some(document) = window.document() {
                                    if let Some(label) = document.get_element_by_id("pip-size-value") {
                                        label.set_text_content(Some(&format!("{}%", value_str)));
                                    }
                                }
                            }
                        }
                    }
                }
            }) as Box<dyn FnMut(_)>);

            let _ = input_element.add_event_listener_with_callback("input", closure.as_ref().unchecked_ref());
            closure.forget();
        }
    }

    // PiP position buttons
    setup_pip_position_button(document, "pip-pos-tl", PipPosition::TopLeft);
    setup_pip_position_button(document, "pip-pos-tr", PipPosition::TopRight);
    setup_pip_position_button(document, "pip-pos-bl", PipPosition::BottomLeft);
    setup_pip_position_button(document, "pip-pos-br", PipPosition::BottomRight);
}

fn setup_pip_position_button(document: &web_sys::Document, id: &str, position: PipPosition) {
    if let Some(button) = document.get_element_by_id(id) {
        let id_owned = id.to_string();
        let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            RECORDER_STATE.with(|state| {
                state.borrow_mut().pip_position = position;
            });
            // Update visual "selected" state on buttons
            update_position_button_selection("pip-pos", &id_owned);
        }) as Box<dyn FnMut(_)>);

        if let Ok(element) = button.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }
}

fn setup_chart_controls(document: &web_sys::Document) {
    // Chart enable checkbox
    if let Some(checkbox) = document.get_element_by_id("chart-enable") {
        let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
            if let Some(target) = event.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    RECORDER_STATE.with(|state| {
                        state.borrow_mut().chart_enabled = input.checked();
                    });
                    update_chart_controls_visibility(input.checked());
                }
            }
        }) as Box<dyn FnMut(_)>);

        if let Ok(element) = checkbox.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }

    // Chart type select
    if let Some(select) = document.get_element_by_id("chart-type") {
        let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
            if let Some(target) = event.target() {
                if let Ok(select_elem) = target.dyn_into::<web_sys::HtmlSelectElement>() {
                    RECORDER_STATE.with(|state| {
                        state.borrow_mut().chart_type = select_elem.value();
                    });
                }
            }
        }) as Box<dyn FnMut(_)>);

        if let Ok(element) = select.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }

    // Chart size slider
    if let Some(slider) = document.get_element_by_id("chart-size") {
        // Cast to HtmlInputElement once (it inherits from HtmlElement so can do both read value and add listener)
        if let Ok(input_element) = slider.dyn_into::<web_sys::HtmlInputElement>() {
            // Read initial value from slider to sync state with DOM
            // This handles browser form restoration and ensures state matches the displayed value
            if let Ok(value) = input_element.value().parse::<f64>() {
                RECORDER_STATE.with(|state| {
                    state.borrow_mut().chart_size = value / 100.0;
                });
                crate::recorder::utils::log(&format!("[Recorder] Initial chart size: {}%", value));
            }

            let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
                if let Some(target) = event.target() {
                    if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                        let value_str = input.value();
                        if let Ok(value) = value_str.parse::<f64>() {
                            RECORDER_STATE.with(|state| {
                                state.borrow_mut().chart_size = value / 100.0;
                            });
                            // Update the label text next to the slider
                            if let Some(window) = web_sys::window() {
                                if let Some(document) = window.document() {
                                    if let Some(label) = document.get_element_by_id("chart-size-value") {
                                        label.set_text_content(Some(&format!("{}%", value_str)));
                                    }
                                }
                            }
                        }
                    }
                }
            }) as Box<dyn FnMut(_)>);

            let _ = input_element.add_event_listener_with_callback("input", closure.as_ref().unchecked_ref());
            closure.forget();
        }
    }

    // Chart position buttons (top and bottom only)
    setup_chart_position_button(document, "chart-pos-top", ChartPosition::Top);
    setup_chart_position_button(document, "chart-pos-bottom", ChartPosition::Bottom);
}

fn setup_chart_position_button(document: &web_sys::Document, id: &str, position: ChartPosition) {
    if let Some(button) = document.get_element_by_id(id) {
        let id_owned = id.to_string();
        let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            RECORDER_STATE.with(|state| {
                state.borrow_mut().chart_position = position;
            });
            // Update visual "selected" state on buttons
            update_chart_position_button_selection(&id_owned);
        }) as Box<dyn FnMut(_)>);

        if let Ok(element) = button.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }
}

/// Updates the visual "selected" class on chart position buttons.
fn update_chart_position_button_selection(selected_id: &str) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            // List of all chart position button IDs
            let button_ids = ["chart-pos-top", "chart-pos-bottom"];

            for button_id in &button_ids {
                if let Some(element) = document.get_element_by_id(button_id) {
                    // Get current class list
                    let class_name = element.class_name();

                    if *button_id == selected_id {
                        // Add "selected" class if not already present
                        if !class_name.contains("selected") {
                            element.set_class_name(&format!("{} selected", class_name.trim()));
                        }
                    } else {
                        // Remove "selected" class
                        let new_class = class_name
                            .split_whitespace()
                            .filter(|c| *c != "selected")
                            .collect::<Vec<_>>()
                            .join(" ");
                        element.set_class_name(&new_class);
                    }
                }
            }
        }
    }
}

/// Updates the visual "selected" class on PiP position buttons.
/// prefix is the button ID prefix (e.g., "pip-pos")
/// selected_id is the full ID of the button that should be selected
fn update_position_button_selection(prefix: &str, selected_id: &str) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            // List of all position suffixes
            let suffixes = ["tl", "tr", "bl", "br"];

            for suffix in &suffixes {
                let button_id = format!("{}-{}", prefix, suffix);
                if let Some(element) = document.get_element_by_id(&button_id) {
                    // Get current class list
                    let class_name = element.class_name();

                    if button_id == selected_id {
                        // Add "selected" class if not already present
                        if !class_name.contains("selected") {
                            element.set_class_name(&format!("{} selected", class_name.trim()));
                        }
                    } else {
                        // Remove "selected" class
                        let new_class = class_name
                            .split_whitespace()
                            .filter(|c| *c != "selected")
                            .collect::<Vec<_>>()
                            .join(" ");
                        element.set_class_name(&new_class);
                    }
                }
            }
        }
    }
}

fn setup_sensor_overlay_toggle(document: &web_sys::Document) {
    // Sensor overlay checkbox (Issue 015)
    if let Some(checkbox) = document.get_element_by_id("show-sensors-overlay") {
        let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
            if let Some(target) = event.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    let enabled = input.checked();
                    // Update global sensor manager overlay setting
                    if let Ok(mut manager_guard) = crate::SENSOR_MANAGER.lock() {
                        if let Some(ref mut mgr) = *manager_guard {
                            mgr.set_overlay_enabled(enabled);
                            crate::recorder::utils::log(&format!(
                                "[Recorder] Sensor overlay {}",
                                if enabled { "enabled" } else { "disabled" }
                            ));
                        }
                    }
                }
            }
        }) as Box<dyn FnMut(_)>);

        if let Ok(element) = checkbox.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }
}

fn setup_recording_buttons(document: &web_sys::Document) {
    // Start recording button
    if let Some(button) = document.get_element_by_id("start-recording") {
        let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            wasm_bindgen_futures::spawn_local(async {
                // Request sensor permissions first (must be in user gesture context)
                if let Err(e) = request_sensor_permissions().await {
                    crate::recorder::utils::log(&format!("[Recorder] Sensor permission error: {:?}", e));
                    // Continue anyway - sensors are optional
                }

                // Start sensor tracking
                if let Err(e) = start_sensor_tracking().await {
                    crate::recorder::utils::log(&format!("[Recorder] Sensor tracking start error: {:?}", e));
                    // Continue anyway - sensors are optional
                }

                let result = RECORDER_STATE.with(|state| {
                    let state_clone = state.clone();
                    async move {
                        state_clone.borrow_mut().start_recording().await
                    }
                });

                match result.await {
                    Ok(_) => {
                        crate::recorder::utils::log("[Recorder] Started successfully");
                        update_recording_ui(true);
                    }
                    Err(e) => {
                        crate::recorder::utils::log(&format!("[Recorder] Start failed: {:?}", e));
                        // Stop sensor tracking if recording failed to start
                        stop_sensor_tracking();
                    }
                }
            });
        }) as Box<dyn FnMut(_)>);

        if let Ok(element) = button.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }

    // Stop recording button
    if let Some(button) = document.get_element_by_id("stop-recording") {
        let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            wasm_bindgen_futures::spawn_local(async {
                let result = RECORDER_STATE.with(|state| {
                    let state_clone = state.clone();
                    async move {
                        state_clone.borrow_mut().stop_recording().await
                    }
                });

                match result.await {
                    Ok(_) => {
                        crate::recorder::utils::log("[Recorder] Stopped successfully");
                        update_recording_ui(false);
                        // Stop sensor tracking when recording stops
                        stop_sensor_tracking();
                    }
                    Err(e) => {
                        crate::recorder::utils::log(&format!("[Recorder] Stop failed: {:?}", e));
                        // Still try to stop sensor tracking
                        stop_sensor_tracking();
                    }
                }
            });
        }) as Box<dyn FnMut(_)>);

        if let Ok(element) = button.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }
}

fn update_pip_visibility(visible: bool) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(element) = document.get_element_by_id("pip-controls") {
                if let Ok(html_element) = element.dyn_into::<web_sys::HtmlElement>() {
                    let display_value = if visible { "block" } else { "none" };
                    let _ = html_element.set_attribute("style", &format!("display: {}", display_value));
                }
            }
        }
    }
}

fn update_chart_controls_visibility(visible: bool) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            let ids = ["chart-type", "chart-size", "chart-position"];
            for id in &ids {
                if let Some(element) = document.get_element_by_id(id) {
                    if let Some(parent) = element.parent_element() {
                        if let Ok(html_parent) = parent.dyn_into::<web_sys::HtmlElement>() {
                            let display_value = if visible { "block" } else { "none" };
                            let _ = html_parent.set_attribute("style", &format!("display: {}", display_value));
                        }
                    }
                }
            }
        }
    }
}

fn update_recording_ui(recording: bool) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            // Toggle button states
            if let Some(start_btn) = document.get_element_by_id("start-recording") {
                if let Ok(button) = start_btn.dyn_into::<web_sys::HtmlButtonElement>() {
                    button.set_disabled(recording);
                }
            }

            if let Some(stop_btn) = document.get_element_by_id("stop-recording") {
                if let Ok(button) = stop_btn.dyn_into::<web_sys::HtmlButtonElement>() {
                    button.set_disabled(!recording);
                }
            }

            // Disable mode/settings during recording (but NOT pip-size to allow resizing during recording)
            let control_ids = ["mode-camera", "mode-screen", "mode-combined",
                             "chart-enable", "chart-type", "chart-size"];
            for id in &control_ids {
                if let Some(element) = document.get_element_by_id(id) {
                    if let Ok(input) = element.dyn_into::<web_sys::HtmlInputElement>() {
                        input.set_disabled(recording);
                    }
                }
            }

            // Update recording status badge
            if let Some(status_badge) = document.get_element_by_id("recording-status") {
                if recording {
                    status_badge.set_text_content(Some("Recording"));
                    status_badge.set_class_name("status-badge recording");
                } else {
                    status_badge.set_text_content(Some("Ready"));
                    status_badge.set_class_name("status-badge ready");
                }
            }

            // Show/hide metrics display
            if let Some(metrics) = document.get_element_by_id("recording-metrics") {
                if let Ok(element) = metrics.dyn_into::<web_sys::HtmlElement>() {
                    if recording {
                        element.style().set_property("display", "block").ok();
                    } else {
                        element.style().set_property("display", "none").ok();
                    }
                }
            }

            // Show/hide recording buttons
            if let Some(start_btn) = document.get_element_by_id("start-recording") {
                if let Ok(element) = start_btn.dyn_into::<web_sys::HtmlElement>() {
                    if recording {
                        element.style().set_property("display", "none").ok();
                    } else {
                        element.style().set_property("display", "inline-block").ok();
                    }
                }
            }
            if let Some(stop_btn) = document.get_element_by_id("stop-recording") {
                if let Ok(element) = stop_btn.dyn_into::<web_sys::HtmlElement>() {
                    if recording {
                        element.style().set_property("display", "inline-block").ok();
                    } else {
                        element.style().set_property("display", "none").ok();
                    }
                }
            }
        }
    }
}

// Update recording metrics display (called from render loop)
pub fn update_recording_metrics(duration_secs: f64, frame_count: u32, size_bytes: u64) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(el) = document.get_element_by_id("recording-duration") {
                el.set_text_content(Some(&format!("{:.1}s", duration_secs)));
            }
            if let Some(el) = document.get_element_by_id("recording-frames") {
                el.set_text_content(Some(&frame_count.to_string()));
            }
            if let Some(el) = document.get_element_by_id("recording-size") {
                let size_mb = size_bytes as f64 / (1024.0 * 1024.0);
                el.set_text_content(Some(&format!("{:.2} MB", size_mb)));
            }
        }
    }
}
