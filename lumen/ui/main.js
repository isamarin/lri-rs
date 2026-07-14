const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;
const getCurrentWindow = window.__TAURI__.webviewWindow.getCurrentWindow;

const ROWS = [
	["A1", "A2", "A3", "A4", "A5"],
	["B1", "B2", "B3", "B4", "B5"],
	["C1", "C2", "C3", "C4", "C5", "C6"],
];

const $ = (sel) => document.querySelector(sel);

let current = null;
let files = [];
let thumbGen = 0;

function showDetail(show) {
	$("#empty-state").classList.toggle("hidden", show);
	$("#detail").classList.toggle("hidden", !show);
}

function metaCard(label, value) {
	return `<div class="meta-card"><div class="label">${label}</div><div class="value">${value ?? "—"}</div></div>`;
}

function renderMeta(summary) {
	const awb = summary.awb_gain
		? summary.awb_gain.map((v) => v.toFixed(2)).join(", ")
		: null;

	$("#meta-grid").innerHTML = [
		metaCard("File", summary.name),
		metaCard("Firmware", summary.firmware),
		metaCard("Focal", summary.focal_length),
		metaCard("Exposure", summary.integration_ms != null ? `${summary.integration_ms} ms` : null),
		metaCard("Gain", summary.gain != null ? summary.gain.toFixed(2) : null),
		metaCard("HDR", summary.hdr),
		metaCard("Scene", summary.scene),
		metaCard("Tripod", summary.on_tripod),
		metaCard("AF", summary.af_achieved),
		metaCard("AWB", summary.awb),
		metaCard("WB gains", awb),
		metaCard("Reference", summary.reference_camera),
		metaCard(
			"Fusion data",
			summary.fusion
				? `geo ${summary.fusion.modules_with_intrinsics}/${summary.fusion.geometry_modules}` +
					(summary.fusion.tof_range_m != null ? ` · tof ${summary.fusion.tof_range_m.toFixed(2)}m` : "") +
					(summary.fusion.imu_frames != null ? ` · imu ${summary.fusion.imu_frames}` : "") +
					(summary.fusion.has_gps ? " · gps" : "")
				: null
		),
	].join("");
}

function camClass(cam) {
	if (!cam) return "missing";
	if (cam.bayer_jpeg) return "color-bayer";
	if (cam.sensor.includes("Mono")) return "color-mono";
	return "color-packed";
}

function renderCameras(summary) {
	const byId = Object.fromEntries(summary.cameras.map((c) => [c.id, c]));
	$("#camera-count").textContent = String(summary.image_count);

	$("#camera-grid").innerHTML = ROWS.map((row) => {
		const cells = row.map((id) => {
			const cam = byId[id];
			const ref = summary.reference_camera === id ? " ref" : "";
			if (!cam) {
				return `<div class="cam missing${ref}" data-cam="${id}">
					<div class="id">${id}</div>
					<div class="thumb empty-thumb"></div>
					<div class="info">—</div>
				</div>`;
			}
			return `<div class="cam ${camClass(cam)}${ref}" data-cam="${id}">
				<div class="id">${id}</div>
				<div class="thumb"><img alt="${id}" /></div>
				<div class="info">${cam.sensor}<br>${cam.format}</div>
			</div>`;
		}).join("");
		return `<div class="camera-row">${cells}</div>`;
	}).join("");

	loadThumbnails(summary);
}

async function loadThumbnails(summary) {
	const gen = ++thumbGen;
	const path = summary.path;
	const cameras = summary.cameras.map((c) => c.id);

	for (const id of cameras) {
		const cell = document.querySelector(`[data-cam="${id}"] img`);
		if (cell) cell.classList.add("loading");
	}

	try {
		const thumbs = await invoke("camera_thumbnails_batch", { path, cameras, jobs: null });
		if (gen !== thumbGen) return;
		for (const [id, dataUrl] of Object.entries(thumbs)) {
			const cell = document.querySelector(`[data-cam="${id}"] img`);
			if (!cell) continue;
			cell.src = dataUrl;
			cell.classList.remove("loading");
		}
	} catch (e) {
		for (const id of cameras) {
			const cell = document.querySelector(`[data-cam="${id}"] img`);
			if (!cell) continue;
			cell.classList.remove("loading");
			cell.alt = "err";
		}
	}
}

function renderFileList() {
	const list = $("#file-list");
	list.innerHTML = files.map((f, i) => `
		<li>
			<button type="button" data-idx="${i}" class="${current === f.path ? "active" : ""}">
				${f.name}
				<span class="sub">${f.image_count} modules · ${f.firmware ?? "?"}</span>
			</button>
		</li>
	`).join("");

	list.querySelectorAll("button").forEach((btn) => {
		btn.addEventListener("click", async () => {
			await selectFile(files[Number(btn.dataset.idx)].path);
		});
	});
}

async function selectFile(path) {
	const summary = await invoke("inspect_lri", { path });
	current = path;
	files = files.map((f) => (f.path === path ? summary : f));
	showDetail(true);
	renderMeta(summary);
	renderCameras(summary);
	renderFileList();
}

async function loadDirectory(path) {
	const scan = await invoke("scan_directory", { path });
	files = scan.files;
	current = files[0]?.path ?? null;
	renderFileList();
	if (current) {
		await selectFile(current);
	} else {
		showDetail(false);
	}
}

async function openFile() {
	const path = await invoke("pick_lri_file");
	if (!path) return;
	await selectFile(path);
	files = [await invoke("inspect_lri", { path })];
	renderFileList();
}

async function openDir() {
	const path = await invoke("pick_directory");
	if (!path) return;
	await loadDirectory(path);
}

function setProgress(done, total, camera) {
	const wrap = $("#export-progress-wrap");
	const bar = $("#progress-bar");
	const label = $("#progress-label");
	wrap.classList.remove("hidden");
	const pct = total > 0 ? Math.round((done / total) * 100) : 0;
	bar.style.width = `${pct}%`;
	label.textContent = `${done}/${total} · ${camera}`;
}

async function exportDngs() {
	if (!current) return;
	const output = await invoke("pick_output_dir");
	if (!output) return;

	const status = $("#export-status");
	const wrap = $("#export-progress-wrap");
	status.className = "status";
	status.textContent = "";
	wrap.classList.remove("hidden");
	setProgress(0, 1, "…");

	const unlisten = await listen("export-progress", (event) => {
		const { done, total, camera } = event.payload;
		setProgress(done, total, camera);
	});

	try {
		const count = await invoke("extract_lri", {
			input: current,
			output,
			jobs: null,
		});
		status.textContent = `Done — ${count} DNGs → ${output}`;
		status.classList.add("ok");
		setProgress(count, count, "done");
	} catch (e) {
		status.textContent = String(e);
		status.classList.add("err");
	} finally {
		unlisten();
	}
}

function pickLriFromPaths(paths) {
	if (!paths?.length) return null;
	const hit = paths.find((p) => p.toLowerCase().endsWith(".lri"));
	return hit ?? null;
}

async function setupDragDrop() {
	const overlay = $("#drop-overlay");
	const win = getCurrentWindow();

	await win.onDragDropEvent(async (event) => {
		const { type, paths } = event.payload;
		if (type === "over" || type === "enter") {
			overlay.classList.remove("hidden");
		} else if (type === "leave") {
			overlay.classList.add("hidden");
		} else if (type === "drop") {
			overlay.classList.add("hidden");
			const lri = pickLriFromPaths(paths);
			if (lri) {
				await selectFile(lri);
				files = [await invoke("inspect_lri", { path: lri })];
				renderFileList();
			}
		}
	});
}

$("#btn-open-file").addEventListener("click", () => openFile().catch(console.error));
$("#btn-open-dir").addEventListener("click", () => openDir().catch(console.error));
$("#btn-export").addEventListener("click", () => exportDngs().catch(console.error));

setupDragDrop().catch(console.error);