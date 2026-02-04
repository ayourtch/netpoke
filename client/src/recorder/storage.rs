use wasm_bindgen::prelude::*;
use crate::recorder::types::{Recording, RecordingMetadata, MotionDataPoint};

#[wasm_bindgen(module = "/static/lib/recorder/indexed_db.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    pub async fn openDb() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn saveRecording(
        id: &str,
        blob: &web_sys::Blob,
        metadata: &JsValue,
        motion_data: &JsValue,
    ) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn getAllRecordings() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn deleteRecording(id: &str) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn getRecording(id: &str) -> Result<JsValue, JsValue>;
}

pub struct IndexedDbWrapper;

impl IndexedDbWrapper {
    pub async fn open() -> Result<Self, JsValue> {
        openDb().await?;
        Ok(Self)
    }

    pub async fn save_recording(
        &self,
        id: &str,
        blob: &web_sys::Blob,
        metadata: &RecordingMetadata,
        motion_data: &[crate::recorder::types::MotionDataPoint],
    ) -> Result<(), JsValue> {
        let metadata_js = serde_wasm_bindgen::to_value(metadata)?;
        let motion_data_js = serde_wasm_bindgen::to_value(motion_data)?;
        saveRecording(id, blob, &metadata_js, &motion_data_js).await
    }

    pub async fn get_all_recordings(&self) -> Result<Vec<Recording>, JsValue> {
        let js_recordings = getAllRecordings().await?;
        let array: js_sys::Array = js_recordings.dyn_into()?;

        let mut recordings = Vec::new();
        for i in 0..array.length() {
            let item = array.get(i);
            let rec = self.parse_recording(item)?;
            recordings.push(rec);
        }

        Ok(recordings)
    }

    pub async fn delete_recording(&self, id: &str) -> Result<(), JsValue> {
        deleteRecording(id).await
    }

    fn parse_recording(&self, js_value: JsValue) -> Result<Recording, JsValue> {
        let obj = js_sys::Object::from(js_value);
        let id = js_sys::Reflect::get(&obj, &"id".into())?
            .as_string()
            .ok_or("Missing id")?;
        let timestamp = js_sys::Reflect::get(&obj, &"timestamp".into())?
            .as_f64()
            .ok_or("Missing timestamp")?;

        let video_blob_js = js_sys::Reflect::get(&obj, &"videoBlob".into())?;
        let video_blob: web_sys::Blob = video_blob_js.dyn_into()?;
        let blob_size = video_blob.size() as usize;

        let metadata_js = js_sys::Reflect::get(&obj, &"metadata".into())?;
        let metadata: RecordingMetadata = serde_wasm_bindgen::from_value(metadata_js)?;

        Ok(Recording {
            id,
            timestamp,
            blob_size,
            metadata,
        })
    }
}
