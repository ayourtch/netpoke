use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceType {
    Camera,
    Screen,
    Combined,
}

impl SourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceType::Camera => "camera",
            SourceType::Screen => "screen",
            SourceType::Combined => "combined",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            SourceType::Camera => "Camera",
            SourceType::Screen => "Screen",
            SourceType::Combined => "Screen + Camera (PiP)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    #[serde(default)]
    pub frame_count: u32,
    pub duration: f64,
    #[serde(default)]
    pub mime_type: String,
    pub start_time_utc: String,
    pub end_time_utc: String,
    pub source_type: SourceType,
    #[serde(default)]
    pub camera_facing: CameraFacing,
    #[serde(default)]
    pub chart_included: bool,
    #[serde(default)]
    pub chart_type: Option<String>,
    #[serde(default)]
    pub test_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraFacing {
    #[serde(rename = "environment")]
    Environment,  // Back camera
    #[serde(rename = "user")]
    User,         // Front camera
    #[serde(rename = "unknown")]
    Unknown,      // Screen recording or unknown
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub id: String,
    pub timestamp: f64,
    pub blob_size: usize,
    pub metadata: RecordingMetadata,
}

impl Default for CameraFacing {
    fn default() -> Self {
        CameraFacing::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionDataPoint {
    pub timestamp_relative: f64,
    pub timestamp_utc: String,
    pub gps: Option<GpsData>,
    pub magnetometer: Option<OrientationData>,
    pub orientation: Option<OrientationData>,
    pub acceleration: AccelerationData,
    pub acceleration_including_gravity: AccelerationData,
    pub rotation_rate: RotationData,
    #[serde(default)]
    pub camera_direction: Option<f64>,  // Compass direction camera is facing (0-360°, 0=North)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsData {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub accuracy: f64,
    pub altitude_accuracy: Option<f64>,
    pub heading: Option<f64>,
    pub speed: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrientationData {
    pub alpha: Option<f64>,
    pub beta: Option<f64>,
    pub gamma: Option<f64>,
    pub absolute: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccelerationData {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationData {
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
}

#[derive(Debug, Clone)]
pub struct Recording {
    pub id: String,
    pub timestamp: f64,
    pub blob_size: usize,
    pub metadata: RecordingMetadata,
}
