use js_sys::Date;

pub fn format_duration(seconds: f64) -> String {
    format!("{:.1}s", seconds)
}

pub fn format_size_mb(bytes: usize) -> String {
    format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
}

pub fn current_timestamp_ms() -> f64 {
    Date::now()
}

pub fn current_timestamp_utc() -> String {
    let date = Date::new_0();
    date.to_iso_string().as_string().unwrap()
}

pub fn log(msg: &str) {
    web_sys::console::log_1(&msg.into());
}
