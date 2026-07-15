const invoke = window.__TAURI__.core.invoke;
const Channel = window.__TAURI__.core.Channel;
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
let fuseBusy = false;
let lastFusePreview = null;
let fuseDropActive = false;

const FUSE_STAGES = {
	prepare: "Preparing",
	depth: "Depth sweep",
	warp: "Warping modules",
	blend: "Blending",
	export: "Exporting",
};

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

function fuseMode() {
	const checked = document.querySelector('input[name="fuse-mode"]:checked');
	return checked?.value === "full-res" ? "full-res" : "preview";
}

function updateFuseOptions() {
	const fullRes = fuseMode() === "full-res";
	const opts = $("#fuse-export-options");
	opts.classList.toggle("disabled", !fullRes);
	$("#btn-fuse-export").classList.toggle("hidden", !fullRes);
}

function setFuseProgress(stage, done, total) {
	const wrap = $("#fuse-progress-wrap");
	const bar = $("#fuse-progress-bar");
	const label = $("#fuse-progress-label");
	wrap.classList.remove("hidden");
	const pct = total > 0 ? Math.round((done / total) * 100) : 0;
	bar.style.width = `${pct}%`;
	const name = FUSE_STAGES[stage] ?? stage;
	label.textContent = total > 1 ? `${name} · ${done}/${total}` : name;
}

function classifyDropPaths(paths) {
	const lris = [];
	const dirs = [];
	for (const p of paths ?? []) {
		if (p.toLowerCase().endsWith(".lri")) lris.push(p);
		else dirs.push(p);
	}
	return { lris, dirs };
}

async function hitFuseZone(position) {
	const zone = $("#fuse-drop-zone");
	if (!zone || zone.offsetParent === null || !position) return false;
	const rect = zone.getBoundingClientRect();
	const scale = await getCurrentWindow().scaleFactor();
	const x = position.x / scale;
	const y = position.y / scale;
	return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
}

function setFuseDropActive(active) {
	fuseDropActive = active;
	$("#fuse-drop-zone")?.classList.toggle("active", active);
	const text = $("#drop-overlay-text");
	if (!text) return;
	text.textContent = active
		? "Drop to fuse & export"
		: "Drop .lri file";
}

async function startFileDrag(filePath, iconDataUrl) {
	const onEvent = new Channel();
	await invoke("plugin:drag|start_drag", {
		item: [filePath],
		image: iconDataUrl,
		options: { mode: "Copy" },
		onEvent,
	});
}

function renderFuseExports(exportPaths, previewDataUrl) {
	const wrap = $("#fuse-exports");
	const chips = $("#fuse-export-chips");
	if (!exportPaths?.length) {
		wrap.classList.add("hidden");
		chips.innerHTML = "";
		return;
	}

	wrap.classList.remove("hidden");
	chips.innerHTML = exportPaths.map((path) => {
		const name = path.split(/[/\\]/).pop();
		return `<button type="button" class="export-chip" data-path="${path}">
			<span class="chip-icon">⇱</span>${name}
		</button>`;
	}).join("");

	chips.querySelectorAll(".export-chip").forEach((chip) => {
		chip.addEventListener("mousedown", (e) => {
			e.preventDefault();
			startFileDrag(chip.dataset.path, previewDataUrl).catch(console.error);
		});
	});
}

function bindFusePreviewDrag(previewDataUrl, exportPaths) {
	const img = $("#fuse-preview");
	if (!img) return;
	const primary = exportPaths?.[0];
	img.onmousedown = (e) => {
		if (!primary) return;
		e.preventDefault();
		startFileDrag(primary, previewDataUrl).catch(console.error);
	};
}

function renderFuseStats(summary, outputDir) {
	const ncc = summary.depth_ncc_vs_lumen != null
		? summary.depth_ncc_vs_lumen.toFixed(4)
		: "—";
	const depth = `${summary.depth_plane_mm.toFixed(0)} mm`;
	const modules = String(summary.modules_warped);
	const size = summary.full_res
		? `${summary.canvas[0]}×${summary.canvas[1]}`
		: `${summary.preview_max_side}px preview`;

	$("#fuse-stats").innerHTML = [
		["Depth plane", depth],
		["Modules", modules],
		["NCC vs Lumen", ncc],
		["Output", size],
		["Folder", outputDir],
	].map(([label, value]) => `
		<div class="fuse-stat">
			<div class="label">${label}</div>
			<div class="value">${value}</div>
		</div>
	`).join("");
}

async function runFuse(outputDir) {
	if (!current || fuseBusy) return;

	const fullRes = fuseMode() === "full-res";
	const status = $("#fuse-status");
	const wrap = $("#fuse-progress-wrap");
	const result = $("#fuse-result");

	status.className = "status";
	status.textContent = "";
	wrap.classList.remove("hidden");
	setFuseProgress("prepare", 0, 1);
	result.classList.add("hidden");
	fuseBusy = true;
	$("#btn-fuse").disabled = true;
	$("#btn-fuse-export").disabled = true;

	const unlisten = await listen("fuse-progress", (event) => {
		const { stage, done, total } = event.payload;
		setFuseProgress(stage, done, total);
	});

	try {
		const res = await invoke("fuse_lri", {
			input: current,
			output: outputDir,
			maxSide: 1024,
			fullRes,
			exportTiff: $("#opt-tiff").checked,
			exportDng: $("#opt-dng").checked,
			lumenJpg: null,
		});

		lastFusePreview = res.preview_data_url;
		$("#fuse-preview").src = res.preview_data_url;
		renderFuseStats(res.summary, res.output_dir);
		renderFuseExports(res.export_paths, res.preview_data_url);
		bindFusePreviewDrag(res.preview_data_url, res.export_paths);
		result.classList.remove("hidden");

		const exports = res.summary.exports.join(", ");
		status.textContent = fullRes
			? `Done — ${exports}`
			: `Preview ready (temp: ${res.output_dir})`;
		status.classList.add("ok");
		setFuseProgress("export", 1, 1);
	} catch (e) {
		status.textContent = String(e);
		status.classList.add("err");
	} finally {
		fuseBusy = false;
		$("#btn-fuse").disabled = false;
		$("#btn-fuse-export").disabled = false;
		unlisten();
	}
}

async function fusePreview() {
	if (fuseMode() === "full-res") {
		const output = await invoke("pick_output_dir");
		if (!output) return;
		await runFuse(output);
		return;
	}
	await runFuse(null);
}

async function fuseToFolder() {
	const output = await invoke("pick_output_dir");
	if (!output) return;
	await runFuse(output);
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

async function handleFuseDrop(paths) {
	const { lris, dirs } = classifyDropPaths(paths);
	const outputDir = dirs[0] ?? null;
	let lri = lris[0] ?? null;

	if (!lri && outputDir) {
		const scan = await invoke("scan_directory", { path: outputDir });
		lri = scan.files[0]?.path ?? null;
	}

	if (!lri && current && outputDir) {
		await runFuse(outputDir);
		return;
	}

	if (!lri) return;

	await selectFile(lri);
	if (!files.some((f) => f.path === lri)) {
		files = [await invoke("inspect_lri", { path: lri })];
		renderFileList();
	}

	if (outputDir) {
		await runFuse(outputDir);
		return;
	}

	if (fuseMode() === "full-res") {
		const picked = await invoke("pick_output_dir");
		if (picked) await runFuse(picked);
		return;
	}

	await runFuse(null);
}

async function setupDragDrop() {
	const overlay = $("#drop-overlay");
	const win = getCurrentWindow();

	await win.onDragDropEvent(async (event) => {
		const { type, paths, position } = event.payload;
		if (type === "over" || type === "enter") {
			const onFuse = await hitFuseZone(position);
			setFuseDropActive(onFuse);
			overlay.classList.remove("hidden");
		} else if (type === "leave") {
			setFuseDropActive(false);
			overlay.classList.add("hidden");
		} else if (type === "drop") {
			setFuseDropActive(false);
			overlay.classList.add("hidden");
			const onFuse = await hitFuseZone(position);
			if (onFuse) {
				await handleFuseDrop(paths);
				return;
			}

			const lri = pickLriFromPaths(paths);
			if (lri) {
				await selectFile(lri);
				files = [await invoke("inspect_lri", { path: lri })];
				renderFileList();
				return;
			}

			const { dirs } = classifyDropPaths(paths);
			if (dirs[0]) {
				await loadDirectory(dirs[0]);
			}
		}
	});
}

document.querySelectorAll('input[name="fuse-mode"]').forEach((el) => {
	el.addEventListener("change", updateFuseOptions);
});

$("#btn-open-file").addEventListener("click", () => openFile().catch(console.error));
$("#btn-open-dir").addEventListener("click", () => openDir().catch(console.error));
$("#btn-export").addEventListener("click", () => exportDngs().catch(console.error));
$("#btn-fuse").addEventListener("click", () => fusePreview().catch(console.error));
$("#btn-fuse-export").addEventListener("click", () => fuseToFolder().catch(console.error));

updateFuseOptions();
setupDragDrop().catch(console.error);