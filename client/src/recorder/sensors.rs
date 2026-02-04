use crate::recorder::types::{AccelerationData, CameraFacing, GpsData, MotionDataPoint, OrientationData, RotationData};

#[derive(Clone)]
pub struct SensorManager {
    motion_data: Vec<MotionDataPoint>,
    current_gps: Option<GpsData>,
    current_magnetometer: Option<OrientationData>,
    current_orientation: Option<OrientationData>,
    current_acceleration: Option<AccelerationData>,
    current_acceleration_g: Option<AccelerationData>,
    current_rotation: Option<RotationData>,
    start_time: f64,
    overlay_enabled: bool,
    camera_facing: CameraFacing,
}

impl SensorManager {
    pub fn new(start_time: f64, camera_facing: CameraFacing) -> Self {
        Self {
            motion_data: Vec::new(),
            current_gps: None,
            current_magnetometer: None,
            current_orientation: None,
            current_acceleration: None,
            current_acceleration_g: None,
            current_rotation: None,
            start_time,
            overlay_enabled: false,
            camera_facing,
        }
    }

    pub fn get_camera_facing(&self) -> CameraFacing {
        self.camera_facing
    }

    pub fn set_overlay_enabled(&mut self, enabled: bool) {
        self.overlay_enabled = enabled;
    }

    pub fn is_overlay_enabled(&self) -> bool {
        self.overlay_enabled
    }

    pub fn update_gps(&mut self, gps: GpsData) {
        self.current_gps = Some(gps);
    }

    pub fn update_magnetometer(&mut self, mag: OrientationData) {
        self.current_magnetometer = Some(mag);
    }

    pub fn update_orientation(&mut self, orientation: OrientationData) {
        crate::recorder::utils::log(&format!(
            "[COMPASS] update_orientation: alpha={:?}, absolute={}",
            orientation.alpha,
            orientation.absolute
        ));
        self.current_orientation = Some(orientation);
    }

    pub fn add_motion_event(
        &mut self,
        timestamp_utc: String,
        current_time: f64,
        acceleration: AccelerationData,
        acceleration_g: AccelerationData,
        rotation: RotationData,
    ) {
        self.current_acceleration = Some(acceleration.clone());
        self.current_acceleration_g = Some(acceleration_g.clone());
        self.current_rotation = Some(rotation.clone());

        // Calculate camera direction from orientation + camera type
        let camera_direction = self.calculate_camera_direction();

        let data_point = MotionDataPoint {
            timestamp_relative: current_time - self.start_time,
            timestamp_utc,
            gps: self.current_gps.clone(),
            magnetometer: self.current_magnetometer.clone(),
            orientation: self.current_orientation.clone(),
            acceleration,
            acceleration_including_gravity: acceleration_g,
            rotation_rate: rotation,
            camera_direction,
        };

        self.motion_data.push(data_point);
    }

    fn calculate_camera_direction(&self) -> Option<f64> {
        crate::recorder::utils::log("[COMPASS] calculate_camera_direction called");

        // Get compass heading from orientation (when absolute=true, alpha is compass heading)
        if self.current_orientation.is_none() {
            crate::recorder::utils::log("[COMPASS] No current_orientation available");
            return None;
        }

        let orientation = self.current_orientation.as_ref()?;

        crate::recorder::utils::log(&format!(
            "[COMPASS] Orientation: alpha={:?}, absolute={}",
            orientation.alpha,
            orientation.absolute
        ));

        if !orientation.absolute {
            crate::recorder::utils::log("[COMPASS] Orientation not absolute, no compass direction");
            return None;  // Need absolute orientation for compass
        }

        let device_heading = orientation.alpha?;

        // Calculate camera direction based on which camera is active
        let camera_direction = match self.camera_facing {
            CameraFacing::Environment => {
                // Back camera: points where device points
                device_heading
            }
            CameraFacing::User => {
                // Front camera: points opposite direction
                (device_heading + 180.0) % 360.0
            }
            CameraFacing::Unknown => {
                // Screen recording or unknown: no camera direction
                return None;
            }
        };

        crate::recorder::utils::log(&format!(
            "[COMPASS] Camera direction calculated: {:.0}° (camera_facing={:?})",
            camera_direction,
            self.camera_facing
        ));

        Some(camera_direction)
    }

    pub fn get_motion_data(&self) -> &Vec<MotionDataPoint> {
        &self.motion_data
    }

    pub fn get_current_gps(&self) -> &Option<GpsData> {
        &self.current_gps
    }

    pub fn get_current_magnetometer(&self) -> &Option<OrientationData> {
        &self.current_magnetometer
    }

    pub fn get_current_orientation(&self) -> &Option<OrientationData> {
        &self.current_orientation
    }

    pub fn get_current_acceleration(&self) -> &Option<AccelerationData> {
        &self.current_acceleration
    }

    pub fn get_current_camera_direction(&self) -> Option<f64> {
        crate::recorder::utils::log("[COMPASS] get_current_camera_direction called");
        let result = self.calculate_camera_direction();
        crate::recorder::utils::log(&format!("[COMPASS] get_current_camera_direction returning: {:?}", result));
        result
    }

    pub fn clear(&mut self) {
        self.motion_data.clear();
        self.current_gps = None;
        self.current_magnetometer = None;
        self.current_orientation = None;
        self.current_acceleration = None;
        self.current_acceleration_g = None;
        self.current_rotation = None;
    }
}
