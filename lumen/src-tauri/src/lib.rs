use std::collections::HashMap;

use camino::Utf8PathBuf;
use light::api::{self, DirScan, LriSummary};
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
	tauri::Builder::default()
		.manage(AppState::new())
		.plugin(tauri_plugin_dialog::init())
		.invoke_handler(tauri::generate_handler![
			inspect_lri,
			scan_directory,
			extract_lri,
			camera_thumbnails_batch,
			pick_lri_file,
			pick_directory,
			pick_output_dir,
		])
		.run(tauri::generate_context!())
		.expect("error while running lumen");
}