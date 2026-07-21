use std::fs;

use anyhow::{Context, Result};
use camino::Utf8Path;
use image::{ImageBuffer, Luma, Rgba};
use lri_rs::{CameraId, LriFile};
use png::{BitDepth, ColorType, Encoder};

use crate::session::LriSession;
use crate::thumbnail;

const ROWS: &[&[&str]] = &[
	&["A1", "A2", "A3", "A4", "A5"],
	&["B1", "B2", "B3", "B4", "B5"],
	&["C1", "C2", "C3", "C4", "C5", "C6"],
];

const CELL_W: u32 = 160;
const CELL_H: u32 = 120;
const PAD: u32 = 8;
const LABEL_H: u32 = 18;

pub fn run(input: &Utf8Path, output: &Utf8Path) -> Result<()> {
	let session = LriSession::open(input)?;
	session.with_lri(|lri| run_decoded(lri, output))
}

fn run_decoded(lri: &LriFile<'_>, output: &Utf8Path) -> Result<()> {
	if !output.exists() {
		fs::create_dir_all(output).context("create output directory")?;
	}

	let mut thumbs = Vec::new();
	for img in &lri.images {
		let png = thumbnail::render_camera_png(lri, img.camera)?;
		let path = output.join(format!("{}.png", img.camera));
		fs::write(&path, &png).with_context(|| format!("write {path}"))?;
		eprintln!("  write {path}");
		thumbs.push((img.camera, png));
	}

	let grid_path = output.join("grid.png");
	let grid = compose_grid(lri, &thumbs)?;
	fs::write(&grid_path, encode_rgba_png(&grid)?).with_context(|| format!("write {grid_path}"))?;
	eprintln!("wrote {grid_path}");

	Ok(())
}

fn compose_grid(lri: &LriFile<'_>, thumbs: &[(CameraId, Vec<u8>)]) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>> {
	let by_cam: std::collections::HashMap<_, _> = thumbs.iter().map(|(c, p)| (*c, p.as_slice())).collect();

	let cols = ROWS.iter().map(|r| r.len()).max().unwrap_or(0) as u32;
	let rows = ROWS.len() as u32;
	let width = PAD + cols * (CELL_W + PAD);
	let height = PAD + rows * (LABEL_H + CELL_H + PAD);

	let mut canvas = ImageBuffer::from_fn(width, height, |_, _| Rgba([28, 28, 32, 255]));

	for (row_i, row) in ROWS.iter().enumerate() {
		let y0 = PAD + row_i as u32 * (LABEL_H + CELL_H + PAD);
		for (col_i, id) in row.iter().enumerate() {
			let x0 = PAD + col_i as u32 * (CELL_W + PAD);
			let camera = thumbnail::parse_camera_id(id);
			let is_ref = lri.image_reference_camera == camera;
			draw_label(&mut canvas, x0, y0, id, is_ref, camera.is_some_and(|c| by_cam.contains_key(&c)));

			let thumb_y = y0 + LABEL_H;
			if let Some(cam) = camera {
				if let Some(png) = by_cam.get(&cam) {
					if let Ok(img) = decode_png_gray(png) {
						blit_thumb(&mut canvas, x0, thumb_y, &img);
						continue;
					}
				}
			}
			draw_missing(&mut canvas, x0, thumb_y);
		}
	}

	Ok(canvas)
}

fn draw_label(canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, x: u32, y: u32, id: &str, is_ref: bool, present: bool) {
	let color = if is_ref {
		Rgba([120, 200, 255, 255])
	} else if present {
		Rgba([220, 220, 220, 255])
	} else {
		Rgba([100, 100, 110, 255])
	};
	let label = if is_ref {
		format!("{id} ref")
	} else {
		id.to_string()
	};
	draw_text(canvas, x + 4, y + 2, &label, color);
}

fn draw_text(canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, x: u32, y: u32, text: &str, color: Rgba<u8>) {
	for (i, ch) in text.chars().enumerate() {
		draw_char(canvas, x + i as u32 * 8, y, ch, color);
	}
}

fn draw_char(canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, x: u32, y: u32, ch: char, color: Rgba<u8>) {
	let glyph = tiny_font(ch);
	for (dy, row) in glyph.iter().enumerate() {
		for (dx, on) in row.iter().enumerate() {
			if *on {
				let px = x + dx as u32;
				let py = y + dy as u32;
				if px < canvas.width() && py < canvas.height() {
					canvas.put_pixel(px, py, color);
				}
			}
		}
	}
}

fn tiny_font(ch: char) -> [[bool; 6]; 8] {
	match ch {
		'A' => [
			[false, true, true, true, false, false],
			[true, false, false, false, true, false],
			[true, false, false, false, true, false],
			[true, true, true, true, true, false],
			[true, false, false, false, true, false],
			[true, false, false, false, true, false],
			[true, false, false, false, true, false],
			[false, false, false, false, false, false],
		],
		'B' => [
			[true, true, true, true, false, false],
			[true, false, false, false, true, false],
			[true, true, true, true, false, false],
			[true, false, false, false, true, false],
			[true, false, false, false, true, false],
			[true, true, true, true, false, false],
			[false, false, false, false, false, false],
			[false, false, false, false, false, false],
		],
		'C' => [
			[false, true, true, true, true, false],
			[true, false, false, false, false, false],
			[true, false, false, false, false, false],
			[true, false, false, false, false, false],
			[true, false, false, false, false, false],
			[false, true, true, true, true, false],
			[false, false, false, false, false, false],
			[false, false, false, false, false, false],
		],
		'0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' => digit_font(ch),
		' ' => [[false; 6]; 8],
		'r' => [
			[false, false, false, false, false, false],
			[true, true, true, false, false, false],
			[true, false, false, true, false, false],
			[true, false, false, false, false, false],
			[true, false, false, false, false, false],
			[true, false, false, false, false, false],
			[false, false, false, false, false, false],
			[false, false, false, false, false, false],
		],
		'e' => [
			[false, false, false, false, false, false],
			[false, true, true, true, false, false],
			[true, false, false, false, true, false],
			[true, true, true, true, true, false],
			[true, false, false, false, false, false],
			[false, true, true, true, false, false],
			[false, false, false, false, false, false],
			[false, false, false, false, false, false],
		],
		'f' => [
			[false, true, true, true, false, false],
			[true, false, false, false, false, false],
			[true, true, true, false, false, false],
			[true, false, false, false, false, false],
			[true, false, false, false, false, false],
			[true, false, false, false, false, false],
			[false, false, false, false, false, false],
			[false, false, false, false, false, false],
		],
		_ => [[false; 6]; 8],
	}
}

fn digit_font(ch: char) -> [[bool; 6]; 8] {
	match ch {
		'0' => [
			[false, true, true, true, false, false],
			[true, false, false, false, true, false],
			[true, false, false, false, true, false],
			[true, false, false, false, true, false],
			[true, false, false, false, true, false],
			[false, true, true, true, false, false],
			[false, false, false, false, false, false],
			[false, false, false, false, false, false],
		],
		'1' => [
			[false, false, true, false, false, false],
			[false, true, true, false, false, false],
			[false, false, true, false, false, false],
			[false, false, true, false, false, false],
			[false, false, true, false, false, false],
			[false, true, true, true, false, false],
			[false, false, false, false, false, false],
			[false, false, false, false, false, false],
		],
		'2' => [
			[false, true, true, true, false, false],
			[true, false, false, false, true, false],
			[false, false, false, true, false, false],
			[false, false, true, false, false, false],
			[false, true, false, false, false, false],
			[true, true, true, true, true, false],
			[false, false, false, false, false, false],
			[false, false, false, false, false, false],
		],
		'3' => [
			[true, true, true, true, false, false],
			[false, false, false, false, true, false],
			[false, true, true, true, false, false],
			[false, false, false, false, true, false],
			[false, false, false, false, true, false],
			[true, true, true, true, false, false],
			[false, false, false, false, false, false],
			[false, false, false, false, false, false],
		],
		'4' => [
			[false, false, true, true, false, false],
			[false, true, false, true, false, false],
			[true, false, false, true, false, false],
			[true, true, true, true, true, false],
			[false, false, false, true, false, false],
			[false, false, false, true, false, false],
			[false, false, false, false, false, false],
			[false, false, false, false, false, false],
		],
		'5' => [
			[true, true, true, true, true, false],
			[true, false, false, false, false, false],
			[true, true, true, true, false, false],
			[false, false, false, false, true, false],
			[false, false, false, false, true, false],
			[true, true, true, true, false, false],
			[false, false, false, false, false, false],
			[false, false, false, false, false, false],
		],
		'6' => [
			[false, true, true, true, false, false],
			[true, false, false, false, false, false],
			[true, true, true, true, false, false],
			[true, false, false, false, true, false],
			[true, false, false, false, true, false],
			[false, true, true, true, false, false],
			[false, false, false, false, false, false],
			[false, false, false, false, false, false],
		],
		_ => [[false; 6]; 8],
	}
}

fn draw_missing(canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, x: u32, y: u32) {
	for dy in 0..CELL_H {
		for dx in 0..CELL_W {
			let c = if (dx + dy) % 16 < 8 { 40 } else { 32 };
			canvas.put_pixel(x + dx, y + dy, Rgba([c, c, c + 2, 255]));
		}
	}
}

fn blit_thumb(canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, x: u32, y: u32, thumb: &ImageBuffer<Luma<u8>, Vec<u8>>) {
	let (tw, th) = thumb.dimensions();
	for dy in 0..CELL_H {
		for dx in 0..CELL_W {
			let sx = (dx as u32 * tw / CELL_W).min(tw.saturating_sub(1));
			let sy = (dy as u32 * th / CELL_H).min(th.saturating_sub(1));
			let px = thumb.get_pixel(sx, sy)[0];
			canvas.put_pixel(x + dx, y + dy, Rgba([px, px, px, 255]));
		}
	}
}

fn decode_png_gray(png: &[u8]) -> Result<ImageBuffer<Luma<u8>, Vec<u8>>> {
	let decoder = png::Decoder::new(png);
	let mut reader = decoder.read_info().context("png read")?;
	let mut buf = vec![0u8; reader.output_buffer_size()];
	let info = reader.next_frame(&mut buf).context("png frame")?;
	let bytes = &buf[..info.buffer_size()];
	match info.color_type {
		ColorType::Grayscale => {
			let img = ImageBuffer::from_raw(info.width, info.height, bytes.to_vec())
				.context("grayscale buffer")?;
			Ok(img)
		}
		_ => anyhow::bail!("unexpected png color type"),
	}
}

fn encode_rgba_png(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<Vec<u8>> {
	let (w, h) = img.dimensions();
	let raw: Vec<u8> = img.pixels().flat_map(|p| p.0).collect();
	let mut buf = Vec::new();
	{
		let mut enc = Encoder::new(&mut buf, w, h);
		enc.set_color(ColorType::Rgba);
		enc.set_depth(BitDepth::Eight);
		let mut writer = enc.write_header().context("grid png header")?;
		writer.write_image_data(&raw).context("grid png data")?;
	}
	Ok(buf)
}