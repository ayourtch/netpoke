# Survey Upload Feature

This document describes how to use the survey video + sensor upload feature in NetPoke.

## Overview

The survey upload feature allows surveyors to:
- Record video and sensor data during network surveys
- Upload recordings to the server with a single click
- Resume interrupted uploads automatically

Analysts can then:
- Browse uploaded surveys by magic key
- View survey metrics and recordings
- Download PCAP files and DTLS keylogs

## For Surveyors

### Recording During a Survey

1. Start a network survey ("Analyze Network")
2. Click "Start Recording" to begin capturing video and sensors
3. Move around while recording to capture different locations
4. Click "Stop Recording" when finished

### Uploading Recordings

1. After stopping a recording, it appears in the Recordings list
2. (Optional) Click "✏️ Notes" to add notes about this recording
3. Click "📤 Upload" to start uploading
4. Wait for the progress bar to complete
5. The button will change to "✓ Uploaded" when complete

### Resuming Interrupted Uploads

If an upload is interrupted (browser closed, network issue):
1. Re-open the survey page
2. Click "Upload" again on the same recording
3. The upload will resume from where it left off

### Troubleshooting

**Upload button shows "⚠️ Retry Upload"**
- The upload failed. Click to retry.
- Check your network connection.
- Ensure the survey session is still valid.

**Progress bar stuck**
- The server may be busy or unreachable.
- Check the browser console for errors.
- Try refreshing the page and uploading again.

**"No active survey session" error**
- You must start a survey ("Analyze Network") before uploading
- The survey session ID is required for organizing uploads

## For Analysts

### Accessing Survey Data

Survey data is available through the analyst API:

```bash
# List all magic keys with session counts
curl "http://server:8080/admin/api/magic-keys"

# List sessions for a magic key
curl "http://server:8080/admin/api/sessions?magic_key=SURVEY-001"

# Get session details
curl "http://server:8080/admin/api/sessions/{session_id}"
```

### API Response Format

**List Magic Keys Response:**
```json
[
  {
    "magic_key": "SURVEY-001",
    "session_count": 5,
    "total_recordings": 12,
    "latest_session_time": 1707146732000
  }
]
```

**List Sessions Response:**
```json
[
  {
    "session_id": "a7f3b2c1-...",
    "magic_key": "SURVEY-001",
    "start_time": 1707145395000,
    "last_update_time": 1707146732000,
    "has_pcap": true,
    "has_keylog": true,
    "recording_count": 2
  }
]
```

**Session Details Response:**
```json
{
  "session_id": "a7f3b2c1-...",
  "magic_key": "SURVEY-001",
  "user_login": null,
  "start_time": 1707145395000,
  "last_update_time": 1707146732000,
  "pcap_path": "/var/lib/netpoke/uploads/...",
  "keylog_path": "/var/lib/netpoke/uploads/...",
  "recordings": [
    {
      "recording_id": "d4e5f6a7-...",
      "video_size_bytes": 52428800,
      "sensor_size_bytes": 1048576,
      "upload_status": "complete",
      "user_notes": "Basement survey",
      "completed_at": 1707146800000
    }
  ],
  "metric_count": 1337
}
```

### File Storage Location

Uploaded files are stored in:
```
{storage.base_path}/{magic_key}/{YYYY}/{MM}/{DD}/{session_id}/
  ├── {recording_id}.webm   # Video recording
  └── {recording_id}.json   # Sensor data
```

## Server Configuration

Add to `server_config.toml`:

```toml
[database]
path = "/var/lib/netpoke/netpoke.db"

[storage]
base_path = "/var/lib/netpoke/uploads"
max_video_size_bytes = 1073741824  # 1 GB
chunk_size_bytes = 1048576          # 1 MB
```

### Configuration Options

| Option | Default | Description |
|--------|---------|-------------|
| database.path | /var/lib/netpoke/netpoke.db | SQLite database location |
| storage.base_path | /var/lib/netpoke/uploads | Upload storage directory |
| storage.max_video_size_bytes | 1 GB | Maximum video file size |
| storage.chunk_size_bytes | 1 MB | Upload chunk size |

### Directory Permissions

Ensure the server process has write access:
```bash
mkdir -p /var/lib/netpoke/uploads
chown -R netpoke:netpoke /var/lib/netpoke
chmod 750 /var/lib/netpoke
```

## Database Schema

The feature uses SQLite with three tables:
- `survey_sessions` - Survey session metadata
- `survey_metrics` - Per-second network metrics
- `recordings` - Uploaded recording metadata

See `server/migrations/001_survey_upload_schema.sql` for full schema.

## Upload Protocol

The upload uses a three-phase protocol for reliability:

### Phase 1: Prepare
```
POST /api/upload/prepare
Content-Type: application/json

{
  "session_id": "uuid",
  "recording_id": "uuid",
  "video_size_bytes": 52428800,
  "sensor_size_bytes": 1048576,
  "device_info": {...},
  "user_notes": "optional notes"
}
```

Returns existing chunk checksums for resume capability.

### Phase 2: Upload Chunks
```
POST /api/upload/chunk
X-Recording-Id: uuid
X-File-Type: video|sensor
X-Chunk-Index: 0
X-Chunk-Checksum: sha256
Content-Type: application/octet-stream

[binary chunk data, up to 1MB]
```

Chunks can be uploaded in any order. Server verifies checksums.

### Phase 3: Finalize
```
POST /api/upload/finalize
Content-Type: application/json

{
  "recording_id": "uuid",
  "video_final_checksum": "sha256",
  "sensor_final_checksum": "sha256"
}
```

Server verifies complete file integrity before marking as complete.

## Future Enhancements

- Web-based analyst UI for browsing surveys
- CSV/JSON export of metrics
- Access control for analyst API
- Automatic cleanup of old surveys
- Video + sensor synchronized playback
