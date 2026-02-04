# Issue 012: Missing Download Functions for Recordings

## Summary
The standalone camera app exports WASM functions for downloading recorded videos and motion data. These allow the HTML UI to trigger downloads. The netpoke client does not export these functions, so the download buttons in the UI will not work.

## Location
- File: `client/src/lib.rs` - Should export download functions
- Reference: `tmp/camera-standalone-for-cross-check/src/lib.rs` lines 42-77, 262-309

## Current Behavior
The netpoke `client/src/lib.rs` does not export:
- `download_video(id: String)`
- `download_motion_data(id: String)`
- `delete_recording_by_id(id: String)`

The recordings UI in nettest.html has download buttons that call these functions:
```html
<button onclick="downloadVideo('{}')">Download Video</button>
<button onclick="downloadMotionData('{}')">Download Motion Data</button>
<button class="danger" onclick="deleteRecordingById('{}')">Delete</button>
```

## Expected Behavior
The standalone camera exports these functions:

```rust
#[wasm_bindgen]
pub async fn download_video(id: String) -> Result<(), JsValue> {
    // Get recording from IndexedDB
    // Create blob URL
    // Trigger download via anchor element
}

#[wasm_bindgen]
pub async fn download_motion_data(id: String) -> Result<(), JsValue> {
    // Get recording from IndexedDB
    // Create JSON blob with metadata and motion data
    // Trigger download
}

#[wasm_bindgen]
pub async fn delete_recording_by_id(id: String) -> Result<(), JsValue> {
    // Confirm with user
    // Delete from IndexedDB
    // Refresh recordings list
}
```

## Impact
- **Priority: High**
- Download Video button: non-functional
- Download Motion Data button: non-functional
- Delete button: non-functional
- Users cannot retrieve their recordings
- Core functionality is broken

## Suggested Implementation
Copy the following functions from `tmp/camera-standalone-for-cross-check/src/lib.rs` to `client/src/lib.rs`:

1. **download_video** (lines 42-77):
```rust
#[wasm_bindgen]
pub async fn download_video(id: String) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;

    let recording_js = crate::recorder::storage::getRecording(&id).await?;

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

    // Revoke URL after delay
    let url_clone = url.clone();
    let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
        let _ = web_sys::Url::revoke_object_url(&url_clone);
    }) as Box<dyn Fn()>);
    window.set_timeout_with_callback_and_timeout_and_arguments_0(
        closure.as_ref().unchecked_ref(),
        1000,
    )?;
    closure.forget();

    Ok(())
}
```

2. **download_motion_data** (lines 262-309)
3. **delete_recording_by_id** (lines 80-133)

Adapt imports to use `crate::recorder::*` paths.

---
*Created: 2026-02-04*
