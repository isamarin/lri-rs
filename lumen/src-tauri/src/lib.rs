use camino::Utf8PathBuf;
use light::api::{self, DirScan, LriSummary};
use lri_rs::LriFile;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_dialog::DialogExt;

#[derive(Clone, Serialize)]
struct ExportProgress {
	done: usize,
	total: usize,
	camera: String,
}

#[tauri::command]
fn inspect_lri(path: String) -> Result<LriSummary, String> {
	api::inspect_file(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn scan_directory(path: String) -> Result<DirScan, String> {
	api::scan_directory(&path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn extract_lri(
	app: AppHandle,
	input: String,
	output: String,
	jobs: Option<usize>,
) -> Result<usize, String> {
	let input = Utf8PathBuf::from(input);
	let output = Utf8PathBuf::from(output);
	let count = api::inspect_file(&input)
		.map_err(|e| e.to_string())?
		.image_count;

	let app2 = app.clone();
	tauri::async_runtime::spawn_blocking(move || {
		light::extract::run_with_progress(&input, &output, jobs, move |done, total, camera| {
			let _ = app2.emit(
				"export-progress",
				ExportProgress {
					done,
					total,
					camera: camera.to_string(),
				},
			);
		})
	})
	.await
	.map_err(|e| e.to_string())?
	.map_err(|e| e.to_string())?;

	Ok(count)
}

#[tauri::command]
fn camera_thumbnail(path: String, camera: String) -> Result<String, String> {
	let camera = light::thumbnail::parse_camera_id(&camera).ok_or("unknown camera id")?;
	let data = std::fs::read(&path).map_err(|e| e.to_string())?;
	let lri = LriFile::decode(&data).map_err(|e| e.to_string())?;
	light::thumbnail::thumbnail_base64(&lri, camera).map_err(|e| e.to_string())
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
		.plugin(tauri_plugin_dialog::init())
		.invoke_handler(tauri::generate_handler![
			inspect_lri,
			scan_directory,
			extract_lri,
			camera_thumbnail,
			pick_lri_file,
			pick_directory,
			pick_output_dir,
		])
		.run(tauri::generate_context!())
		.expect("error while running lumen");
}