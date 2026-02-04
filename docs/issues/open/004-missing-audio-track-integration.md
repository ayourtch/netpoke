# Issue 004: Missing Audio Track Integration in Recording

## Summary
The standalone camera app properly captures and adds audio tracks to the recording stream. The netpoke recorder implementation does not include audio track handling, resulting in silent recordings.

## Location
- File: `client/src/recorder/state.rs`
- Function: `start_recording()`
- Reference: `tmp/camera-standalone-for-cross-check/src/app.rs` lines 238-257

## Current Behavior
In `client/src/recorder/state.rs` `start_recording()`:
```rust
// Start MediaRecorder with canvas stream
let canvas_stream = canvas
    .capture_stream()
    .map_err(|_| "Failed to capture canvas stream")?;

self.recorder = Some(Recorder::new(&canvas_stream)?);
```

The canvas stream only contains video. No audio tracks are added.

## Expected Behavior
The standalone camera implementation properly adds audio tracks:
```rust
// Add audio track from source
let source_stream = match source_type {
    SourceType::Camera | SourceType::Combined => self.camera_stream.as_ref(),
    SourceType::Screen => self.screen_stream.as_ref(),
};

if let Some(stream) = source_stream {
    let audio_tracks = stream.get_audio_tracks();
    if audio_tracks.length() > 0 {
        let audio_track = web_sys::MediaStreamTrack::from(audio_tracks.get(0));
        canvas_stream.add_track(&audio_track);
    }
}
```

## Impact
- **Priority: High**
- All recordings will be silent (no audio)
- For screen recordings, system audio will not be captured
- For camera recordings, microphone audio will not be captured
- Users expecting audio in their recordings will be disappointed

## Suggested Implementation
After creating the canvas stream but before creating the recorder, add audio track handling:

```rust
// In start_recording() after canvas_stream creation:

// Add audio track from source
let source_stream = match self.source_type {
    SourceType::Camera | SourceType::Combined => self.camera_stream.as_ref(),
    SourceType::Screen => self.screen_stream.as_ref(),
};

if let Some(stream) = source_stream {
    let audio_tracks = stream.get_audio_tracks();
    crate::recorder::utils::log(&format!("Found {} audio tracks", audio_tracks.length()));
    if audio_tracks.length() > 0 {
        let audio_track = web_sys::MediaStreamTrack::from(audio_tracks.get(0));
        canvas_stream.add_track(&audio_track);
        crate::recorder::utils::log("Added audio track to recording");
    }
}
```

Note: May need to add `web_sys::MediaStreamTrack` to the imports if not already present.

---
*Created: 2026-02-04*
