# NetPoke: Survey Feature Specification

## Overview

The Survey feature enables walk-through network assessments by capturing synchronized video, sensor data, and network measurements. This creates comprehensive documentation of network performance across physical space.

**Key Insight**: NetPoke surveys measure **application-layer performance**, complementing RF survey tools that measure radio signal characteristics.

---

## Current Implementation (Prototype)

The prototype is implemented in `server/static/camera-tracker.html`.

### Captured Data

#### Video Stream
- Rear camera (environment-facing)
- Resolution: Up to 1920x1080
- Format: WebM or MP4 (browser-dependent)
- Bitrate: 2.5 Mbps
- Recorded in 1-second chunks

#### Sensor Data (per motion event, ~60Hz)

```javascript
{
    timestampRelative: 1234,              // ms since start
    timestampUTC: "2024-01-15T10:30:00Z", // absolute time
    
    gps: {
        latitude: 37.7749,
        longitude: -122.4194,
        altitude: 15.0,
        accuracy: 5.0,           // meters
        altitudeAccuracy: 10.0,
        heading: 180.0,          // degrees from north
        speed: 1.2,              // m/s
        timestamp: "2024-01-15T10:30:00Z"
    },
    
    magnetometer: {              // absolute orientation (compass)
        alpha: 180.0,            // compass heading (0-360)
        beta: 0.0,
        gamma: 0.0,
        absolute: true
    },
    
    acceleration: {              // linear acceleration (m/s²)
        x: 0.1,
        y: 0.2,
        z: 9.8
    },
    
    accelerationIncludingGravity: {
        x: 0.1,
        y: 0.2,
        z: 9.8
    },
    
    rotationRate: {              // angular velocity (deg/s)
        alpha: 0.5,
        beta: 0.1,
        gamma: 0.2
    },
    
    orientation: {               // device orientation
        alpha: 45.0,             // rotation around z-axis
        beta: 0.0,               // rotation around x-axis
        gamma: 0.0,              // rotation around y-axis
        absolute: false
    }
}
```

### Storage

Currently uses IndexedDB for local storage:
- Video blob
- Motion data array
- Metadata (duration, frame count, timestamps)

### Current Limitations
- Manual download required (no server upload)
- No integration with WebRTC measurements
- No organization/project structure

---

## Planned Architecture

### Unified Survey Session

**Critical Architecture Decision**: Video and sensor data are stored **locally only** during the survey and uploaded **after** the survey completes. This prevents upload traffic from interfering with network measurements (latency, loss, RTT would be contaminated by concurrent large uploads).

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     During Survey (Real-time)                            │
│                                                                          │
│  survey_session_id: "survey_2024-01-15_abc123"                          │
│                                                                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐  │
│  │  Video Stream   │  │  Sensor Data    │  │  Network Measurements   │  │
│  │                 │  │                 │  │                         │  │
│  │  - Camera feed  │  │  - GPS          │  │  - Latency/jitter/loss  │  │
│  │  - Timestamps   │  │  - Accelerometer│  │  - Traceroute           │  │
│  │                 │  │  - Magnetometer │  │  - MTU discovery        │  │
│  │                 │  │  - Orientation  │  │  - Throughput           │  │
│  │                 │  │  - Timestamps   │  │  - Timestamps           │  │
│  └────────┬────────┘  └────────┬────────┘  └────────────┬────────────┘  │
│           │                    │                        │               │
│           ▼                    ▼                        ▼               │
│  ┌─────────────────────────────────────┐    ┌─────────────────────────┐ │
│  │     IndexedDB (Local Storage)       │    │   Server (Real-time)    │ │
│  │  - Video blob                       │    │   - Network metrics     │ │
│  │  - Sensor data JSON                 │    │   - Traceroute results  │ │
│  │  - survey_session_id for matching   │    │   - survey_session_id   │ │
│  └─────────────────────────────────────┘    └─────────────────────────┘ │
│                                                                          │
│  NO UPLOAD during survey - preserves measurement integrity               │
└──────────────────────────────────────────────────────────────────────────┘

                              Survey Ends
                                  │
                                  ▼

┌─────────────────────────────────────────────────────────────────────────┐
│                     After Survey (User-Initiated Upload)                 │
│                                                                          │
│  ┌─────────────────────────────────────┐                                │
│  │     IndexedDB (Local Storage)       │                                │
│  │  - Video blob                       │──────┐                         │
│  │  - Sensor data JSON                 │      │                         │
│  │  - survey_session_id                │      │                         │
│  └─────────────────────────────────────┘      │                         │
│                                               │  User clicks "Upload"   │
│                                               ▼                         │
│                              ┌────────────────────────┐                 │
│                              │  Survey Upload API     │                 │
│                              │  POST /api/survey      │                 │
│                              │  (chunked/resumable)   │                 │
│                              └────────────────────────┘                 │
│                                               │                         │
│                                               ▼                         │
│                              ┌────────────────────────┐                 │
│                              │  Server Storage        │                 │
│                              │  - Matches with        │                 │
│                              │    network data via    │                 │
│                              │    survey_session_id   │                 │
│                              └────────────────────────┘                 │
└─────────────────────────────────────────────────────────────────────────┘
```

### Why Local-First Storage?

1. **Measurement Integrity**: Uploading video (2.5 Mbps+) during measurement would:
   - Increase latency readings
   - Cause packet loss from congestion
   - Affect jitter measurements
   - Make throughput tests meaningless

2. **Offline Support**: Surveys can be performed in areas with poor connectivity and uploaded later when on a better connection.

3. **User Control**: User decides when to upload, can review locally first, can delete bad surveys without uploading.

4. **Bandwidth Efficiency**: Upload can happen over WiFi later, not consuming mobile data during survey.

### Data Correlation

All data streams share a common `survey_session_id` and timeline for post-hoc correlation:

**During Survey**:
- **Client (local)**: Stores video + sensors in IndexedDB with `survey_session_id`
- **Server (real-time)**: Receives network metrics tagged with same `survey_session_id`

**After Upload**:
- Server matches uploaded video/sensors with stored network data via `survey_session_id`
- All data aligned using UTC timestamps for synchronized playback

**Timeline Structure**:
- **Base timestamp**: Survey start time (UTC ISO 8601)
- **Relative timestamps**: Milliseconds since start (for each data point)
- **survey_session_id**: UUID linking all data streams

Example timeline:
```
survey_session_id: "survey_2024-01-15_abc123"
start_time_utc: "2024-01-15T10:30:00.000Z"

0ms      - Survey started
0ms      - First sensor reading (stored locally)
10ms     - First probe packet sent (server receives)
50ms     - Probe response received, RTT: 40ms (server stores)
100ms    - GPS fix acquired (stored locally)
500ms    - Traceroute started (server)
1000ms   - First video chunk recorded (stored locally)
5000ms   - Traceroute complete, 12 hops (server stores)
...
60000ms  - Survey ended

--- After survey ends ---

User clicks "Upload Survey"
  → Video blob uploaded with survey_session_id
  → Sensor JSON uploaded with survey_session_id
  → Server matches with existing network data
  → Survey status: "ready" for playback
```

### Data Storage Split

| Data Type | Stored During Survey | Uploaded After |
|-----------|---------------------|----------------|
| Video blob | Client (IndexedDB) | Yes (large) |
| Sensor data (JSON) | Client (IndexedDB) | Yes (medium) |
| Probe metrics | Server (real-time) | No (already on server) |
| Traceroute results | Server (real-time) | No (already on server) |
| MTU discovery | Server (real-time) | No (already on server) |
| Session metadata | Both | Merged on upload |

---

## Survey Modes

### Mode 1: Basic Network Test (Current)
- Network measurements only
- No video or sensors
- Suitable for desktop/laptop

### Mode 2: Mobile Survey (Primary Use Case)
- Video + sensors + network measurements
- Walk-through site assessment
- Requires mobile device with camera

### Mode 3: Comprehensive Survey (Future - Premium)
- Mode 2 features, plus:
- Screen capture of RF survey tool (e.g., Ekahau)
- Dual-stream recording (camera + screen)
- Requires desktop/laptop with RF tool
- Behind authenticated login (not Magic Key)

---

## Screen Capture Integration (Future)

### Use Case
Professional surveyor runs Ekahau on laptop while also capturing NetPoke metrics. Both RF data and application-layer data are synchronized.

### Implementation Approach

```javascript
// Request screen capture (requires user gesture)
const screenStream = await navigator.mediaDevices.getDisplayMedia({
    video: {
        displaySurface: "window",  // Capture specific window
        cursor: "never"
    }
});

// Combine with camera if available
const cameraStream = await navigator.mediaDevices.getUserMedia({
    video: { facingMode: "environment" }
});

// Create picture-in-picture or side-by-side
// Option A: PiP overlay
// Option B: Side-by-side canvas
// Option C: Separate video tracks
```

### Access Control
- Requires full authenticated login (not Magic Key)
- User must be member of organization
- Screen capture requires explicit user permission

### Data Structure

```javascript
{
    survey_session_id: "survey_2024-01-15_abc123",
    type: "comprehensive",
    
    streams: {
        camera: {
            blob: <Blob>,
            mimeType: "video/webm",
            resolution: "1920x1080"
        },
        screen: {
            blob: <Blob>,
            mimeType: "video/webm",
            resolution: "1920x1080",
            windowTitle: "Ekahau Pro"
        }
    },
    
    sensors: [...],
    networkMeasurements: [...],
    
    metadata: {
        startTimeUTC: "2024-01-15T10:30:00Z",
        duration: 3600,
        surveyor: "user@example.com",
        organization: "org_abc",
        project: "proj_xyz"
    }
}
```

---

## API Specification

### Survey Upload

```
POST /api/survey/upload
Content-Type: multipart/form-data
Authorization: Bearer <token> OR Cookie: survey_session=<magic_key_session>

Parts:
- metadata: JSON with survey metadata
- video: Video file (camera)
- screen: Video file (screen capture, optional)
- sensors: JSON with sensor data array
- network: JSON with network measurements
```

### Survey List

```
GET /api/survey/list?project_id=<project_id>
Authorization: Bearer <token>

Response:
{
    surveys: [
        {
            id: "survey_abc123",
            created: "2024-01-15T10:30:00Z",
            duration: 3600,
            hasVideo: true,
            hasScreen: false,
            surveyor: "user@example.com",
            summary: {
                avgLatency: 25.5,
                maxLatency: 150.0,
                packetLoss: 0.1,
                pathHops: 12
            }
        }
    ]
}
```

### Survey Retrieval

```
GET /api/survey/<survey_id>
Authorization: Bearer <token>

Response:
{
    id: "survey_abc123",
    metadata: {...},
    sensors: [...],
    networkMeasurements: [...],
    videoUrl: "/api/survey/abc123/video",
    screenUrl: "/api/survey/abc123/screen"  // if applicable
}
```

### Survey Video Stream

```
GET /api/survey/<survey_id>/video
Authorization: Bearer <token>
Range: bytes=0-1000000

Response: Video stream with range support
```

---

## Survey Playback UI

### Timeline View

```
┌─────────────────────────────────────────────────────────────────────────┐
│  Survey: Building A - Floor 2                     Duration: 5:32        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                                                                  │   │
│  │                    Video Playback                                │   │
│  │                    (scrubable)                                   │   │
│  │                                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  ──●────────────────────────────────────────────────────────────────   │
│  0:00                        Timeline                             5:32  │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  Latency (ms)                                                    │   │
│  │  100 ─┐    ┌─┐                                                   │   │
│  │   50 ─┤────┤ ├─────────────────┐  ┌─────                        │   │
│  │    0 ─┴────┴─┴─────────────────┴──┴─────────────────────────    │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  Packet Loss (%)                                                 │   │
│  │   5% ─      ┌┐                                                   │   │
│  │   0% ───────┴┴───────────────────────────────────────────────   │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  ┌───────────────────┐  ┌───────────────────────────────────────┐     │
│  │ Current Position  │  │ Network Path (at current time)         │     │
│  │ GPS: 37.77, -122.4│  │ You → Router → ISP → ... → Server     │     │
│  │ Heading: 180°     │  │ Hop 3: 45ms (bottleneck)               │     │
│  └───────────────────┘  └───────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────────────────┘
```

### Key Features
- **Video scrubbing**: Drag timeline to any point
- **Synchronized metrics**: Charts update with video position
- **Event markers**: Automatic markers for high latency, loss, path changes
- **Export**: Generate PDF report with screenshots and metrics

---

## Offline Support & Upload Flow

### Local-First Architecture

All video and sensor data is stored locally in IndexedDB during the survey. This is mandatory, not optional, because:

1. **Measurement integrity** - Upload would contaminate network metrics
2. **Reliability** - Survey completes even if connectivity is lost
3. **User control** - Review before uploading, delete bad takes

### IndexedDB Schema

```javascript
// Database: "WiFiVerifySurveys"

// Pending surveys (not yet uploaded)
{
    storeName: "pending_surveys",
    keyPath: "survey_session_id",
    indexes: ["created_at", "project_id", "status"],
    
    record: {
        survey_session_id: "survey_2024-01-15_abc123",
        project_id: "proj_xyz",           // From Magic Key or user selection
        magic_key_id: "mk_abc",           // If created via Magic Key
        
        created_at: "2024-01-15T10:30:00Z",
        ended_at: "2024-01-15T11:00:00Z",
        duration_seconds: 1800,
        
        // Large blobs stored separately for efficiency
        video_blob_id: "blob_video_abc123",
        screen_blob_id: null,             // Optional screen capture
        
        // Sensor data (can be large, but JSON)
        sensor_data: [...],               // Array of sensor readings
        
        // Metadata
        metadata: {
            device: "iPhone 15 Pro",
            browser: "Safari 17",
            has_video: true,
            has_screen: false,
            has_gps: true,
            sensor_count: 5400,
        },
        
        // Upload status
        status: "pending",                // pending | uploading | uploaded | failed
        upload_progress: 0,               // 0-100
        upload_error: null,
        last_upload_attempt: null,
    }
}

// Video blobs (stored separately for memory efficiency)
{
    storeName: "video_blobs",
    keyPath: "id",
    
    record: {
        id: "blob_video_abc123",
        survey_session_id: "survey_2024-01-15_abc123",
        blob: <Blob>,                     // Actual video data
        mime_type: "video/webm",
        size_bytes: 52428800,             // 50MB
    }
}

// Screen capture blobs (optional, premium feature)
{
    storeName: "screen_blobs",
    keyPath: "id",
    
    record: {
        id: "blob_screen_abc123",
        survey_session_id: "survey_2024-01-15_abc123",
        blob: <Blob>,
        mime_type: "video/webm",
        size_bytes: 104857600,            // 100MB
    }
}
```

### Upload Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Survey List UI                                │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  Survey: 2024-01-15 10:30am                                    │ │
│  │  Duration: 30 min | Size: 150 MB | Status: Ready to upload     │ │
│  │                                                                │ │
│  │  [▶ Preview]  [⬆ Upload]  [🗑 Delete]                          │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  Survey: 2024-01-15 2:00pm                                     │ │
│  │  Duration: 15 min | Size: 75 MB | Status: Uploading (45%)      │ │
│  │                                                                │ │
│  │  [████████░░░░░░░░░░] 45%  [⏸ Pause]                           │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  Survey: 2024-01-14 9:00am                                     │ │
│  │  Duration: 45 min | Status: ✓ Uploaded                         │ │
│  │                                                                │ │
│  │  [🔗 View on Server]  [🗑 Delete Local Copy]                   │ │
│  └────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

### Upload Process

1. **User initiates upload** (manual action required)
2. **Check connectivity** - Warn if on metered/cellular connection
3. **Chunked upload** - Video uploaded in 5MB chunks for resumability
4. **Progress tracking** - UI shows upload progress
5. **Resumable** - Can pause and resume, survives page refresh
6. **Retry logic** - Exponential backoff on failure
7. **Completion** - Server confirms, local marked as "uploaded"
8. **Local cleanup** - Optional: delete local copy after confirmed upload

### Upload API

```
POST /api/survey/upload/init
Authorization: Bearer <token> OR Cookie: survey_session=<magic_key_session>
Content-Type: application/json

{
    "survey_session_id": "survey_2024-01-15_abc123",
    "project_id": "proj_xyz",
    "video_size_bytes": 52428800,
    "screen_size_bytes": 0,
    "sensor_data_size_bytes": 1048576,
    "metadata": {...}
}

Response:
{
    "upload_id": "upload_abc123",
    "video_upload_url": "/api/survey/upload/upload_abc123/video",
    "screen_upload_url": null,
    "sensor_upload_url": "/api/survey/upload/upload_abc123/sensors",
    "chunk_size_bytes": 5242880
}

---

PUT /api/survey/upload/<upload_id>/video
Content-Type: application/octet-stream
Content-Range: bytes 0-5242879/52428800

<binary chunk data>

Response:
{
    "received_bytes": 5242880,
    "total_bytes": 52428800,
    "complete": false
}

---

POST /api/survey/upload/<upload_id>/complete
Authorization: Bearer <token>

Response:
{
    "survey_id": "survey_abc123",
    "status": "processing",
    "view_url": "/surveys/survey_abc123"
}
```

### Network-Aware Upload

```javascript
// Check connection type before upload
const connection = navigator.connection;

if (connection) {
    if (connection.type === 'cellular' || connection.saveData) {
        // Warn user about data usage
        const proceed = await confirmDialog(
            "You're on a cellular connection. " +
            "This upload is " + formatBytes(totalSize) + ". " +
            "Continue or wait for WiFi?"
        );
        if (!proceed) return;
    }
    
    // Adjust chunk size based on connection
    if (connection.effectiveType === '4g') {
        chunkSize = 5 * 1024 * 1024;  // 5MB chunks
    } else if (connection.effectiveType === '3g') {
        chunkSize = 1 * 1024 * 1024;  // 1MB chunks
    } else {
        chunkSize = 256 * 1024;        // 256KB chunks
    }
}
```

---

## Data Retention

| Tier | Video Retention | Metrics Retention | Storage Limit |
|------|-----------------|-------------------|---------------|
| Free | 7 days | 30 days | 1 GB |
| Pro | 30 days | 1 year | 10 GB |
| Team | 90 days | 2 years | 100 GB |
| Enterprise | Custom | Unlimited | Custom |

---

## Privacy Considerations

### Video Content
- May capture faces, screens, sensitive areas
- Clear warning before recording starts
- Organization controls retention
- User can delete own surveys

### Location Data
- GPS coordinates included
- Can be disabled by user
- Aggregated/anonymized for analytics

### Network Data
- IP addresses visible in traceroute
- Internal network topology revealed
- Access controlled by organization

---

## Implementation Phases

### Phase 1: Integration (MVP)
- [ ] Integrate camera-tracker with network test page
- [ ] Add survey_session_id to all measurements
- [ ] Create survey upload API endpoint
- [ ] Basic survey list and retrieval

### Phase 2: Playback UI
- [ ] Timeline-based survey viewer
- [ ] Video scrubbing with metric sync
- [ ] Basic reporting/export

### Phase 3: Organization Support
- [ ] Link surveys to projects
- [ ] Magic Key generates project-scoped surveys
- [ ] Organization admin can view all surveys

### Phase 4: Screen Capture (Premium)
- [ ] Add screen capture option for authenticated users
- [ ] Dual-stream recording (camera + screen)
- [ ] Side-by-side playback view

### Phase 5: Advanced Features
- [ ] Automatic problem detection
- [ ] Path visualization on floor plan
- [ ] Comparative analysis (before/after)
- [ ] AR overlay (experimental)
