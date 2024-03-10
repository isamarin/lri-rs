use camino::Utf8PathBuf;
use lri_rs::{AwbGain, RawData, RawImage, Whitepoint};
use nalgebra::{Matrix3, Matrix3x1};

use crate::rotate;

pub fn make_png(img: &RawImage, path: Utf8PathBuf, awb_gain: AwbGain) {
	use rawproc::image::RawMetadata;
	use rawproc::{colorspace::BayerRgb, image::Image};

	let RawImage {
		camera,
		sensor,
		width,
		height,
		format,
		data: _data,
		sbro,
		color,
	} = img;

	println!(
		"{camera} {sensor:?} [{}:{}] {width}x{height} {format}",
		sbro.0, sbro.1
	);

	let bayered = img.unpack().unwrap();

	let (mut rgb, color_format) = match img.cfa_string() {
		Some(cfa_string) => {
			let rawimg: Image<u16, BayerRgb> = Image::from_raw_parts(
				4160,
				3120,
				// We only care about CFA here because all we're doing is debayering
				RawMetadata {
					whitebalance: [1.0; 3],
					whitelevels: [1024; 3],
					crop: None,
					// ugh CFA isn't exposed, so we pulled in rawloader for now
					cfa: rawloader::CFA::new(cfa_string),
					cam_to_xyz: nalgebra::Matrix3::zeros(),
				},
				bayered,
			);

			(rawimg.debayer().data, png::ColorType::Rgb)
			//(bayered, png::ColorType::Grayscale)
		}
		None => (bayered, png::ColorType::Grayscale),
	};

	rotate::rotate_180(rgb.as_mut_slice());
	let mut floats: Vec<f32> = rgb
		.into_iter()
		.map(|p| ((p.saturating_sub(42) as f32) / 1023.0))
		.collect();

	if !color.is_empty() {
		print!("\tAvailable whitepoints: ");
		color.iter().for_each(|c| print!("{:?} ", c.whitepoint));
		println!();
	}

	match img.color_info(Whitepoint::D65) {
		Some(c) => {
			println!("\tUsing D65");
			let to_xyz = Matrix3::from_row_slice(&c.forward_matrix);
			// We're using Whitepoint::D65, but there is no D50 profile.
			// If we use the BRUCE_XYZ_RGB_D65 matrix the image
			// comes out too warm.
			let to_srgb = Matrix3::from_row_slice(&BRUCE_XYZ_RGB_D50);

			let premul = to_xyz * to_srgb;

			let wb_max = awb_gain.r.max(awb_gain.b);

			let wb_r = awb_gain.r; // / wb_max;
			let wb_g = 1.0; // / wb_max;
			let wb_b = awb_gain.b; // / wb_max;

			for (idx, chnk) in floats.chunks_mut(3).enumerate() {
				/*let r = chnk[0] * (1.0 / c.rg);
				let g = chnk[1];
				let b = chnk[2] * (1.0 / c.bg);*/
				let r = chnk[0] * wb_r;
				let g = chnk[1] * wb_g;
				let b = chnk[2] * wb_b;

				if idx == 4 {
					println!(
						"R: {:.2},{:.2},{:.2} - W: {:.2},{:.2},{:.2}",
						chnk[0], chnk[1], chnk[2], r, g, b
					);
				}

				let px = Matrix3x1::new(r, g, b);
				let rgb = premul * px;

				chnk[0] = srgb_gamma(rgb[0]);
				chnk[1] = srgb_gamma(rgb[1]);
				chnk[2] = srgb_gamma(rgb[2]);

				if idx == 4 {
					println!(
						"S: {:.2},{:.2},{:.2} - G: {:.2},{:.2},{:.2}",
						rgb[0], rgb[1], rgb[2], chnk[0], chnk[1], chnk[2]
					);
				}
			}
		}
		None => {
			println!("\tColor profile for D65 not found. Doing gamma and nothing else!");
			floats.iter_mut().for_each(|f| *f = srgb_gamma(*f));
		}
	}

	let bytes: Vec<u8> = floats.into_iter().map(|f| (f * 255.0) as u8).collect();

	println!("\tWriting {}", &path);
	write_png(path, *width, *height, &bytes, color_format)
}

#[rustfmt::skip]
#[allow(dead_code)]
const BRUCE_XYZ_RGB_D50: [f32; 9] = [
	3.1338561,  -1.6168667, -0.4906146,
	-0.9787684,  1.9161415,  0.0334540,
	0.0719453,  -0.2289914,  1.4052427
];

#[rustfmt::skip]
const BRUCE_XYZ_RGB_D65: [f32; 9] = [
	 3.2404542, -1.5371385, -0.4985314,
	-0.9692660,  1.8760108,  0.0415560,
 	 0.0556434, -0.2040259,  1.0572252
];

#[rustfmt::skip]
const BRADFORD_D50_D65: [f32; 9] = [
	 0.9555766, -0.0230393,  0.0631636,
	-0.0282895,  1.0099416,  0.0210077,
	 0.0122982, -0.0204830,  1.3299098,
];

#[inline]
pub fn srgb_gamma(mut float: f32) -> f32 {
	if float <= 0.0031308 {
		float *= 12.92;
	} else {
		float = float.powf(1.0 / 2.4) * 1.055 - 0.055;
	}

	float.clamp(0.0, 1.0)
}

fn bayer(img: &RawImage, width: usize, height: usize) -> Vec<u8> {
	match img.data {
		RawData::Packed10bpp { data } => {
			let unpacked = img.unpack().unwrap();

			// I've only seen it on one color defintion or
			// something, but there's a black level of 42, so subtract it.
			// without it the image is entirely too red.
			//ten_data.iter_mut().for_each(|p| *p = p.saturating_sub(42));

			unpacked
				.into_iter()
				.map(|p| ((p.saturating_sub(42)) >> 2) as u8)
				.collect()
		}
		RawData::BayerJpeg {
			header: _,
			format,
			jpeg0,
			jpeg1,
			jpeg2,
			jpeg3,
		} => {
			let mut bayered = vec![0; width * height];

			match format {
				0 => {
					let mut into = vec![0; (width * height) / 4];

					let mut channel = |jpeg: &[u8], offset: usize| {
						zune_jpeg::JpegDecoder::new(jpeg)
							.decode_into(&mut into)
							.unwrap();

						for idx in 0..into.len() {
							let ww = width / 2;
							let in_x = idx % ww;
							let in_y = idx / ww;

							let bayer_x = (in_x * 2) + (offset % 2);
							let bayer_y = (in_y * 2) + (offset / 2);

							let bayer_idx = bayer_y * width + bayer_x;
							bayered[bayer_idx] = into[idx];
						}
					};

					//BGGR
					//RGGB
					//GRBG
					channel(jpeg0, 0);
					channel(jpeg1, 1);
					channel(jpeg2, 2);
					channel(jpeg3, 3);
				}
				1 => {
					zune_jpeg::JpegDecoder::new(jpeg0)
						.decode_into(&mut bayered)
						.unwrap();
				}
				_ => unreachable!(),
			}

			bayered
		}
	}
}

fn write_png<P: AsRef<std::path::Path>>(
	path: P,
	width: usize,
	height: usize,
	data: &[u8],
	color_format: png::ColorType,
) {
	//return;
	use std::fs::File;

	let file = File::create(path).unwrap();
	let mut enc = png::Encoder::new(file, width as u32, height as u32);
	enc.set_color(color_format);
	enc.set_depth(png::BitDepth::Eight);
	let mut writer = enc.write_header().unwrap();
	writer.write_image_data(data).unwrap();
}
