use wasm_bindgen::prelude::*;
use web_sys::MediaStream;

#[wasm_bindgen(module = "/static/lib/recorder/media_recorder.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    fn createMediaRecorder(stream: &MediaStream) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    fn startRecorder(id: &str) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    async fn stopRecorder(id: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    pub fn getChunksSize(id: &str) -> Result<f64, JsValue>;
}

pub struct Recorder {
    pub recorder_id: String,
    mime_type: String,
    pub start_time: f64,
    pub start_time_utc: String,
}

impl Recorder {
    pub fn new(stream: &MediaStream) -> Result<Self, JsValue> {
        let result = createMediaRecorder(stream)?;
        let obj = js_sys::Object::from(result);

        let id = js_sys::Reflect::get(&obj, &"id".into())?
            .as_string()
            .ok_or("Missing recorder id")?;
        let mime_type = js_sys::Reflect::get(&obj, &"mimeType".into())?
            .as_string()
            .ok_or("Missing mimeType")?;

        Ok(Self {
            recorder_id: id,
            mime_type,
            start_time: js_sys::Date::now(),
            start_time_utc: crate::recorder::utils::current_timestamp_utc(),
        })
    }

    pub fn start(&self) -> Result<(), JsValue> {
        startRecorder(&self.recorder_id)
    }

    pub async fn stop(&self) -> Result<web_sys::Blob, JsValue> {
        let blob_js = stopRecorder(&self.recorder_id).await?;
        Ok(web_sys::Blob::from(blob_js))
    }

    pub fn get_chunks_size(&self) -> f64 {
        getChunksSize(&self.recorder_id).unwrap_or(0.0)
    }

    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }
}
