use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use std::cell::RefCell;
use std::rc::Rc;

use crate::recorder::state::RecorderState;
use crate::recorder::types::{SourceType, PipPosition};

thread_local! {
    static RECORDER_STATE: Rc<RefCell<RecorderState>> =
        Rc::new(RefCell::new(RecorderState::new()));
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
        let _ = state.borrow_mut().render_frame();
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
        let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
            if let Some(target) = event.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    if let Ok(value) = input.value().parse::<f64>() {
                        RECORDER_STATE.with(|state| {
                            state.borrow_mut().pip_size = value / 100.0;
                        });
                    }
                }
            }
        }) as Box<dyn FnMut(_)>);

        if let Ok(element) = slider.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.add_event_listener_with_callback("input", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }

    // PiP position buttons
    setup_pip_position_button(document, "pip-pos-tl", PipPosition::TopLeft);
    setup_pip_position_button(document, "pip-pos-tr", PipPosition::TopRight);
    setup_pip_position_button(document, "pip-pos-bl", PipPosition::BottomLeft);
    setup_pip_position_button(document, "pip-pos-br", PipPosition::BottomRight);
}

fn setup_pip_position_button(document: &web_sys::Document, id: &str, position: PipPosition) {
    if let Some(button) = document.get_element_by_id(id) {
        let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            RECORDER_STATE.with(|state| {
                state.borrow_mut().pip_position = position;
            });
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
        let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
            if let Some(target) = event.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    if let Ok(value) = input.value().parse::<f64>() {
                        RECORDER_STATE.with(|state| {
                            state.borrow_mut().chart_size = value / 100.0;
                        });
                    }
                }
            }
        }) as Box<dyn FnMut(_)>);

        if let Ok(element) = slider.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.add_event_listener_with_callback("input", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }

    // Chart position buttons
    setup_chart_position_button(document, "chart-pos-tl", PipPosition::TopLeft);
    setup_chart_position_button(document, "chart-pos-tr", PipPosition::TopRight);
    setup_chart_position_button(document, "chart-pos-bl", PipPosition::BottomLeft);
    setup_chart_position_button(document, "chart-pos-br", PipPosition::BottomRight);
}

fn setup_chart_position_button(document: &web_sys::Document, id: &str, position: PipPosition) {
    if let Some(button) = document.get_element_by_id(id) {
        let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            RECORDER_STATE.with(|state| {
                state.borrow_mut().chart_position = position;
            });
        }) as Box<dyn FnMut(_)>);

        if let Ok(element) = button.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
        }
        closure.forget();
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
                    }
                    Err(e) => {
                        crate::recorder::utils::log(&format!("[Recorder] Stop failed: {:?}", e));
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

            // Disable mode/settings during recording
            let control_ids = ["mode-camera", "mode-screen", "mode-combined",
                             "pip-size", "chart-enable", "chart-type", "chart-size"];
            for id in &control_ids {
                if let Some(element) = document.get_element_by_id(id) {
                    if let Ok(input) = element.dyn_into::<web_sys::HtmlInputElement>() {
                        input.set_disabled(recording);
                    }
                }
            }
        }
    }
}
