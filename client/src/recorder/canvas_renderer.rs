use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlVideoElement};
use crate::recorder::types::SourceType;

pub struct CanvasRenderer {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
}

impl CanvasRenderer {
    pub fn new(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let ctx = canvas
            .get_context("2d")?
            .ok_or("No 2d context")?
            .dyn_into::<CanvasRenderingContext2d>()?;

        Ok(Self { canvas, ctx })
    }

    pub fn render_frame(
        &self,
        source_type: SourceType,
        screen_video: Option<&HtmlVideoElement>,
        camera_video: Option<&HtmlVideoElement>,
        pip_position: &str,
        pip_size_percent: f64,
    ) -> Result<(), JsValue> {
        match source_type {
            SourceType::Camera => {
                if let Some(video) = camera_video {
                    self.render_camera(video)?;
                }
            }
            SourceType::Screen => {
                if let Some(video) = screen_video {
                    self.render_screen(video)?;
                }
            }
            SourceType::Combined => {
                if let (Some(screen), Some(camera)) = (screen_video, camera_video) {
                    self.render_combined(screen, camera, pip_position, pip_size_percent)?;
                }
            }
        }
        Ok(())
    }

    fn render_camera(&self, camera_video: &HtmlVideoElement) -> Result<(), JsValue> {
        if camera_video.ready_state() < 2 {
            return Ok(());
        }

        let width = camera_video.video_width();
        let height = camera_video.video_height();

        if width == 0 || height == 0 {
            return Ok(());
        }

        self.canvas.set_width(width);
        self.canvas.set_height(height);

        self.ctx
            .draw_image_with_html_video_element(camera_video, 0.0, 0.0)?;

        let canvas_width = camera_video.video_width();
        // Draw marquee
        self.draw_marquee(canvas_width as f64)?;

        Ok(())
    }

    fn render_screen(&self, screen_video: &HtmlVideoElement) -> Result<(), JsValue> {
        if screen_video.ready_state() < 2 {
            return Ok(());
        }

        let width = screen_video.video_width();
        let height = screen_video.video_height();

        if width == 0 || height == 0 {
            return Ok(());
        }

        self.canvas.set_width(width);
        self.canvas.set_height(height);

        self.ctx
            .draw_image_with_html_video_element(screen_video, 0.0, 0.0)?;

        let canvas_width = screen_video.video_width();
        // Draw marquee
        self.draw_marquee(canvas_width as f64)?;

        Ok(())
    }

    fn render_combined(
        &self,
        screen_video: &HtmlVideoElement,
        camera_video: &HtmlVideoElement,
        pip_position: &str,
        pip_size_percent: f64,
    ) -> Result<(), JsValue> {
        if screen_video.ready_state() < 2 {
            return Ok(());
        }

        let canvas_width = screen_video.video_width();
        let canvas_height = screen_video.video_height();

        if canvas_width == 0 || canvas_height == 0 {
            return Ok(());
        }

        self.canvas.set_width(canvas_width);
        self.canvas.set_height(canvas_height);

        // Draw screen as background
        self.ctx.draw_image_with_html_video_element_and_dw_and_dh(
            screen_video,
            0.0,
            0.0,
            canvas_width as f64,
            canvas_height as f64,
        )?;

        // Draw PiP camera overlay
        if camera_video.ready_state() >= 2 {
            let camera_width = camera_video.video_width() as f64;
            let camera_height = camera_video.video_height() as f64;

            if camera_width > 0.0 && camera_height > 0.0 {
                let pip_width = canvas_width as f64 * (pip_size_percent / 100.0);
                let pip_height = (camera_height / camera_width) * pip_width;
                let margin = 20.0;

                let (pip_x, pip_y) = match pip_position {
                    "bottom-right" => (
                        canvas_width as f64 - pip_width - margin,
                        canvas_height as f64 - pip_height - margin,
                    ),
                    "bottom-left" => (margin, canvas_height as f64 - pip_height - margin),
                    "top-right" => (canvas_width as f64 - pip_width - margin, margin),
                    "top-left" => (margin, margin),
                    _ => (
                        canvas_width as f64 - pip_width - margin,
                        canvas_height as f64 - pip_height - margin,
                    ),
                };

                // Draw shadow
                self.ctx.set_shadow_color("rgba(0,0,0,0.5)");
                self.ctx.set_shadow_blur(10.0);
                self.ctx.set_fill_style(&JsValue::from_str("#000"));
                self.ctx
                    .fill_rect(pip_x - 2.0, pip_y - 2.0, pip_width + 4.0, pip_height + 4.0);
                self.ctx.set_shadow_blur(0.0);

                // Draw camera feed
                self.ctx.draw_image_with_html_video_element_and_dw_and_dh(
                    camera_video,
                    pip_x,
                    pip_y,
                    pip_width,
                    pip_height,
                )?;

                // Draw border
                self.ctx.set_stroke_style(&JsValue::from_str("#fff"));
                self.ctx.set_line_width(2.0);
                self.ctx.stroke_rect(pip_x, pip_y, pip_width, pip_height);
            }
        }

        // Draw marquee
        self.draw_marquee(canvas_width as f64)?;

        Ok(())
    }

    fn draw_marquee(&self, canvas_width: f64) -> Result<(), JsValue> {
        let text = "https://stdio.be/cast - record your own screencast";
        let font_size = 20.0;
        let marquee_y = 30.0;

        self.ctx.set_font(&format!(
            "bold {}px system-ui, -apple-system, sans-serif",
            font_size
        ));
        self.ctx
            .set_fill_style(&JsValue::from_str("rgba(255, 255, 255, 0.9)"));
        self.ctx.set_text_align("center");

        // Scrolling effect at 0.123 pixels/ms
        let now = js_sys::Date::now();
        let text_metrics = self.ctx.measure_text(text)?;
        let text_width = text_metrics.width();
        let scroll_x = (now * 0.123) % (text_width + 200.0);
        let draw_x = canvas_width / 2.0 - scroll_x + 100.0;
        let draw_x2 = draw_x + text_width + 200.0;

        // Draw with shadow for readability
        self.ctx.set_shadow_color("rgba(0, 0, 0, 0.8)");
        self.ctx.set_shadow_blur(4.0);
        self.ctx.fill_text(text, draw_x, marquee_y + 12.0)?;
        self.ctx.fill_text(text, draw_x2, marquee_y + 12.0)?;
        self.ctx.set_shadow_blur(0.0);

        Ok(())
    }

    pub fn get_canvas_stream(&self, frame_rate: i32) -> Result<web_sys::MediaStream, JsValue> {
        self.canvas.capture_stream_with_frame_request_rate(frame_rate as f64)
    }

    pub fn render_sensor_overlay(
        &self,
        timestamp_utc: &str,
        gps: &Option<crate::recorder::types::GpsData>,
        magnetometer: &Option<crate::recorder::types::OrientationData>,
        orientation: &Option<crate::recorder::types::OrientationData>,
        acceleration: &Option<crate::recorder::types::AccelerationData>,
        camera_direction: &Option<f64>,
    ) -> Result<(), JsValue> {
        let ctx = &self.ctx;

        // Draw background panel (taller to fit camera direction)
        ctx.set_fill_style(&JsValue::from_str("rgba(0, 0, 0, 0.8)"));
        ctx.fill_rect(15.0, 15.0, 400.0, 138.0);

        // Set text style
        ctx.set_fill_style(&JsValue::from_str("#ffffff"));
        ctx.set_font("12px monospace");
        ctx.set_text_align("left");  // Left-justify text
        ctx.set_text_baseline("top");  // Align to top for consistent positioning

        let mut y = 30.0;  // Top of panel + 15px padding
        let x = 80.0;  // Left edge of panel + 65px padding
        let line_height = 18.0;

        // Timestamp
        ctx.fill_text(timestamp_utc, x, y)?;
        y += line_height;

        // GPS
        let gps_text = if let Some(gps_data) = gps {
            format!(
                "GPS: {:.6}, {:.6} ±{:.1}m",
                gps_data.latitude, gps_data.longitude, gps_data.accuracy
            )
        } else {
            "GPS: acquiring...".to_string()
        };
        ctx.fill_text(&gps_text, x, y)?;
        y += line_height;

        // Compass heading (from orientation when absolute=true)
        let compass_text = if let Some(orient) = orientation {
            if orient.absolute {
                if let Some(alpha) = orient.alpha {
                    format!("Compass: {:.0}°", alpha)
                } else {
                    "Compass: -".to_string()
                }
            } else {
                "Compass: not absolute".to_string()
            }
        } else {
            "Compass: -".to_string()
        };
        ctx.fill_text(&compass_text, x, y)?;
        y += line_height;

        // Camera direction (calculated from compass + camera facing)
        let camera_text = if let Some(direction) = camera_direction {
            format!("Camera facing: {:.0}°", direction)
        } else {
            "Camera facing: -".to_string()
        };
        ctx.fill_text(&camera_text, x, y)?;
        y += line_height;

        // Orientation
        let orient_text = if let Some(orient) = orientation {
            format!(
                "Tilt: β:{}° γ:{}°",
                orient.beta.map(|v| format!("{:.0}", v)).unwrap_or("-".to_string()),
                orient.gamma.map(|v| format!("{:.0}", v)).unwrap_or("-".to_string())
            )
        } else {
            "Tilt: -".to_string()
        };
        ctx.fill_text(&orient_text, x, y)?;
        y += line_height;

        // Acceleration
        let accel_text = if let Some(accel) = acceleration {
            format!(
                "Accel: x:{:.2} y:{:.2} z:{:.2}",
                accel.x, accel.y, accel.z
            )
        } else {
            "Accel: -".to_string()
        };
        ctx.fill_text(&accel_text, x, y)?;

        Ok(())
    }

    pub fn render_compass(
        &self,
        camera_direction: Option<f64>,
    ) -> Result<(), JsValue> {
        crate::recorder::utils::log(&format!("[COMPASS] render_compass called with direction: {:?}", camera_direction));

        let ctx = &self.ctx;

        // Position compass in top-right corner
        let canvas_width = self.canvas.width() as f64;
        let cx = canvas_width - 120.0;  // Center X
        let cy = 100.0;  // Center Y
        let radius = 50.0;

        // Only draw if we have a direction
        if let Some(direction) = camera_direction {
            crate::recorder::utils::log(&format!("[COMPASS] Drawing compass at ({}, {}) with direction {:.0}°", cx, cy, direction));
            // Save context
            ctx.save();

            // Draw compass base (slight isometric ellipse for 3D effect)
            ctx.set_fill_style(&JsValue::from_str("rgba(0, 0, 0, 0.7)"));
            ctx.begin_path();
            ctx.ellipse(cx, cy, radius, radius * 0.8, 0.0, 0.0, std::f64::consts::PI * 2.0)?;
            ctx.fill();

            // Draw border
            ctx.set_stroke_style(&JsValue::from_str("#ffffff"));
            ctx.set_line_width(2.0);
            ctx.stroke();

            // Translate to compass center for rotation
            ctx.translate(cx, cy)?;

            // Rotate so north points up when camera faces north
            // Rotate the compass to keep North pointing geographically north
            let rotation = direction.to_radians();
            ctx.rotate(rotation)?;

            // Draw cardinal directions
            ctx.set_fill_style(&JsValue::from_str("#ffffff"));
            ctx.set_font("14px bold sans-serif");
            ctx.set_text_align("center");
            ctx.set_text_baseline("middle");

            // North (blue)
            ctx.set_fill_style(&JsValue::from_str("#4444ff"));
            ctx.fill_text("N", 0.0, -radius + 15.0)?;

            // South (red)
            ctx.set_fill_style(&JsValue::from_str("#ff4444"));
            ctx.fill_text("S", 0.0, radius - 15.0)?;

            // East (white)
            ctx.set_fill_style(&JsValue::from_str("#ffffff"));
            ctx.fill_text("E", radius - 15.0, 0.0)?;

            // West (white)
            ctx.fill_text("W", -radius + 15.0, 0.0)?;

            // Draw north needle (pointing up = north, blue)
            ctx.set_fill_style(&JsValue::from_str("#4444ff"));
            ctx.begin_path();
            ctx.move_to(0.0, -radius + 25.0);  // Tip of needle
            ctx.line_to(-8.0, -10.0);  // Left base
            ctx.line_to(8.0, -10.0);   // Right base
            ctx.close_path();
            ctx.fill();

            // Draw south needle (red)
            ctx.set_fill_style(&JsValue::from_str("#ff4444"));
            ctx.begin_path();
            ctx.move_to(0.0, radius - 25.0);  // Tip
            ctx.line_to(-8.0, 10.0);   // Left base
            ctx.line_to(8.0, 10.0);    // Right base
            ctx.close_path();
            ctx.fill();

            // Draw center dot
            ctx.set_fill_style(&JsValue::from_str("#ffffff"));
            ctx.begin_path();
            ctx.arc(0.0, 0.0, 4.0, 0.0, std::f64::consts::PI * 2.0)?;
            ctx.fill();

            // Restore context
            ctx.restore();

            // Draw direction text below compass
            ctx.set_fill_style(&JsValue::from_str("#ffffff"));
            ctx.set_font("12px monospace");
            ctx.set_text_align("center");
            ctx.fill_text(&format!("{:.0}°", direction), cx, cy + radius + 15.0)?;
        } else {
            crate::recorder::utils::log("[COMPASS] No camera_direction, not drawing compass");
        }

        Ok(())
    }

    /// Composite Chart.js canvas into recording
    pub fn render_chart_overlay(
        &self,
        chart_element_id: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<(), JsValue> {
        let document = web_sys::window()
            .ok_or("No window")?
            .document()
            .ok_or("No document")?;

        let chart_canvas: web_sys::HtmlCanvasElement = document
            .get_element_by_id(chart_element_id)
            .ok_or("Chart canvas not found")?
            .dyn_into()
            .map_err(|_| "Element is not a canvas")?;

        self.ctx
            .draw_image_with_html_canvas_element_and_dw_and_dh(
                &chart_canvas,
                x,
                y,
                width,
                height,
            )?;

        Ok(())
    }
}
