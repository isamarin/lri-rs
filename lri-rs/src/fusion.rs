use crate::types::CameraId;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FusionMeta {
	pub tof_range_m: Option<f32>,
	pub tof_calibration: Option<TofCalibration>,
	pub imu: Option<ImuSummary>,
	pub gps: Option<GpsFix>,
	pub module_geometry: Vec<ModuleGeometry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TofCalibration {
	pub offset_distance: f32,
	pub offset_measurement: f32,
	pub xtalk_distance: f32,
	pub xtalk_measurement: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImuSummary {
	pub frames: usize,
	pub accel_samples: usize,
	pub gyro_samples: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpsFix {
	pub latitude: f64,
	pub longitude: f64,
	pub altitude_m: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModuleGeometry {
	pub camera: CameraId,
	pub mirror_type: Option<MirrorType>,
	pub focus_calibrations: Vec<FocusCalibration>,
	pub distortion: DistortionSummary,
	pub has_vignetting: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MirrorType {
	None,
	Glued,
	Movable,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DistortionSummary {
	pub polynomial: bool,
	pub cra: bool,
	pub poly_coeffs: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FocusCalibration {
	pub focus_distance: f32,
	pub k_matrix: Option<[f32; 9]>,
	pub rotation: Option<[f32; 9]>,
	pub translation: Option<[f32; 3]>,
	pub reprojection_error: Option<f32>,
	pub focus_hall_code: Option<f32>,
	pub has_movable_mirror: bool,
}

impl FusionMeta {
	pub fn geometry_module_count(&self) -> usize {
		self.module_geometry.len()
	}

	pub fn modules_with_intrinsics(&self) -> usize {
		self.module_geometry
			.iter()
			.filter(|m| {
				m.focus_calibrations
					.iter()
					.any(|f| f.k_matrix.is_some())
			})
			.count()
	}
}

pub(crate) fn mirror_type_from_proto(
	mt: lri_proto::geometric_calibration::geometric_calibration::MirrorType,
) -> MirrorType {
	use lri_proto::geometric_calibration::geometric_calibration::MirrorType as Mt;
	match mt {
		Mt::NONE => MirrorType::None,
		Mt::GLUED => MirrorType::Glued,
		Mt::MOVABLE => MirrorType::Movable,
	}
}

pub(crate) fn mat3(mat: lri_proto::matrix3x3f::Matrix3x3F) -> [f32; 9] {
	[
		mat.x00(),
		mat.x01(),
		mat.x02(),
		mat.x10(),
		mat.x11(),
		mat.x12(),
		mat.x20(),
		mat.x21(),
		mat.x22(),
	]
}

pub(crate) fn point3(p: lri_proto::point3f::Point3F) -> [f32; 3] {
	[p.x(), p.y(), p.z()]
}

pub(crate) fn extract_module_geometry(
	mcal: &lri_proto::lightheader::FactoryModuleCalibration,
) -> Option<ModuleGeometry> {
	let geometry = mcal.geometry.as_ref()?;
	let camera = mcal.camera_id().into();

	let mirror_type = Some(mirror_type_from_proto(geometry.mirror_type()));

	let mut focus_calibrations = Vec::new();
	for bundle in &geometry.per_focus_calibration {
		let k_matrix = bundle
			.intrinsics
			.as_ref()
			.and_then(|i| i.k_mat.as_ref())
			.map(|m| mat3(m.clone()));

		let mut rotation = None;
		let mut translation = None;
		let mut reprojection_error = None;
		let mut has_movable_mirror = false;

		if let Some(extr) = bundle.extrinsics.as_ref() {
			if let Some(canonical) = extr.canonical.as_ref() {
				if let Some(r) = canonical.rotation.as_ref() {
					rotation = Some(mat3(r.clone()));
				}
				if let Some(t) = canonical.translation.as_ref() {
					translation = Some(point3(t.clone()));
				}
				reprojection_error = canonical.reprojection_error;
			}
			has_movable_mirror = extr.moveable_mirror.is_some();
		}

		focus_calibrations.push(FocusCalibration {
			focus_distance: bundle.focus_distance(),
			k_matrix,
			rotation,
			translation,
			reprojection_error,
			focus_hall_code: bundle.focus_hall_code,
			has_movable_mirror,
		});
	}

	let mut distortion = DistortionSummary::default();
	if let Some(dist) = geometry.distortion.as_ref() {
		if let Some(poly) = dist.polynomial.as_ref() {
			distortion.polynomial = true;
			distortion.poly_coeffs = poly.coeffs.len();
		}
		if dist.cra.is_some() {
			distortion.cra = true;
		}
	}

	Some(ModuleGeometry {
		camera,
		mirror_type,
		focus_calibrations,
		distortion,
		has_vignetting: mcal.vignetting.is_some(),
	})
}

pub(crate) fn imu_summary(imu_frames: &[lri_proto::imu_data::IMUData]) -> ImuSummary {
	let mut accel_samples = 0usize;
	let mut gyro_samples = 0usize;
	for frame in imu_frames {
		accel_samples += frame.accelerometer.len();
		gyro_samples += frame.gyroscope.len();
	}
	ImuSummary {
		frames: imu_frames.len(),
		accel_samples,
		gyro_samples,
	}
}

pub(crate) fn gps_from_proto(gps: lri_proto::gps_data::GPSData) -> Option<GpsFix> {
	let latitude = gps.latitude?;
	let longitude = gps.longitude?;
	let altitude_m = gps.altitude.into_option().map(|a| a.value());
	Some(GpsFix {
		latitude,
		longitude,
		altitude_m,
	})
}

pub(crate) fn tof_from_device(
	dc: lri_proto::lightheader::FactoryDeviceCalibration,
) -> Option<TofCalibration> {
	let tof = dc.tof.into_option()?;
	Some(TofCalibration {
		offset_distance: tof.offset_distance(),
		offset_measurement: tof.offset_measurement(),
		xtalk_distance: tof.xtalk_distance(),
		xtalk_measurement: tof.xtalk_measurement(),
	})
}