use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{MediaStream, MediaStreamConstraints, MediaStreamTrack};

pub async fn get_camera_stream() -> Result<MediaStream, JsValue> {
    let window = web_sys::window().ok_or("No window")?;
    let navigator = window.navigator();
    let media_devices = navigator.media_devices()?;

    let mut constraints = MediaStreamConstraints::new();
    constraints.set_audio(&JsValue::TRUE);
    constraints.set_video(&create_camera_constraints());

    let promise = media_devices.get_user_media_with_constraints(&constraints)?;
    let stream_js = JsFuture::from(promise).await?;
    Ok(MediaStream::from(stream_js))
}

pub async fn get_screen_stream() -> Result<MediaStream, JsValue> {
    let window = web_sys::window().ok_or("No window")?;
    let navigator = window.navigator();
    let media_devices = navigator.media_devices()?;

    let mut constraints = web_sys::DisplayMediaStreamConstraints::new();
    constraints.set_audio(&JsValue::TRUE);
    constraints.set_video(&create_screen_constraints());

    let promise = media_devices.get_display_media_with_constraints(&constraints)?;
    let stream_js = JsFuture::from(promise).await?;
    Ok(MediaStream::from(stream_js))
}

fn create_camera_constraints() -> JsValue {
    let obj = js_sys::Object::new();

    // facingMode: 'user' (front camera)
    js_sys::Reflect::set(&obj, &"facingMode".into(), &"user".into()).unwrap();

    // width: { ideal: 1280 }
    let width_obj = js_sys::Object::new();
    js_sys::Reflect::set(&width_obj, &"ideal".into(), &1280.into()).unwrap();
    js_sys::Reflect::set(&obj, &"width".into(), &width_obj).unwrap();

    // height: { ideal: 720 }
    let height_obj = js_sys::Object::new();
    js_sys::Reflect::set(&height_obj, &"ideal".into(), &720.into()).unwrap();
    js_sys::Reflect::set(&obj, &"height".into(), &height_obj).unwrap();

    obj.into()
}

fn create_screen_constraints() -> JsValue {
    let obj = js_sys::Object::new();

    // width: { ideal: 1920 }
    let width_obj = js_sys::Object::new();
    js_sys::Reflect::set(&width_obj, &"ideal".into(), &1920.into()).unwrap();
    js_sys::Reflect::set(&obj, &"width".into(), &width_obj).unwrap();

    // height: { ideal: 1080 }
    let height_obj = js_sys::Object::new();
    js_sys::Reflect::set(&height_obj, &"ideal".into(), &1080.into()).unwrap();
    js_sys::Reflect::set(&obj, &"height".into(), &height_obj).unwrap();

    // frameRate: { ideal: 30 }
    let frame_rate_obj = js_sys::Object::new();
    js_sys::Reflect::set(&frame_rate_obj, &"ideal".into(), &30.into()).unwrap();
    js_sys::Reflect::set(&obj, &"frameRate".into(), &frame_rate_obj).unwrap();

    obj.into()
}

pub fn add_screen_stop_listener(stream: &MediaStream, callback: Box<dyn Fn()>) -> Result<(), JsValue> {
    let tracks = stream.get_video_tracks();
    if tracks.length() > 0 {
        let track = MediaStreamTrack::from(tracks.get(0));
        let closure = Closure::wrap(callback as Box<dyn Fn()>);
        track.add_event_listener_with_callback("ended", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }
    Ok(())
}

pub fn stop_stream(stream: &MediaStream) {
    let tracks = stream.get_tracks();
    for i in 0..tracks.length() {
        let track = MediaStreamTrack::from(tracks.get(i));
        track.stop();
    }
}
