use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD, Engine};
use camino::Utf8PathBuf;
use light::api::{self, DirScan, LriSummary};
use light::fuse::{self, FuseSummary};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

mod state;

use state::AppState;

#[derive(Clone, Serialize)]
struct ExportProgress {
	done: usize,
	total: usize,
	camera: String,
}

#[derive(Clone, Serialize)]
struct FuseProgress {
	stage: String,
	done: usize,
	total: usize,
}

#[derive(Serialize)]
struct FuseResult {
	summary: FuseSummary,
	output_dir: String,
	preview_data_url: String,
	export_paths: Vec<String>,
}

fn png_to_data_url(path: &camino::Utf8Path) -> Result<String, String> {
	let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
	Ok(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}

#[tauri::command]
fn inspect_lri(state: State<AppState>, path: String) -> Result<LriSummary, String> {
	state.open(&path)
}

#[tauri::command]
fn scan_directory(path: String) -> Result<DirScan, String> {
	api::scan_directory(&path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn extract_lri(
	app: AppHandle,
	state: State<'_, AppState>,
	input: String,
	output: String,
	jobs: Option<usize>,
) -> Result<usize, String> {
	let summary = state.open(&input)?;
	let count = summary.image_count;
	let input = Utf8PathBuf::from(input);
	let output = Utf8PathBuf::from(output);
	let handle = state.inner().clone();

	let app2 = app.clone();
	tauri::async_runtime::spawn_blocking(move || {
		handle.with_session(input.as_str(), |session| {
			light::extract::run_session_with_progress(session, &output, jobs, move |done, total, camera| {
				let _ = app2.emit(
					"export-progress",
					ExportProgress {
						done,
						total,
						camera: camera.to_string(),
					},
				);
			})
			.map_err(|e| e.to_string())
		})
	})
	.await
	.map_err(|e| e.to_string())??;

	Ok(count)
}

#[tauri::command]
async fn fuse_lri(
	app: AppHandle,
	input: String,
	output: Option<String>,
	max_side: Option<u32>,
	full_res: bool,
	export_tiff: bool,
	export_dng: bool,
	lumen_jpg: Option<String>,
) -> Result<FuseResult, String> {
	let input_path = Utf8PathBuf::from(&input);
	let output_path = match output {
		Some(dir) => Utf8PathBuf::from(dir),
		None => {
			let stem = input_path
				.file_stem()
				.unwrap_or("fuse");
			std::env::temp_dir()
				.join(format!("luminat-fuse-{stem}"))
				.try_into()
				.map_err(|_| "temp output path".to_string())?
		}
	};

	let lumen = lumen_jpg.map(Utf8PathBuf::from);
	let max_side = max_side.unwrap_or(1024);
	let app2 = app.clone();

	tauri::async_runtime::spawn_blocking(move || {
		let summary = fuse::run_with_progress(
			&input_path,
			&output_path,
			lumen.as_deref(),
			max_side,
			full_res,
			export_tiff,
			export_dng,
			1500.0,
			8000.0,
			25,
			move |stage, done, total| {
				let _ = app2.emit(
					"fuse-progress",
					FuseProgress {
						stage: stage.to_string(),
						done,
						total,
					},
				);
			},
		)
		.map_err(|e| e.to_string())?;

		let preview_file = if full_res {
			output_path.join("fused_cropped.png")
		} else {
			output_path.join("fused.png")
		};
		let preview_data_url = png_to_data_url(&preview_file)?;
		let export_paths: Vec<String> = summary
			.exports
			.iter()
			.map(|name| output_path.join(name).to_string())
			.collect();

		Ok(FuseResult {
			summary,
			output_dir: output_path.to_string(),
			preview_data_url,
			export_paths,
		})
	})
	.await
	.map_err(|e| e.to_string())?
}

#[tauri::command]
fn camera_thumbnails_batch(
	state: State<AppState>,
	path: String,
	cameras: Vec<String>,
	jobs: Option<usize>,
) -> Result<HashMap<String, String>, String> {
	state.with_session(&path, |session| {
		session
			.with_lri(|lri| {
				let ids: Vec<_> = cameras
					.iter()
					.filter_map(|c| light::thumbnail::parse_camera_id(c))
					.collect();
				light::thumbnail::thumbnails_batch(lri, &ids, jobs)
			})
			.map_err(|e| e.to_string())
	})
}

#[tauri::command]
async fn pick_lri_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
	let path = app
		.dialog()
		.file()
		.add_filter("Light RAW", &["lri"])
		.blocking_pick_file();
	Ok(path.map(|p| p.to_string()))
}

#[tauri::command]
async fn pick_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
	let path = app.dialog().file().blocking_pick_folder();
	Ok(path.map(|p| p.to_string()))
}

#[tauri::command]
async fn pick_output_dir(app: tauri::AppHandle) -> Result<Option<String>, String> {
	let path = app.dialog().file().blocking_pick_folder();
	Ok(path.map(|p| p.to_string()))
}

#[tauri::command]
async fn pick_lumen_jpg(app: tauri::AppHandle) -> Result<Option<String>, String> {
	let path = app
		.dialog()
		.file()
		.add_filter("JPEG", &["jpg", "jpeg"])
		.blocking_pick_file();
	Ok(path.map(|p| p.to_string()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
	tauri::Builder::default()
		.manage(AppState::new())
		.plugin(tauri_plugin_drag::init())
		.plugin(tauri_plugin_dialog::init())
		.invoke_handler(tauri::generate_handler![
			inspect_lri,
			scan_directory,
			extract_lri,
			fuse_lri,
			camera_thumbnails_batch,
			pick_lri_file,
			pick_directory,
			pick_output_dir,
			pick_lumen_jpg,
		])
		.run(tauri::generate_context!())
		.expect("error while running Luminat");
}