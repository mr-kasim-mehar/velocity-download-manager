import "./styles.css";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { AppSettings, DownloadItem, SourceType } from "./types";

type Filter = "all" | "active" | "completed" | "failed";
let downloads: DownloadItem[] = [];
let settings: AppSettings = { default_destination: "" };
let filter: Filter = "all";
let query = "";
let selectedType: SourceType | "auto" = "auto";

const icons: Record<string, string> = {
  download: '<svg viewBox="0 0 24 24"><path d="M12 3v12m0 0 5-5m-5 5-5-5M5 21h14"/></svg>',
  activity: '<svg viewBox="0 0 24 24"><path d="M4 12h3l2-7 4 14 2-7h5"/></svg>',
  check: '<svg viewBox="0 0 24 24"><path d="m5 12 4 4L19 6"/></svg>',
  warning: '<svg viewBox="0 0 24 24"><path d="M12 3 2.8 20h18.4L12 3Zm0 6v5m0 3v.01"/></svg>',
  search: '<svg viewBox="0 0 24 24"><circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/></svg>',
  plus: '<svg viewBox="0 0 24 24"><path d="M12 5v14M5 12h14"/></svg>',
  torrent: '<svg viewBox="0 0 24 24"><path d="M5 5h14v14H5zM9 9h6v6H9z"/></svg>',
  settings: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H3v-4h.1a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1a1.7 1.7 0 0 0 1.9.3A1.7 1.7 0 0 0 10 3V3h4v.1a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z"/></svg>',
  folder: '<svg viewBox="0 0 24 24"><path d="M3 6h7l2 2h9v11H3z"/></svg>',
  play: '<svg viewBox="0 0 24 24"><path d="m9 6 9 6-9 6Z"/></svg>',
  pause: '<svg viewBox="0 0 24 24"><path d="M9 5v14m6-14v14"/></svg>',
  external: '<svg viewBox="0 0 24 24"><path d="M14 5h5v5m0-5-9 9M19 14v5H5V5h5"/></svg>',
  close: '<svg viewBox="0 0 24 24"><path d="m6 6 12 12M18 6 6 18"/></svg>',
  link: '<svg viewBox="0 0 24 24"><path d="M10 13a5 5 0 0 0 7 0l2-2a5 5 0 0 0-7-7l-1 1M14 11a5 5 0 0 0-7 0l-2 2a5 5 0 0 0 7 7l1-1"/></svg>',
  video: '<svg viewBox="0 0 24 24"><rect x="3" y="5" width="18" height="14" rx="2"/><path d="m10 9 5 3-5 3Z"/></svg>',
  trash: '<svg viewBox="0 0 24 24"><path d="M5 7h14M9 7V4h6v3m2 0-1 13H8L7 7"/></svg>'
};
const icon = (name: string) => `<span class="icon">${icons[name]}</span>`;
const app = document.querySelector<HTMLDivElement>("#app")!;

app.innerHTML = `
<div class="app-shell">
  <aside class="sidebar">
    <div class="identity"><img src="/velocity-mark.png" alt=""><div><strong>Velocity</strong><span>Download Manager</span></div></div>
    <nav aria-label="Download views">
      <p class="nav-label">Library</p>
      <button class="nav-row active" data-filter="all">${icon("download")}<span>All downloads</span><b id="count-all">0</b></button>
      <button class="nav-row" data-filter="active">${icon("activity")}<span>Active</span><b id="count-active">0</b></button>
      <button class="nav-row" data-filter="completed">${icon("check")}<span>Completed</span><b id="count-completed">0</b></button>
      <button class="nav-row" data-filter="failed">${icon("warning")}<span>Needs attention</span><b id="count-failed">0</b></button>
    </nav>
    <div class="sidebar-footer">
      <button class="nav-row" id="open-settings">${icon("settings")}<span>Settings</span></button>
      <div class="engine-state"><i></i><span>Download engines ready</span></div>
    </div>
  </aside>
  <main>
    <header class="topbar">
      <div><h1>Downloads</h1><p id="page-subtitle">Manage files, media and torrents</p></div>
      <div class="top-actions">
        <label class="search-field">${icon("search")}<input id="search" type="search" placeholder="Search" aria-label="Search downloads"></label>
        <button class="button secondary" id="import-torrent">${icon("torrent")} Import torrent</button>
        <button class="button primary" id="add-source">${icon("plus")} Add download</button>
      </div>
    </header>
    <section class="summary-bar">
      <div class="transfer-summary"><i id="activity-dot"></i><strong id="active-summary">Idle</strong><span id="overall-speed">0 B/s</span></div>
      <div class="summary-actions"><button id="pause-all">Pause all</button><button id="clear-completed">Clear completed</button></div>
    </section>
    <section id="download-list" class="download-list" aria-live="polite"></section>
  </main>
</div>

<div class="overlay hidden" id="source-overlay">
  <form class="dialog" id="source-form">
    <div class="dialog-header"><div><h2>Add download</h2><p>Add a direct file, supported media page, or magnet link.</p></div><button class="icon-button close-dialog" type="button">${icon("close")}</button></div>
    <div class="source-tabs" role="tablist">
      <button type="button" class="source-tab active" data-kind="auto">Auto detect</button>
      <button type="button" class="source-tab" data-kind="direct">Direct file</button>
      <button type="button" class="source-tab" data-kind="media">Media</button>
      <button type="button" class="source-tab" data-kind="torrent">Magnet</button>
    </div>
    <label class="field-label" for="source-input">URL or magnet link</label>
    <div class="input-with-icon">${icon("link")}<input id="source-input" required spellcheck="false" placeholder="https://example.com/file.zip"></div>
    <section class="media-options hidden" id="media-options">
      <div class="option-grid">
        <label><span>Output</span><select id="media-mode"><option value="video">Video + audio</option><option value="audio">Audio only</option></select></label>
        <label id="resolution-field"><span>Maximum resolution</span><select id="media-resolution"><option value="best">Best available</option><option value="2160p">2160p (4K)</option><option value="1440p">1440p (2K)</option><option value="1080p">1080p (Full HD)</option><option value="720p">720p (HD)</option><option value="480p">480p</option><option value="360p">360p</option></select></label>
        <label class="hidden" id="audio-format-field"><span>Audio format</span><select id="audio-format"><option value="mp3">MP3</option><option value="m4a">M4A</option><option value="aac">AAC</option><option value="opus">Opus</option><option value="flac">FLAC (lossless)</option><option value="wav">WAV (uncompressed)</option></select></label>
      </div>
      <p id="media-option-note">Velocity downloads the best video up to this resolution and merges the best audio.</p>
    </section>
    <label class="field-label" for="destination-input">Save to</label>
    <div class="folder-input"><div class="input-with-icon">${icon("folder")}<input id="destination-input" readonly></div><button type="button" class="button secondary" id="browse-destination">Browse</button></div>
    <label class="field-label" for="filename-input">File name <span>Optional for direct downloads</span></label>
    <input class="text-input" id="filename-input" placeholder="Detect automatically">
    <p class="form-note" id="source-note">Velocity will choose the correct download engine.</p>
    <p class="form-error" id="source-error"></p>
    <div class="dialog-actions"><button type="button" class="button secondary close-dialog">Cancel</button><button type="submit" class="button primary">Start download</button></div>
  </form>
</div>

<div class="overlay hidden" id="settings-overlay">
  <form class="dialog settings-dialog" id="settings-form">
    <div class="dialog-header"><div><h2>Settings</h2><p>General download preferences</p></div><button class="icon-button close-settings" type="button">${icon("close")}</button></div>
    <section class="setting-section"><h3>Download location</h3><p>New downloads are saved here unless you choose another folder for an individual job.</p><div class="folder-input"><div class="input-with-icon">${icon("folder")}<input id="default-destination" readonly></div><button type="button" class="button secondary" id="browse-default">Change</button></div></section>
    <section class="setting-section"><h3>Browser integration</h3><p>Add an IDM-style Download this video button to Chrome, Edge, Opera GX, Brave, and Firefox.</p><div class="setting-row"><div><strong>Velocity Download Companion</strong><span>Open the packaged extension and follow its installation guide.</span></div><button type="button" class="button secondary" id="open-browser-extension">Open package</button></div></section>
    <section class="setting-section"><h3>Resource usage</h3><div class="setting-row"><div><strong>Memory-efficient transfers</strong><span>Files stream directly to disk in bounded chunks.</span></div><span class="status-pill good">Enabled</span></div><div class="setting-row"><div><strong>Local processing</strong><span>History and URLs remain on this device.</span></div><span class="status-pill good">Enabled</span></div></section>
    <section class="setting-section about-section"><h3>About Velocity</h3><div class="developer-card"><div class="developer-avatar">MQ</div><div class="developer-details"><strong>Muhammad Qasim</strong><span>Security researcher · Application developer · Ethical hacker</span><p>Velocity is free to use. Built with a focus on privacy, performance, and accessible downloading.</p></div><button type="button" class="button secondary" id="open-developer-instagram">Instagram ${icon("external")}</button></div></section>
    <p class="form-error" id="settings-error"></p>
    <div class="dialog-actions"><button type="button" class="button secondary close-settings">Cancel</button><button type="submit" class="button primary">Save changes</button></div>
  </form>
</div>
<div class="toast" id="toast"></div>`;

const el = <T extends Element>(selector: string) => document.querySelector<T>(selector)!;
const escapeHtml = (value: string) => { const node = document.createElement("div"); node.textContent = value; return node.innerHTML; };

function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
  return `${(value / 1024 ** index).toFixed(index ? 1 : 0)} ${units[index]}`;
}
function sourceLabel(type: SourceType): string { return type === "media" ? "Media" : type === "torrent" ? "Torrent" : "Direct"; }
function sourceIcon(type: SourceType): string { return type === "media" ? "video" : type === "torrent" ? "torrent" : "download"; }
function mediaLabel(item: DownloadItem): string {
  if (item.source_type !== "media") return sourceLabel(item.source_type);
  return item.media_mode === "audio" ? `Audio · ${item.audio_format}` : `Video · ${item.media_resolution === "best" ? "Best" : item.media_resolution}`;
}
function joinPath(folder: string, filename: string): string { return `${folder}${folder.endsWith("/") || folder.endsWith("\\") ? "" : "/"}${filename}`; }

function detail(item: DownloadItem): string {
  if (item.status === "resolving") return item.error || (item.source_type === "torrent" ? "Fetching torrent metadata" : "Resolving media formats");
  if (item.status === "downloading") {
    const progress = item.total
      ? `${formatBytes(item.downloaded)} of ${formatBytes(item.total)}`
      : `${formatBytes(item.downloaded)}${item.progress !== null ? ` · ${item.progress.toFixed(1)}%` : ""}`;
    const etaSeconds = item.total && item.speed > 0 ? (item.total - item.downloaded) / item.speed : 0;
    const eta = etaSeconds > 0 ? ` · ${etaSeconds < 60 ? Math.ceil(etaSeconds) + " sec" : Math.ceil(etaSeconds / 60) + " min"} remaining` : "";
    const peers = item.source_type === "torrent" && item.peers !== null ? ` · ${item.peers} peers` : "";
    const warning = item.source_type === "torrent" && item.speed === 0 && item.error ? ` · ${item.error}` : "";
    return `${progress} · ${formatBytes(item.speed)}/s${peers}${eta}${warning}`;
  }
  if (item.status === "completed") return `${formatBytes(item.downloaded)} · Saved to ${item.destination}`;
  if (item.status === "failed") return item.error || "Download failed";
  if (item.status === "paused") return `${formatBytes(item.downloaded)} · Paused`;
  return "Waiting to start";
}

function filteredDownloads(): DownloadItem[] {
  return downloads.filter((item) => {
    const stateMatches = filter === "all" || (filter === "active" && ["queued", "resolving", "downloading", "paused"].includes(item.status)) || item.status === filter;
    return stateMatches && `${item.filename} ${item.url}`.toLowerCase().includes(query);
  });
}

function render(): void {
  const active = downloads.filter((item) => ["queued", "resolving", "downloading", "paused"].includes(item.status));
  const transferring = downloads.filter((item) => item.status === "downloading");
  const completed = downloads.filter((item) => item.status === "completed");
  const failed = downloads.filter((item) => item.status === "failed");
  el("#count-all").textContent = String(downloads.length);
  el("#count-active").textContent = String(active.length);
  el("#count-completed").textContent = String(completed.length);
  el("#count-failed").textContent = String(failed.length);
  el("#active-summary").textContent = transferring.length ? `${transferring.length} transferring` : "Idle";
  el("#overall-speed").textContent = `${formatBytes(transferring.reduce((total, item) => total + item.speed, 0))}/s`;
  el("#activity-dot").classList.toggle("online", transferring.length > 0);
  el<HTMLButtonElement>("#clear-completed").disabled = completed.length === 0;
  el<HTMLButtonElement>("#pause-all").disabled = transferring.length === 0;
  const list = el("#download-list");
  const visible = filteredDownloads();
  if (!visible.length) {
    list.innerHTML = `<div class="empty-state"><div class="empty-symbol">${icon(filter === "failed" ? "check" : "download")}</div><h2>${downloads.length ? "No downloads in this view" : "No downloads yet"}</h2><p>${downloads.length ? "Choose another view or adjust your search." : "Add a file, media page, magnet link, or torrent to get started."}</p>${downloads.length ? "" : `<button class="button primary" id="empty-add">${icon("plus")} Add download</button>`}</div>`;
    document.querySelector("#empty-add")?.addEventListener("click", () => openSourceDialog());
    return;
  }
  list.innerHTML = visible.map((item) => {
    const progress = Math.min(100, Math.max(0, item.progress ?? (item.total ? item.downloaded / item.total * 100 : item.status === "completed" ? 100 : 0)));
    const canResume = ["paused", "failed", "cancelled"].includes(item.status);
    const canPause = ["queued", "resolving", "downloading"].includes(item.status);
    return `<article class="download-row"><div class="source-symbol ${item.source_type}">${icon(sourceIcon(item.source_type))}</div><div class="download-content"><div class="download-heading"><strong title="${escapeHtml(item.filename)}">${escapeHtml(item.filename)}</strong><span class="type-label">${escapeHtml(mediaLabel(item))}</span></div><p class="download-detail ${item.status === "failed" ? "error" : ""}">${escapeHtml(detail(item))}</p><div class="progress-track ${item.total || item.progress !== null ? "" : "indeterminate"}"><span style="width:${progress}%"></span></div></div><span class="job-status ${item.status}">${item.status}</span><div class="row-actions">${canPause ? `<button data-action="pause" data-id="${item.id}" title="Pause">${icon("pause")}</button>` : ""}${canResume ? `<button data-action="resume" data-id="${item.id}" title="Resume or retry">${icon("play")}</button>` : ""}${item.status === "completed" ? `<button data-action="open" data-id="${item.id}" title="Open">${icon("external")}</button>` : ""}<button data-action="reveal" data-id="${item.id}" title="Show in folder">${icon("folder")}</button><button data-action="remove" data-id="${item.id}" title="Remove from history">${icon("trash")}</button></div></article>`;
  }).join("");
}

function openSourceDialog(kind: SourceType | "auto" = "auto", preset = ""): void {
  selectedType = kind;
  document.querySelectorAll(".source-tab").forEach((tab) => tab.classList.toggle("active", (tab as HTMLElement).dataset.kind === kind));
  el<HTMLInputElement>("#source-input").value = preset;
  el<HTMLInputElement>("#destination-input").value = settings.default_destination;
  updateSourceUi();
  el("#source-error").textContent = "";
  el("#source-overlay").classList.remove("hidden");
  setTimeout(() => el<HTMLInputElement>("#source-input").focus(), 50);
}
function updateSourceUi(): void {
  const input = el<HTMLInputElement>("#source-input");
  input.placeholder = selectedType === "torrent" ? "magnet:?xt=urn:btih:…" : selectedType === "media" ? "https://www.youtube.com/watch?v=…" : "https://example.com/file.zip";
  const torrent = selectedType === "torrent" || (selectedType === "auto" && input.value.trim().toLowerCase().startsWith("magnet:?"));
  const note = el("#source-note");
  note.classList.toggle("network-note", torrent);
  note.textContent = torrent
    ? "Windows may ask to allow aria2c network access. BitTorrent needs this for incoming peers; allow Private networks only."
    : "Velocity will choose the correct download engine.";
  el("#media-options").classList.toggle("hidden", selectedType !== "media");
  updateMediaOptionUi();
}
function updateMediaOptionUi(): void {
  const audioOnly = el<HTMLSelectElement>("#media-mode").value === "audio";
  el("#resolution-field").classList.toggle("hidden", audioOnly);
  el("#audio-format-field").classList.toggle("hidden", !audioOnly);
  el("#media-option-note").textContent = audioOnly
    ? "Audio is extracted locally with FFmpeg. MP3 is the most widely compatible choice."
    : "Velocity downloads the best video up to this resolution and merges the best audio.";
}
function closeSourceDialog(): void { el("#source-overlay").classList.add("hidden"); el<HTMLFormElement>("#source-form").reset(); }
function openSettings(): void { el<HTMLInputElement>("#default-destination").value = settings.default_destination; el("#settings-error").textContent = ""; el("#settings-overlay").classList.remove("hidden"); }
function closeSettings(): void { el("#settings-overlay").classList.add("hidden"); }
function toast(message: string): void { const node = el("#toast"); node.textContent = message; node.classList.add("show"); setTimeout(() => node.classList.remove("show"), 2600); }
async function chooseFolder(target: HTMLInputElement): Promise<void> { const folder = await openDialog({ directory: true, multiple: false, title: "Choose download folder", defaultPath: target.value || settings.default_destination }); if (typeof folder === "string") target.value = folder; }
async function importTorrent(): Promise<void> { const file = await openDialog({ multiple: false, directory: false, title: "Import torrent", filters: [{ name: "BitTorrent metadata", extensions: ["torrent"] }] }); if (typeof file === "string") openSourceDialog("torrent", file); }
async function refresh(): Promise<void> { downloads = await invoke<DownloadItem[]>("list_downloads"); render(); }

el("#add-source").addEventListener("click", () => openSourceDialog());
el("#import-torrent").addEventListener("click", () => void importTorrent());
el("#open-settings").addEventListener("click", openSettings);
document.querySelectorAll(".close-dialog").forEach((button) => button.addEventListener("click", closeSourceDialog));
document.querySelectorAll(".close-settings").forEach((button) => button.addEventListener("click", closeSettings));
el("#source-overlay").addEventListener("click", (event) => { if (event.target === el("#source-overlay")) closeSourceDialog(); });
el("#settings-overlay").addEventListener("click", (event) => { if (event.target === el("#settings-overlay")) closeSettings(); });
document.querySelectorAll<HTMLButtonElement>(".source-tab").forEach((tab) => tab.addEventListener("click", () => { selectedType = tab.dataset.kind as SourceType | "auto"; document.querySelectorAll(".source-tab").forEach((other) => other.classList.toggle("active", other === tab)); updateSourceUi(); }));
el<HTMLInputElement>("#source-input").addEventListener("input", updateSourceUi);
el<HTMLSelectElement>("#media-mode").addEventListener("change", updateMediaOptionUi);
el("#browse-destination").addEventListener("click", () => void chooseFolder(el<HTMLInputElement>("#destination-input")));
el("#browse-default").addEventListener("click", () => void chooseFolder(el<HTMLInputElement>("#default-destination")));
el("#open-browser-extension").addEventListener("click", async () => {
  try {
    const path = await invoke<string>("browser_extension_package");
    await revealItemInDir(path);
    toast("Browser extension package opened");
  } catch (error) { toast(String(error)); }
});
el("#open-developer-instagram").addEventListener("click", async () => {
  try { await openUrl("https://www.instagram.com/m_kasim2/"); }
  catch (error) { toast(`Could not open Instagram: ${String(error)}`); }
});

el<HTMLFormElement>("#source-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  try {
    await invoke("add_source", { request: { source: el<HTMLInputElement>("#source-input").value.trim(), source_type: selectedType, destination: el<HTMLInputElement>("#destination-input").value || undefined, filename: el<HTMLInputElement>("#filename-input").value.trim() || undefined, media_mode: el<HTMLSelectElement>("#media-mode").value, media_resolution: el<HTMLSelectElement>("#media-resolution").value, audio_format: el<HTMLSelectElement>("#audio-format").value }});
    closeSourceDialog(); toast("Download added"); await refresh();
  } catch (error) { el("#source-error").textContent = String(error); }
});
el<HTMLFormElement>("#settings-form").addEventListener("submit", async (event) => { event.preventDefault(); try { settings = await invoke<AppSettings>("set_default_destination", { destination: el<HTMLInputElement>("#default-destination").value }); closeSettings(); toast("Settings saved"); } catch (error) { el("#settings-error").textContent = String(error); } });
el("#download-list").addEventListener("click", async (event) => {
  const button = (event.target as HTMLElement).closest<HTMLButtonElement>("button[data-action]"); if (!button) return;
  const id = button.dataset.id!; const item = downloads.find((download) => download.id === id);
  try {
    if (button.dataset.action === "pause") await invoke("pause_download", { id });
    if (button.dataset.action === "resume") await invoke("resume_download", { id });
    if (button.dataset.action === "remove") await invoke("remove_download", { id, deleteFile: false });
    if (button.dataset.action === "open" && item) await openPath(joinPath(item.destination, item.filename));
    if (button.dataset.action === "reveal" && item) {
      if (item.source_type === "torrent") await openPath(item.destination);
      else await revealItemInDir(joinPath(item.destination, item.filename));
    }
    await refresh();
  } catch (error) { toast(String(error)); }
});
document.querySelectorAll<HTMLButtonElement>(".nav-row[data-filter]").forEach((button) => button.addEventListener("click", () => { filter = button.dataset.filter as Filter; document.querySelectorAll(".nav-row[data-filter]").forEach((item) => item.classList.toggle("active", item === button)); render(); }));
el<HTMLInputElement>("#search").addEventListener("input", (event) => { query = (event.target as HTMLInputElement).value.trim().toLowerCase(); render(); });
el("#pause-all").addEventListener("click", async () => { await invoke("pause_all"); await refresh(); });
el("#clear-completed").addEventListener("click", async () => { await invoke("clear_completed"); await refresh(); });
document.addEventListener("keydown", (event) => { if (event.key === "Escape") { closeSourceDialog(); closeSettings(); } if (event.ctrlKey && event.key.toLowerCase() === "n") { event.preventDefault(); openSourceDialog(); } });

async function initialize(): Promise<void> {
  settings = await invoke<AppSettings>("get_settings");
  await listen<string>("browser-handoff", ({ payload }) => { toast(payload); void refresh(); });
  await listen<DownloadItem>("download-progress", ({ payload }) => { const index = downloads.findIndex((item) => item.id === payload.id); if (index >= 0) downloads[index] = payload; else downloads.unshift(payload); render(); });
  await refresh();
}
void initialize().catch((error) => toast(`Startup error: ${String(error)}`));
