use futures_util::StreamExt;
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, ETAG, IF_RANGE, LAST_MODIFIED, RANGE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    net::TcpListener,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_deep_link::DeepLinkExt;
use tokio::{
    fs,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::Notify,
};
use url::Url;

#[derive(Clone, Serialize, Deserialize)]
struct DownloadItem {
    id: String,
    url: String,
    filename: String,
    destination: String,
    downloaded: u64,
    total: Option<u64>,
    speed: f64,
    #[serde(default)]
    progress: Option<f64>,
    #[serde(default)]
    peers: Option<u64>,
    #[serde(default)]
    connections: Option<u8>,
    status: String,
    #[serde(default = "default_source_type")]
    source_type: String,
    #[serde(default = "default_media_mode")]
    media_mode: String,
    #[serde(default = "default_media_resolution")]
    media_resolution: String,
    #[serde(default = "default_audio_format")]
    audio_format: String,
    error: Option<String>,
    created_at: u64,
}

fn default_source_type() -> String {
    "direct".into()
}

fn default_media_mode() -> String {
    "video".into()
}
fn default_media_resolution() -> String {
    "best".into()
}
fn default_audio_format() -> String {
    "mp3".into()
}

#[derive(Clone, Serialize, Deserialize)]
struct AppSettings {
    default_destination: String,
}

#[derive(Deserialize)]
struct DownloadRequest {
    url: String,
    destination: Option<String>,
    filename: Option<String>,
}

#[derive(Deserialize)]
struct SourceRequest {
    source: String,
    source_type: Option<String>,
    destination: Option<String>,
    filename: Option<String>,
    media_mode: Option<String>,
    media_resolution: Option<String>,
    audio_format: Option<String>,
}

struct DownloadManager {
    items: Mutex<HashMap<String, DownloadItem>>,
    controls: Mutex<HashMap<String, JobControl>>,
    state_file: PathBuf,
    settings: Mutex<AppSettings>,
    settings_file: PathBuf,
    recent_browser_links: Mutex<HashMap<String, std::time::Instant>>,
}

#[derive(Clone)]
struct JobControl {
    stop: Arc<AtomicBool>,
    done: Arc<Notify>,
}

impl DownloadManager {
    fn load(state_file: PathBuf, settings_file: PathBuf) -> Self {
        let items = std::fs::read_to_string(&state_file)
            .ok()
            .and_then(|data| serde_json::from_str::<Vec<DownloadItem>>(&data).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|mut item| {
                if matches!(item.status.as_str(), "downloading" | "queued" | "resolving") {
                    item.status = "paused".into();
                    item.speed = 0.0;
                }
                (item.id.clone(), item)
            })
            .collect();
        let settings = std::fs::read_to_string(&settings_file)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_else(|| AppSettings {
                default_destination: dirs::download_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .to_string_lossy()
                    .into_owned(),
            });
        Self {
            items: Mutex::new(items),
            controls: Mutex::new(HashMap::new()),
            state_file,
            settings: Mutex::new(settings),
            settings_file,
            recent_browser_links: Mutex::new(HashMap::new()),
        }
    }

    fn save(&self) {
        let mut values: Vec<_> = self.items.lock().unwrap().values().cloned().collect();
        values.sort_by_key(|item| std::cmp::Reverse(item.created_at));
        if let Some(parent) = self.state_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&values) {
            let _ = std::fs::write(&self.state_file, json);
        }
    }

    fn save_settings(&self) {
        if let Some(parent) = self.settings_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&*self.settings.lock().unwrap()) {
            let _ = std::fs::write(&self.settings_file, json);
        }
    }

    fn update<F>(&self, id: &str, mutate: F) -> Option<DownloadItem>
    where
        F: FnOnce(&mut DownloadItem),
    {
        let mut items = self.items.lock().unwrap();
        let item = items.get_mut(id)?;
        mutate(item);
        Some(item.clone())
    }
}

fn safe_filename(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|c| {
            if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.');
    if cleaned.is_empty() {
        "download".into()
    } else {
        cleaned.chars().take(180).collect()
    }
}

fn available_filename(destination: &Path, filename: &str) -> String {
    let candidate = destination.join(filename);
    let partial = destination.join(format!("{filename}.velocity-part"));
    if !candidate.exists() && !partial.exists() {
        return filename.to_string();
    }
    let path = Path::new(filename);
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|| "download".into());
    let extension = path.extension().map(|value| value.to_string_lossy());
    for index in 1..10_000 {
        let next = match &extension {
            Some(extension) => format!("{stem} ({index}).{extension}"),
            None => format!("{stem} ({index})"),
        };
        if !destination.join(&next).exists()
            && !destination.join(format!("{next}.velocity-part")).exists()
        {
            return next;
        }
    }
    format!("{}-{}", uuid::Uuid::new_v4(), filename)
}

fn available_entry_path(destination: &Path, filename: &str) -> PathBuf {
    let candidate = destination.join(filename);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(filename);
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|| "download".into());
    let extension = path.extension().map(|value| value.to_string_lossy());
    for index in 1..10_000 {
        let next = match &extension {
            Some(extension) => format!("{stem} ({index}).{extension}"),
            None => format!("{stem} ({index})"),
        };
        let candidate = destination.join(next);
        if !candidate.exists() {
            return candidate;
        }
    }
    destination.join(format!("{}-{filename}", uuid::Uuid::new_v4()))
}

fn filename_from_url(url: &Url) -> String {
    url.path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|name| !name.is_empty())
        .and_then(|name| percent_decode(name).ok())
        .map(|name| safe_filename(&name))
        .unwrap_or_else(|| "download".into())
}

fn percent_decode(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).map_err(|e| e.to_string())?;
            output.push(u8::from_str_radix(hex, 16).map_err(|e| e.to_string())?);
            i += 3;
        } else {
            output.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(output).map_err(|e| e.to_string())
}

fn emit_item(app: &AppHandle, item: &DownloadItem) {
    let _ = app.emit("download-progress", item);
}

const DIRECT_BUFFER_SIZE: usize = 256 * 1024;
const MIN_SEGMENTED_FILE_SIZE: u64 = 8 * 1024 * 1024;
const EIGHT_CONNECTION_FILE_SIZE: u64 = 64 * 1024 * 1024;
const SLOW_RANGE_PROBE: std::time::Duration = std::time::Duration::from_millis(1200);

#[derive(Debug, Clone, PartialEq, Eq)]
struct ByteRange {
    start: u64,
    end: u64,
}

#[derive(Debug)]
struct RangeProbe {
    total: u64,
    validator: Option<String>,
    elapsed: std::time::Duration,
}

#[derive(Debug)]
enum SegmentError {
    Paused,
    RangeUnsupported(String),
    Transfer(String),
}

struct SegmentRequest {
    client: reqwest::Client,
    url: String,
    validator: Option<String>,
    expected_total: u64,
    range: ByteRange,
    path: PathBuf,
}

#[derive(Clone)]
struct SegmentControl {
    downloaded: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    abort: Arc<AtomicBool>,
}

struct SegmentedAttempt<'a> {
    app: &'a AppHandle,
    manager: &'a Arc<DownloadManager>,
    item: &'a DownloadItem,
    client: &'a reqwest::Client,
    probe: &'a RangeProbe,
    staging: &'a Path,
    part_path: &'a Path,
    final_path: &'a Path,
}

fn parse_content_range(value: &str) -> Option<(u64, u64, u64)> {
    let value = value.strip_prefix("bytes ")?;
    let (range, total) = value.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let start = start.parse().ok()?;
    let end = end.parse().ok()?;
    let total = total.parse().ok()?;
    (start <= end && end < total).then_some((start, end, total))
}

fn build_segment_ranges(total: u64, count: usize) -> Vec<ByteRange> {
    let count = count.max(1).min(total.max(1) as usize);
    let base = total / count as u64;
    let remainder = total % count as u64;
    let mut start = 0_u64;
    (0..count)
        .map(|index| {
            let length = base + u64::from((index as u64) < remainder);
            let range = ByteRange {
                start,
                end: start + length - 1,
            };
            start += length;
            range
        })
        .collect()
}

fn choose_segment_count(total: u64, probe_elapsed: std::time::Duration) -> Option<usize> {
    if total < MIN_SEGMENTED_FILE_SIZE {
        None
    } else if total >= EIGHT_CONNECTION_FILE_SIZE && probe_elapsed <= SLOW_RANGE_PROBE {
        Some(8)
    } else {
        Some(4)
    }
}

async fn probe_range_support(client: &reqwest::Client, url: &str) -> Option<RangeProbe> {
    let started = std::time::Instant::now();
    let response = client
        .get(url)
        .header(RANGE, "bytes=0-0")
        .send()
        .await
        .ok()?;
    if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return None;
    }
    let content_range = response.headers().get(CONTENT_RANGE)?.to_str().ok()?;
    let (start, end, total) = parse_content_range(content_range)?;
    if start != 0 || end != 0 {
        return None;
    }
    let validator = response
        .headers()
        .get(ETAG)
        .or_else(|| response.headers().get(LAST_MODIFIED))
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    Some(RangeProbe {
        total,
        validator,
        elapsed: started.elapsed(),
    })
}

fn direct_staging_dir(app: &AppHandle, id: &str) -> PathBuf {
    app.path()
        .app_cache_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("velocity"))
        .join("direct")
        .join(id)
}

fn segment_path(staging: &Path, range: &ByteRange) -> PathBuf {
    staging.join(format!("segment-{}-{}.part", range.start, range.end))
}

async fn download_segment_inner(
    request: SegmentRequest,
    control: SegmentControl,
) -> Result<(), SegmentError> {
    let SegmentRequest {
        client,
        url,
        validator,
        expected_total,
        range,
        path,
    } = request;
    let length = range.end - range.start + 1;
    let mut existing = fs::metadata(&path)
        .await
        .map(|value| value.len())
        .unwrap_or(0);
    if existing > length {
        let _ = fs::remove_file(&path).await;
        existing = 0;
    }
    if existing == length {
        return Ok(());
    }
    if control.stop.load(Ordering::Relaxed) {
        return Err(SegmentError::Paused);
    }
    if control.abort.load(Ordering::Relaxed) {
        return Err(SegmentError::Transfer("segment attempt cancelled".into()));
    }

    let requested_start = range.start + existing;
    let mut request = client
        .get(&url)
        .header(RANGE, format!("bytes={requested_start}-{}", range.end));
    if let Some(validator) = validator {
        request = request.header(IF_RANGE, validator);
    }
    let response = request
        .send()
        .await
        .map_err(|error| SegmentError::Transfer(error.to_string()))?;
    if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(SegmentError::RangeUnsupported(format!(
            "server returned HTTP {} for a byte range",
            response.status()
        )));
    }
    let returned_range = response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_content_range)
        .ok_or_else(|| SegmentError::RangeUnsupported("invalid Content-Range response".into()))?;
    if returned_range != (requested_start, range.end, expected_total) {
        return Err(SegmentError::RangeUnsupported(
            "server returned a different byte range".into(),
        ));
    }
    let expected = range.end - requested_start + 1;
    if response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|value| value != expected)
    {
        return Err(SegmentError::RangeUnsupported(
            "server returned an unexpected range length".into(),
        ));
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
        .map_err(|error| SegmentError::Transfer(error.to_string()))?;
    let mut written = existing;
    let mut stream = response.bytes_stream();
    while let Some(result) = stream.next().await {
        if control.stop.load(Ordering::Relaxed) {
            return Err(SegmentError::Paused);
        }
        if control.abort.load(Ordering::Relaxed) {
            return Err(SegmentError::Transfer("segment attempt cancelled".into()));
        }
        let chunk = result.map_err(|error| SegmentError::Transfer(error.to_string()))?;
        for bounded in chunk.chunks(DIRECT_BUFFER_SIZE) {
            if written + bounded.len() as u64 > length {
                return Err(SegmentError::RangeUnsupported(
                    "server sent bytes outside the requested range".into(),
                ));
            }
            file.write_all(bounded)
                .await
                .map_err(|error| SegmentError::Transfer(error.to_string()))?;
            written += bounded.len() as u64;
            control
                .downloaded
                .fetch_add(bounded.len() as u64, Ordering::Relaxed);
        }
    }
    file.flush()
        .await
        .map_err(|error| SegmentError::Transfer(error.to_string()))?;
    if written != length {
        return Err(SegmentError::Transfer(format!(
            "range ended early ({written} of {length} bytes)"
        )));
    }
    Ok(())
}

async fn download_segment(
    request: SegmentRequest,
    control: SegmentControl,
) -> Result<(), SegmentError> {
    let result = download_segment_inner(request, control.clone()).await;
    if matches!(
        result,
        Err(SegmentError::RangeUnsupported(_)) | Err(SegmentError::Transfer(_))
    ) {
        control.abort.store(true, Ordering::Relaxed);
    }
    result
}

async fn run_segmented_attempt(
    context: SegmentedAttempt<'_>,
    count: usize,
    stop: Arc<AtomicBool>,
) -> Result<(), SegmentError> {
    let SegmentedAttempt {
        app,
        manager,
        item,
        client,
        probe,
        staging,
        part_path,
        final_path,
    } = context;
    fs::create_dir_all(staging)
        .await
        .map_err(|error| SegmentError::Transfer(error.to_string()))?;
    let ranges = build_segment_ranges(probe.total, count);
    let mut initial = 0_u64;
    for range in &ranges {
        let path = segment_path(staging, range);
        let length = range.end - range.start + 1;
        let existing = fs::metadata(&path)
            .await
            .map(|value| value.len())
            .unwrap_or(0);
        if existing > length {
            let _ = fs::remove_file(path).await;
        } else {
            initial += existing;
        }
    }
    let downloaded = Arc::new(AtomicU64::new(initial));
    let abort = Arc::new(AtomicBool::new(false));
    if let Some(current) = manager.update(&item.id, |download| {
        download.status = "downloading".into();
        download.downloaded = initial;
        download.total = Some(probe.total);
        download.progress = Some(initial as f64 / probe.total as f64 * 100.0);
        download.connections = Some(count as u8);
        download.error = None;
    }) {
        emit_item(app, &current);
    }

    let tasks: Vec<_> = ranges
        .iter()
        .map(|range| {
            tokio::spawn(download_segment(
                SegmentRequest {
                    client: client.clone(),
                    url: item.url.clone(),
                    validator: probe.validator.clone(),
                    expected_total: probe.total,
                    range: range.clone(),
                    path: segment_path(staging, range),
                },
                SegmentControl {
                    downloaded: downloaded.clone(),
                    stop: stop.clone(),
                    abort: abort.clone(),
                },
            ))
        })
        .collect();
    let joined = futures_util::future::join_all(tasks);
    tokio::pin!(joined);
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(350));
    let mut last_bytes = initial;
    let mut last_tick = std::time::Instant::now();
    let results = loop {
        tokio::select! {
            results = &mut joined => break results,
            _ = ticker.tick() => {
                let current_bytes = downloaded.load(Ordering::Relaxed);
                let elapsed = last_tick.elapsed().as_secs_f64();
                let speed = if elapsed > 0.0 { (current_bytes.saturating_sub(last_bytes)) as f64 / elapsed } else { 0.0 };
                if let Some(current) = manager.update(&item.id, |download| {
                    download.downloaded = current_bytes;
                    download.total = Some(probe.total);
                    download.speed = speed;
                    download.progress = Some(current_bytes as f64 / probe.total as f64 * 100.0);
                }) {
                    emit_item(app, &current);
                }
                last_bytes = current_bytes;
                last_tick = std::time::Instant::now();
            }
        }
    };
    for result in results {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(error),
            Err(error) => return Err(SegmentError::Transfer(error.to_string())),
        }
    }
    if stop.load(Ordering::Relaxed) {
        return Err(SegmentError::Paused);
    }

    let mut output = fs::File::create(part_path)
        .await
        .map_err(|error| SegmentError::Transfer(error.to_string()))?;
    for range in &ranges {
        let mut input = fs::File::open(segment_path(staging, range))
            .await
            .map_err(|error| SegmentError::Transfer(error.to_string()))?;
        tokio::io::copy(&mut input, &mut output)
            .await
            .map_err(|error| SegmentError::Transfer(error.to_string()))?;
    }
    output
        .flush()
        .await
        .map_err(|error| SegmentError::Transfer(error.to_string()))?;
    drop(output);
    let merged_length = fs::metadata(part_path)
        .await
        .map_err(|error| SegmentError::Transfer(error.to_string()))?
        .len();
    if merged_length != probe.total {
        return Err(SegmentError::Transfer(format!(
            "assembled file has {merged_length} of {} bytes",
            probe.total
        )));
    }
    if final_path.exists() {
        let _ = fs::remove_file(final_path).await;
    }
    fs::rename(part_path, final_path)
        .await
        .map_err(|error| SegmentError::Transfer(error.to_string()))?;
    let _ = fs::remove_dir_all(staging).await;
    if let Some(current) = manager.update(&item.id, |download| {
        download.status = "completed".into();
        download.downloaded = probe.total;
        download.total = Some(probe.total);
        download.speed = 0.0;
        download.progress = Some(100.0);
        download.connections = None;
    }) {
        emit_item(app, &current);
    }
    manager.save();
    Ok(())
}

async fn run_direct_download(
    app: AppHandle,
    manager: Arc<DownloadManager>,
    id: String,
    stop: Arc<AtomicBool>,
) {
    let Some(item) = manager.items.lock().unwrap().get(&id).cloned() else {
        return;
    };
    let destination = PathBuf::from(&item.destination);
    if let Err(error) = fs::create_dir_all(&destination).await {
        fail(
            &app,
            &manager,
            &id,
            format!("Cannot create download folder: {error}"),
        );
        return;
    }
    let part_path = destination.join(format!("{}.velocity-part", item.filename));
    if fs::metadata(&part_path)
        .await
        .map(|value| value.len())
        .unwrap_or(0)
        > 0
    {
        run_single_connection(app, manager, id, stop).await;
        return;
    }
    let client = match reqwest::Client::builder()
        .user_agent("Velocity/0.5.0")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            fail(&app, &manager, &id, error.to_string());
            return;
        }
    };
    let Some(probe) = probe_range_support(&client, &item.url).await else {
        run_single_connection(app, manager, id, stop).await;
        return;
    };
    let Some(mut count) = choose_segment_count(probe.total, probe.elapsed) else {
        run_single_connection(app, manager, id, stop).await;
        return;
    };
    let staging = direct_staging_dir(&app, &id);
    let final_path = destination.join(&item.filename);
    loop {
        match run_segmented_attempt(
            SegmentedAttempt {
                app: &app,
                manager: &manager,
                item: &item,
                client: &client,
                probe: &probe,
                staging: &staging,
                part_path: &part_path,
                final_path: &final_path,
            },
            count,
            stop.clone(),
        )
        .await
        {
            Ok(()) => return,
            Err(SegmentError::Paused) => {
                if let Some(current) = manager.update(&id, |download| {
                    download.status = "paused".into();
                    download.speed = 0.0;
                    download.connections = None;
                }) {
                    emit_item(&app, &current);
                }
                manager.save();
                return;
            }
            Err(SegmentError::RangeUnsupported(reason)) | Err(SegmentError::Transfer(reason)) => {
                let _ = fs::remove_dir_all(&staging).await;
                if count > 4 {
                    count = 4;
                    if let Some(current) = manager.update(&id, |download| {
                        download.error = Some(format!(
                            "Server was unstable; reduced to {count} connections ({reason})"
                        ));
                    }) {
                        emit_item(&app, &current);
                    }
                    continue;
                }
                run_single_connection(app, manager, id, stop).await;
                return;
            }
        }
    }
}

async fn run_single_connection(
    app: AppHandle,
    manager: Arc<DownloadManager>,
    id: String,
    stop: Arc<AtomicBool>,
) {
    let Some(item) = manager.items.lock().unwrap().get(&id).cloned() else {
        return;
    };
    let destination = PathBuf::from(&item.destination);
    if let Err(error) = fs::create_dir_all(&destination).await {
        fail(
            &app,
            &manager,
            &id,
            format!("Cannot create download folder: {error}"),
        );
        return;
    }
    let final_path = destination.join(&item.filename);
    let part_path = destination.join(format!("{}.velocity-part", item.filename));
    let mut offset = fs::metadata(&part_path).await.map(|m| m.len()).unwrap_or(0);
    let client = match reqwest::Client::builder()
        .user_agent("Velocity/0.5.0")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            fail(&app, &manager, &id, error.to_string());
            return;
        }
    };
    let mut request = client.get(&item.url);
    if offset > 0 {
        request = request.header(RANGE, format!("bytes={offset}-"));
    }
    let response = match request.send().await.and_then(|r| r.error_for_status()) {
        Ok(response) => response,
        Err(error) => {
            fail(&app, &manager, &id, format!("Connection failed: {error}"));
            return;
        }
    };
    let resumed = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    if offset > 0 && !resumed {
        offset = 0;
    }
    let total = response.content_length().map(|length| length + offset);
    let file_result = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(resumed)
        .truncate(!resumed)
        .open(&part_path)
        .await;
    let mut file = match file_result {
        Ok(file) => file,
        Err(error) => {
            fail(&app, &manager, &id, format!("Cannot write file: {error}"));
            return;
        }
    };
    let mut downloaded = offset;
    let mut window_bytes = 0_u64;
    let mut window_start = std::time::Instant::now();
    if let Some(current) = manager.update(&id, |d| {
        d.status = "downloading".into();
        d.downloaded = offset;
        d.total = total;
        d.progress = total.map(|value| offset as f64 / value as f64 * 100.0);
        d.connections = Some(1);
        d.error = None;
    }) {
        emit_item(&app, &current);
    }
    let mut stream = response.bytes_stream();
    while let Some(chunk_result) = stream.next().await {
        if stop.load(Ordering::Relaxed) {
            if let Some(current) = manager.update(&id, |d| {
                d.status = "paused".into();
                d.speed = 0.0;
                d.connections = None;
            }) {
                emit_item(&app, &current);
            }
            manager.save();
            return;
        }
        let chunk = match chunk_result {
            Ok(chunk) => chunk,
            Err(error) => {
                fail(
                    &app,
                    &manager,
                    &id,
                    format!("Transfer interrupted: {error}"),
                );
                return;
            }
        };
        for bounded in chunk.chunks(DIRECT_BUFFER_SIZE) {
            if let Err(error) = file.write_all(bounded).await {
                fail(&app, &manager, &id, format!("Cannot write file: {error}"));
                return;
            }
            downloaded += bounded.len() as u64;
            window_bytes += bounded.len() as u64;
        }
        let elapsed = window_start.elapsed();
        if elapsed.as_millis() >= 350 {
            let speed = window_bytes as f64 / elapsed.as_secs_f64();
            if let Some(current) = manager.update(&id, |d| {
                d.downloaded = downloaded;
                d.total = total;
                d.speed = speed;
                d.progress = total.map(|value| downloaded as f64 / value as f64 * 100.0);
            }) {
                emit_item(&app, &current);
            }
            window_start = std::time::Instant::now();
            window_bytes = 0;
        }
    }
    if let Err(error) = file.flush().await {
        fail(&app, &manager, &id, error.to_string());
        return;
    }
    drop(file);
    if final_path.exists() {
        let _ = fs::remove_file(&final_path).await;
    }
    if let Err(error) = fs::rename(&part_path, &final_path).await {
        fail(&app, &manager, &id, format!("Cannot finish file: {error}"));
        return;
    }
    if let Some(current) = manager.update(&id, |d| {
        d.status = "completed".into();
        d.downloaded = downloaded;
        d.total = Some(total.unwrap_or(downloaded));
        d.speed = 0.0;
        d.progress = Some(100.0);
        d.connections = None;
    }) {
        emit_item(&app, &current);
    }
    manager.save();
}

fn tool_path(app: &AppHandle, tool: &str) -> PathBuf {
    let executable = if cfg!(windows) {
        format!("{tool}.exe")
    } else {
        tool.to_string()
    };
    if let Ok(resources) = app.path().resource_dir() {
        let bundled = resources.join("tools").join(&executable);
        if bundled.exists() {
            return bundled;
        }
    }
    let development = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tools")
        .join(&executable);
    if development.exists() {
        development
    } else {
        PathBuf::from(executable)
    }
}

#[cfg(windows)]
fn hide_worker_console(command: &mut Command) {
    command.creation_flags(0x08000000);
}

#[cfg(not(windows))]
fn hide_worker_console(_command: &mut Command) {}

#[cfg(windows)]
async fn hide_staging_directory(path: &Path) {
    let mut command = Command::new("attrib.exe");
    command
        .arg("+h")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_worker_console(&mut command);
    let _ = command.status().await;
}

#[cfg(not(windows))]
async fn hide_staging_directory(_path: &Path) {}

async fn publish_torrent(staging: &Path, destination: &Path) -> Result<Option<String>, String> {
    let mut entries = fs::read_dir(staging)
        .await
        .map_err(|error| format!("Cannot read completed torrent data: {error}"))?;
    let mut published_name = None;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| format!("Cannot inspect completed torrent data: {error}"))?
    {
        let source = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.to_ascii_lowercase().ends_with(".aria2") {
            let _ = fs::remove_file(source).await;
            continue;
        }
        let target = available_entry_path(destination, &name);
        fs::rename(&source, &target)
            .await
            .map_err(|error| format!("Cannot move the completed torrent into place: {error}"))?;
        if published_name.is_none() {
            published_name = target
                .file_name()
                .map(|value| value.to_string_lossy().into_owned());
        }
    }
    fs::remove_dir(staging)
        .await
        .map_err(|error| format!("Cannot clean the torrent staging folder: {error}"))?;
    if let Some(parent) = staging.parent() {
        let _ = fs::remove_dir(parent).await;
    }
    Ok(published_name)
}

async fn migrate_visible_torrent_state(item: &DownloadItem, destination: &Path, staging: &Path) {
    let legacy_data = destination.join(&item.filename);
    let staged_data = staging.join(&item.filename);
    if legacy_data.exists() && !staged_data.exists() {
        let _ = fs::rename(&legacy_data, &staged_data).await;
    }
    let control_name = format!("{}.aria2", item.filename);
    let legacy_control = destination.join(&control_name);
    let staged_control = staging.join(control_name);
    if legacy_control.exists() && !staged_control.exists() {
        let _ = fs::rename(legacy_control, staged_control).await;
    }
}

async fn run_worker(
    app: AppHandle,
    manager: Arc<DownloadManager>,
    id: String,
    stop: Arc<AtomicBool>,
    worker: &str,
    args: Vec<String>,
) {
    let Some(item) = manager.items.lock().unwrap().get(&id).cloned() else {
        return;
    };
    if let Err(error) = fs::create_dir_all(&item.destination).await {
        fail(
            &app,
            &manager,
            &id,
            format!("Cannot create download folder: {error}"),
        );
        return;
    }
    let executable = tool_path(&app, worker);
    let mut command = Command::new(&executable);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    hide_worker_console(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            fail(
                &app,
                &manager,
                &id,
                format!(
                    "The {worker} engine could not start at {}: {error}",
                    executable.display()
                ),
            );
            return;
        }
    };
    let stdout = child.stdout.take().map(BufReader::new);
    let stderr = child.stderr.take().map(BufReader::new);
    let last_error = Arc::new(Mutex::new(String::new()));
    if let Some(stderr) = stderr {
        let error_buffer = last_error.clone();
        let parse_app = app.clone();
        let parse_manager = manager.clone();
        let parse_id = id.clone();
        let parse_worker = worker.to_string();
        tauri::async_runtime::spawn(async move {
            let mut lines = stderr.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                parse_worker_line(&parse_app, &parse_manager, &parse_id, &parse_worker, &line);
                if !line.trim().is_empty()
                    && !line.contains("VELOCITY_PROGRESS|")
                    && !line.contains("VELOCITY_FILE|")
                {
                    *error_buffer.lock().unwrap() = line;
                }
            }
        });
    }
    if let Some(current) = manager.update(&id, |job| {
        job.status = "downloading".into();
        job.error = None;
    }) {
        emit_item(&app, &current);
    }
    let mut lines = stdout.map(|reader| reader.lines());
    loop {
        if stop.load(Ordering::Relaxed) {
            let _ = child.kill().await;
            if let Some(current) = manager.update(&id, |job| {
                job.status = "paused".into();
                job.speed = 0.0;
            }) {
                emit_item(&app, &current);
            }
            manager.save();
            return;
        }
        let next_line = async {
            match lines.as_mut() {
                Some(lines) => lines.next_line().await,
                None => Ok(None),
            }
        };
        tokio::select! {
            line = next_line => {
                match line {
                    Ok(Some(line)) => parse_worker_line(&app, &manager, &id, worker, &line),
                    Ok(None) => break,
                    Err(error) => {
                        *last_error.lock().unwrap() = error.to_string();
                        break;
                    }
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {}
        }
    }
    match child.wait().await {
        Ok(status) if status.success() => {
            if let Some(current) = manager.update(&id, |job| {
                job.status = "completed".into();
                job.speed = 0.0;
                if job.total.is_some() {
                    job.downloaded = job.total.unwrap_or(job.downloaded);
                }
            }) {
                emit_item(&app, &current);
            }
            manager.save();
        }
        Ok(status) => {
            let detail = last_error.lock().unwrap().clone();
            fail(
                &app,
                &manager,
                &id,
                if detail.is_empty() {
                    format!("{worker} exited with status {status}")
                } else {
                    detail
                },
            );
        }
        Err(error) => fail(&app, &manager, &id, error.to_string()),
    }
}

fn parse_worker_line(
    app: &AppHandle,
    manager: &DownloadManager,
    id: &str,
    worker: &str,
    line: &str,
) {
    if worker == "yt-dlp" {
        let line = line
            .find("VELOCITY_")
            .map(|index| &line[index..])
            .unwrap_or(line);
        if let Some(filename) = line.strip_prefix("VELOCITY_FILE|") {
            if let Some(name) = Path::new(filename.trim()).file_name() {
                if let Some(current) = manager.update(id, |job| {
                    job.filename = name.to_string_lossy().into_owned();
                }) {
                    emit_item(app, &current);
                }
            }
        } else if let Some(progress) = line.strip_prefix("VELOCITY_PROGRESS|") {
            let values: Vec<_> = progress.split('|').collect();
            let downloaded = values.first().and_then(|v| v.parse::<u64>().ok());
            let total = values.get(1).and_then(|v| v.parse::<u64>().ok());
            let speed = values.get(2).and_then(|v| v.parse::<f64>().ok());
            let fragment = values.get(3).and_then(|v| v.parse::<f64>().ok());
            let fragment_count = values.get(4).and_then(|v| v.parse::<f64>().ok());
            let percent = values
                .get(5)
                .and_then(|v| v.trim().trim_end_matches('%').parse::<f64>().ok())
                .or_else(|| match (fragment, fragment_count) {
                    (Some(index), Some(count)) if count > 0.0 => Some(index / count * 100.0),
                    _ => None,
                });
            if let Some(current) = manager.update(id, |job| {
                if let Some(value) = downloaded {
                    job.downloaded = value;
                }
                if total.is_some() {
                    job.total = total;
                }
                job.speed = speed.unwrap_or(0.0);
                if percent.is_some() {
                    job.progress = percent;
                } else if let (Some(downloaded), Some(total)) = (downloaded, total) {
                    if total > 0 {
                        job.progress = Some(downloaded as f64 / total as f64 * 100.0);
                    }
                }
            }) {
                emit_item(app, &current);
            }
        }
    }
}

fn available_loopback_port() -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("Cannot reserve a local engine port: {error}"))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|error| format!("Cannot read the local engine port: {error}"))
}

async fn aria_rpc(
    client: &reqwest::Client,
    endpoint: &str,
    secret: &str,
    method: &str,
    mut params: Vec<Value>,
) -> Result<Value, String> {
    params.insert(0, Value::String(format!("token:{secret}")));
    let response = client
        .post(endpoint)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": "velocity",
            "method": method,
            "params": params,
        }))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let body: Value = response.json().await.map_err(|error| error.to_string())?;
    if let Some(error) = body.get("error") {
        return Err(error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("The torrent engine returned an error")
            .to_string());
    }
    body.get("result")
        .cloned()
        .ok_or_else(|| "The torrent engine returned no status".to_string())
}

fn value_u64(value: &Value, field: &str) -> Option<u64> {
    value.get(field)?.as_str()?.parse().ok()
}

async fn run_torrent_worker(
    app: AppHandle,
    manager: Arc<DownloadManager>,
    id: String,
    stop: Arc<AtomicBool>,
    item: DownloadItem,
) {
    let destination = PathBuf::from(&item.destination);
    let staging_root = destination.join(".velocity-incomplete");
    let staging = staging_root.join(&id);
    if let Err(error) = fs::create_dir_all(&staging).await {
        fail(
            &app,
            &manager,
            &id,
            format!("Cannot create the private torrent staging folder: {error}"),
        );
        return;
    }
    hide_staging_directory(&staging_root).await;
    migrate_visible_torrent_state(&item, &destination, &staging).await;
    let rpc_port = match available_loopback_port() {
        Ok(port) => port,
        Err(error) => {
            fail(&app, &manager, &id, error);
            return;
        }
    };
    let peer_port = available_loopback_port().unwrap_or(6881);
    let secret = uuid::Uuid::new_v4().simple().to_string();
    let mut gid = uuid::Uuid::new_v4().simple().to_string()[..16].to_string();
    let executable = tool_path(&app, "aria2c");
    let args = vec![
        "--enable-rpc=true".to_string(),
        "--rpc-listen-all=false".to_string(),
        format!("--rpc-listen-port={rpc_port}"),
        format!("--rpc-secret={secret}"),
        format!("--gid={gid}"),
        format!("--dir={}", staging.to_string_lossy()),
        "--continue=true".to_string(),
        "--seed-time=0".to_string(),
        "--auto-save-interval=60".to_string(),
        "--summary-interval=0".to_string(),
        "--console-log-level=warn".to_string(),
        "--show-console-readout=false".to_string(),
        "--file-allocation=none".to_string(),
        "--disable-ipv6=true".to_string(),
        "--enable-dht=true".to_string(),
        "--enable-dht6=false".to_string(),
        "--bt-enable-lpd=false".to_string(),
        format!("--listen-port={peer_port}"),
        format!("--dht-listen-port={peer_port}"),
        item.url,
    ];
    let mut command = Command::new(&executable);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    hide_worker_console(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            fail(
                &app,
                &manager,
                &id,
                format!(
                    "The torrent engine could not start at {}: {error}",
                    executable.display()
                ),
            );
            return;
        }
    };
    let last_message = Arc::new(Mutex::new(String::new()));
    if let Some(stream) = child.stdout.take() {
        let message = last_message.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let lower = line.to_ascii_lowercase();
                if ["error", "failed", "failure", "exception", "tracker"]
                    .iter()
                    .any(|term| lower.contains(term))
                {
                    *message.lock().unwrap() = line;
                }
            }
        });
    }
    if let Some(stream) = child.stderr.take() {
        let message = last_message.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let lower = line.to_ascii_lowercase();
                if ["error", "failed", "failure", "exception", "tracker"]
                    .iter()
                    .any(|term| lower.contains(term))
                {
                    *message.lock().unwrap() = line;
                }
            }
        });
    }
    let client = reqwest::Client::new();
    let endpoint = format!("http://127.0.0.1:{rpc_port}/jsonrpc");
    let fields = json!([
        "status",
        "totalLength",
        "completedLength",
        "downloadSpeed",
        "connections",
        "numSeeders",
        "files",
        "bittorrent",
        "followedBy",
        "errorMessage"
    ]);
    let started = std::time::Instant::now();
    loop {
        if stop.load(Ordering::Relaxed) {
            let _ = aria_rpc(
                &client,
                &endpoint,
                &secret,
                "aria2.forcePause",
                vec![Value::String(gid.clone())],
            )
            .await;
            let _ = child.kill().await;
            if let Some(current) = manager.update(&id, |job| {
                job.status = "paused".into();
                job.speed = 0.0;
            }) {
                emit_item(&app, &current);
            }
            manager.save();
            return;
        }
        match aria_rpc(
            &client,
            &endpoint,
            &secret,
            "aria2.tellStatus",
            vec![Value::String(gid.clone()), fields.clone()],
        )
        .await
        {
            Ok(status) => {
                if let Some(next_gid) = status
                    .get("followedBy")
                    .and_then(Value::as_array)
                    .and_then(|values| values.first())
                    .and_then(Value::as_str)
                {
                    gid = next_gid.to_string();
                    continue;
                }
                let state = status
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("waiting");
                let downloaded = value_u64(&status, "completedLength").unwrap_or(0);
                let total = value_u64(&status, "totalLength").filter(|value| *value > 0);
                let speed = value_u64(&status, "downloadSpeed").unwrap_or(0) as f64;
                let peers =
                    value_u64(&status, "connections").or_else(|| value_u64(&status, "numSeeders"));
                let worker_warning = last_message.lock().unwrap().clone();
                let name = status
                    .pointer("/bittorrent/info/name")
                    .and_then(Value::as_str)
                    .map(safe_filename)
                    .or_else(|| {
                        status
                            .pointer("/files/0/path")
                            .and_then(Value::as_str)
                            .and_then(|path| Path::new(path).file_name())
                            .map(|name| name.to_string_lossy().into_owned())
                    });
                if let Some(current) = manager.update(&id, |job| {
                    job.status = if total.is_some() {
                        "downloading".into()
                    } else {
                        "resolving".into()
                    };
                    job.downloaded = downloaded;
                    job.total = total;
                    job.speed = speed;
                    job.progress = total.map(|value| downloaded as f64 / value as f64 * 100.0);
                    job.peers = peers;
                    job.error = if worker_warning.is_empty() {
                        None
                    } else {
                        Some(worker_warning.clone())
                    };
                    if let Some(name) = &name {
                        job.filename = name.clone();
                    }
                }) {
                    emit_item(&app, &current);
                }
                if state == "complete" {
                    let _ = child.kill().await;
                    let published_name = match publish_torrent(&staging, &destination).await {
                        Ok(name) => name,
                        Err(error) => {
                            fail(&app, &manager, &id, error);
                            return;
                        }
                    };
                    if let Some(current) = manager.update(&id, |job| {
                        job.status = "completed".into();
                        job.speed = 0.0;
                        job.progress = Some(100.0);
                        job.downloaded = job.total.unwrap_or(job.downloaded);
                        if let Some(name) = &published_name {
                            job.filename = name.clone();
                        }
                    }) {
                        emit_item(&app, &current);
                    }
                    manager.save();
                    return;
                }
                if matches!(state, "error" | "removed") {
                    let message = status
                        .get("errorMessage")
                        .and_then(Value::as_str)
                        .filter(|message| !message.is_empty())
                        .map(str::to_string)
                        .unwrap_or_else(|| last_message.lock().unwrap().clone());
                    let _ = child.kill().await;
                    fail(
                        &app,
                        &manager,
                        &id,
                        if message.is_empty() {
                            "The torrent engine stopped unexpectedly".into()
                        } else {
                            message
                        },
                    );
                    return;
                }
            }
            Err(error) => {
                if let Ok(Some(status)) = child.try_wait() {
                    let detail = last_message.lock().unwrap().clone();
                    fail(
                        &app,
                        &manager,
                        &id,
                        if detail.is_empty() {
                            format!("The torrent engine exited with {status}: {error}")
                        } else {
                            detail
                        },
                    );
                    return;
                }
                if started.elapsed() > std::time::Duration::from_secs(15) {
                    let _ = child.kill().await;
                    fail(
                        &app,
                        &manager,
                        &id,
                        format!("Could not connect to the torrent engine: {error}"),
                    );
                    return;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(450)).await;
    }
}

async fn run_download(
    app: AppHandle,
    manager: Arc<DownloadManager>,
    id: String,
    stop: Arc<AtomicBool>,
) {
    let Some(item) = manager.items.lock().unwrap().get(&id).cloned() else {
        return;
    };
    match item.source_type.as_str() {
        "media" => {
            let temp_dir = app
                .path()
                .app_cache_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("velocity"))
                .join("media")
                .join(&id);
            let _ = fs::create_dir_all(&temp_dir).await;
            let deno = tool_path(&app, "deno").to_string_lossy().into_owned();
            let ffmpeg = tool_path(&app, "ffmpeg");
            let ffmpeg_location = ffmpeg
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_string_lossy()
                .into_owned();
            let mut args = vec![
                "--newline".into(),
                "--no-colors".into(),
                "--progress".into(),
                "--no-playlist".into(),
                "--windows-filenames".into(),
                "--js-runtimes".into(),
                format!("deno:{deno}"),
                "--ffmpeg-location".into(),
                ffmpeg_location,
                "--progress-template".into(),
                "download:VELOCITY_PROGRESS|%(progress.downloaded_bytes)s|%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.fragment_index)s|%(progress.fragment_count)s|%(progress._percent_str)s".into(),
                "--progress-delta".into(),
                "0.4".into(),
                "--print".into(),
                "before_dl:VELOCITY_FILE|%(filename)s".into(),
                "--print".into(),
                "after_move:VELOCITY_FILE|%(filepath)s".into(),
            ];
            if item.media_mode == "audio" {
                args.extend([
                    "-f".into(),
                    "ba/b".into(),
                    "--extract-audio".into(),
                    "--audio-format".into(),
                    item.audio_format.clone(),
                    "--audio-quality".into(),
                    "0".into(),
                ]);
            } else {
                let height = item.media_resolution.strip_suffix('p');
                let selector = height
                    .map(|height| format!("bv*[height<={height}][ext=mp4]+ba[ext=m4a]/b[height<={height}][ext=mp4]/bv*[height<={height}]+ba/b[height<={height}]") )
                    .unwrap_or_else(|| "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/bv*+ba/b".into());
                args.extend([
                    "-f".into(),
                    selector,
                    "--merge-output-format".into(),
                    "mp4".into(),
                ]);
            }
            args.extend([
                "-P".into(),
                item.destination,
                "-P".into(),
                format!("temp:{}", temp_dir.to_string_lossy()),
                item.url,
            ]);
            run_worker(
                app.clone(),
                manager.clone(),
                id.clone(),
                stop,
                "yt-dlp",
                args,
            )
            .await;
            let completed = manager
                .items
                .lock()
                .unwrap()
                .get(&id)
                .map(|job| job.status == "completed")
                .unwrap_or(false);
            if completed {
                let _ = fs::remove_dir_all(temp_dir).await;
            }
        }
        "torrent" => {
            run_torrent_worker(app, manager, id, stop, item).await;
        }
        _ => run_direct_download(app, manager, id, stop).await,
    }
}

fn fail(app: &AppHandle, manager: &DownloadManager, id: &str, message: String) {
    if let Some(current) = manager.update(id, |d| {
        d.status = "failed".into();
        d.speed = 0.0;
        d.connections = None;
        d.error = Some(message);
    }) {
        emit_item(app, &current);
    }
    manager.save();
}

fn start(app: AppHandle, manager: Arc<DownloadManager>, id: String) {
    let stop = Arc::new(AtomicBool::new(false));
    let done = Arc::new(Notify::new());
    manager.controls.lock().unwrap().insert(
        id.clone(),
        JobControl {
            stop: stop.clone(),
            done: done.clone(),
        },
    );
    tauri::async_runtime::spawn(async move {
        run_download(app, manager.clone(), id.clone(), stop.clone()).await;
        let mut controls = manager.controls.lock().unwrap();
        let owns_slot = controls
            .get(&id)
            .map(|control| Arc::ptr_eq(&control.stop, &stop))
            .unwrap_or(false);
        if owns_slot {
            controls.remove(&id);
        }
        drop(controls);
        done.notify_waiters();
    });
}

#[tauri::command]
fn list_downloads(manager: State<'_, Arc<DownloadManager>>) -> Vec<DownloadItem> {
    let mut items: Vec<_> = manager.items.lock().unwrap().values().cloned().collect();
    items.sort_by_key(|item| std::cmp::Reverse(item.created_at));
    items
}

#[tauri::command]
fn add_download(
    app: AppHandle,
    manager: State<'_, Arc<DownloadManager>>,
    request: DownloadRequest,
) -> Result<DownloadItem, String> {
    let parsed = Url::parse(request.url.trim())
        .map_err(|_| "Enter a valid http or https URL".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Only http and https downloads are supported".into());
    }
    let destination = request
        .destination
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from)
        .or_else(dirs::download_dir)
        .ok_or("Could not find your Downloads folder")?;
    let filename = request
        .filename
        .filter(|v| !v.trim().is_empty())
        .map(|v| safe_filename(&v))
        .unwrap_or_else(|| filename_from_url(&parsed));
    let item = DownloadItem {
        id: uuid::Uuid::new_v4().to_string(),
        url: parsed.to_string(),
        filename,
        destination: destination.to_string_lossy().into_owned(),
        downloaded: 0,
        total: None,
        speed: 0.0,
        progress: None,
        peers: None,
        connections: None,
        status: "queued".into(),
        source_type: "direct".into(),
        media_mode: default_media_mode(),
        media_resolution: default_media_resolution(),
        audio_format: default_audio_format(),
        error: None,
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    manager
        .items
        .lock()
        .unwrap()
        .insert(item.id.clone(), item.clone());
    manager.save();
    start(app, manager.inner().clone(), item.id.clone());
    Ok(item)
}

fn detected_source_type(source: &str) -> &'static str {
    let lower = source.to_ascii_lowercase();
    if lower.starts_with("magnet:?") || lower.ends_with(".torrent") {
        "torrent"
    } else if [
        "youtube.com",
        "youtu.be",
        "vimeo.com",
        "dailymotion.com",
        "tiktok.com",
        "instagram.com",
        "facebook.com",
        "fb.watch",
        "twitter.com",
        "x.com",
        "reddit.com",
        "twitch.tv",
        "soundcloud.com",
        "streamable.com",
        "rumble.com",
        "bilibili.com",
        "pinterest.com",
        "pin.it",
        "vk.com",
    ]
    .iter()
    .any(|host| lower.contains(host))
    {
        "media"
    } else if let Ok(url) = Url::parse(source) {
        let last_segment = url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .unwrap_or_default();
        if last_segment.contains('.') {
            "direct"
        } else {
            "media"
        }
    } else {
        "direct"
    }
}

fn create_source(
    app: AppHandle,
    manager: Arc<DownloadManager>,
    request: SourceRequest,
) -> Result<DownloadItem, String> {
    let source = request.source.trim().to_string();
    let kind = request
        .source_type
        .as_deref()
        .filter(|value| *value != "auto")
        .unwrap_or_else(|| detected_source_type(&source));
    if !matches!(kind, "direct" | "media" | "torrent") {
        return Err("Unsupported source type".into());
    }
    let media_mode = request.media_mode.as_deref().unwrap_or("video");
    if !matches!(media_mode, "video" | "audio") {
        return Err("Choose video or audio-only output".into());
    }
    let media_resolution = request.media_resolution.as_deref().unwrap_or("best");
    if !matches!(
        media_resolution,
        "best" | "2160p" | "1440p" | "1080p" | "720p" | "480p" | "360p"
    ) {
        return Err("Choose a supported video resolution".into());
    }
    let audio_format = request.audio_format.as_deref().unwrap_or("mp3");
    if !matches!(
        audio_format,
        "mp3" | "m4a" | "aac" | "opus" | "flac" | "wav"
    ) {
        return Err("Choose a supported audio format".into());
    }
    if kind == "torrent" {
        if !source.to_ascii_lowercase().starts_with("magnet:?") {
            let path = PathBuf::from(&source);
            let is_torrent = path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case("torrent"))
                .unwrap_or(false);
            if !path.is_file() || !is_torrent {
                return Err("Choose a valid .torrent file or enter a magnet link".into());
            }
        }
    } else {
        let parsed = Url::parse(&source).map_err(|_| "Enter a valid URL".to_string())?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err("Only http and https URLs are supported here".into());
        }
    }
    let destination = request
        .destination
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| manager.settings.lock().unwrap().default_destination.clone());
    let filename = request
        .filename
        .filter(|value| !value.trim().is_empty())
        .map(|value| safe_filename(&value))
        .unwrap_or_else(|| match kind {
            "media" => "Resolving media…".into(),
            "torrent" if source.starts_with("magnet:") => "Resolving torrent metadata…".into(),
            "torrent" => Path::new(&source)
                .file_stem()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Torrent download".into()),
            _ => Url::parse(&source)
                .map(|url| filename_from_url(&url))
                .unwrap_or_else(|_| "download".into()),
        });
    let filename = if kind == "direct" {
        available_filename(Path::new(&destination), &filename)
    } else {
        filename
    };
    let item = DownloadItem {
        id: uuid::Uuid::new_v4().to_string(),
        url: source,
        filename,
        destination,
        downloaded: 0,
        total: None,
        speed: 0.0,
        progress: None,
        peers: None,
        connections: None,
        status: if kind == "direct" {
            "queued"
        } else {
            "resolving"
        }
        .into(),
        source_type: kind.into(),
        media_mode: media_mode.into(),
        media_resolution: media_resolution.into(),
        audio_format: audio_format.into(),
        error: None,
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    manager
        .items
        .lock()
        .unwrap()
        .insert(item.id.clone(), item.clone());
    manager.save();
    start(app, manager, item.id.clone());
    Ok(item)
}

#[tauri::command]
fn add_source(
    app: AppHandle,
    manager: State<'_, Arc<DownloadManager>>,
    request: SourceRequest,
) -> Result<DownloadItem, String> {
    create_source(app, manager.inner().clone(), request)
}

#[tauri::command]
fn get_settings(manager: State<'_, Arc<DownloadManager>>) -> AppSettings {
    manager.settings.lock().unwrap().clone()
}

#[tauri::command]
fn set_default_destination(
    manager: State<'_, Arc<DownloadManager>>,
    destination: String,
) -> Result<AppSettings, String> {
    let path = PathBuf::from(destination.trim());
    if destination.trim().is_empty() {
        return Err("Choose a download folder".into());
    }
    std::fs::create_dir_all(&path).map_err(|error| format!("Cannot use that folder: {error}"))?;
    manager.settings.lock().unwrap().default_destination = path.to_string_lossy().into_owned();
    manager.save_settings();
    Ok(manager.settings.lock().unwrap().clone())
}

#[tauri::command]
fn browser_extension_package(app: AppHandle) -> Result<String, String> {
    let installed = app
        .path()
        .resource_dir()
        .map_err(|error| error.to_string())?
        .join("browser-extension.zip");
    if installed.exists() {
        return Ok(installed.to_string_lossy().into_owned());
    }
    let development = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("browser-extension");
    if development.exists() {
        Ok(development.to_string_lossy().into_owned())
    } else {
        Err("The browser extension package is missing".into())
    }
}

#[tauri::command]
fn pause_download(manager: State<'_, Arc<DownloadManager>>, id: String) -> Result<(), String> {
    manager
        .controls
        .lock()
        .unwrap()
        .get(&id)
        .ok_or("Download is not active")?
        .stop
        .store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn resume_download(
    app: AppHandle,
    manager: State<'_, Arc<DownloadManager>>,
    id: String,
) -> Result<(), String> {
    if manager.controls.lock().unwrap().contains_key(&id) {
        return Err("The download engine is still stopping; try again in a moment".into());
    }
    let allowed = manager
        .items
        .lock()
        .unwrap()
        .get(&id)
        .map(|d| d.status != "completed" && d.status != "downloading")
        .unwrap_or(false);
    if !allowed {
        return Err("Download cannot be resumed".into());
    }
    manager.update(&id, |d| {
        d.status = "queued".into();
        d.error = None;
    });
    start(app, manager.inner().clone(), id);
    Ok(())
}

#[tauri::command]
fn pause_all(manager: State<'_, Arc<DownloadManager>>) {
    for control in manager.controls.lock().unwrap().values() {
        control.stop.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
fn clear_completed(manager: State<'_, Arc<DownloadManager>>) {
    manager
        .items
        .lock()
        .unwrap()
        .retain(|_, item| item.status != "completed");
    manager.save();
}

#[tauri::command]
async fn remove_download(
    app: AppHandle,
    manager: State<'_, Arc<DownloadManager>>,
    id: String,
    delete_file: bool,
) -> Result<(), String> {
    let control = manager.controls.lock().unwrap().get(&id).cloned();
    if let Some(control) = control {
        let finished = control.done.notified();
        control.stop.store(true, Ordering::Relaxed);
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), finished).await;
    }
    let item = manager
        .items
        .lock()
        .unwrap()
        .remove(&id)
        .ok_or("Download not found")?;
    if delete_file {
        let _ = std::fs::remove_file(Path::new(&item.destination).join(&item.filename));
        let _ = std::fs::remove_file(
            Path::new(&item.destination).join(format!("{}.velocity-part", item.filename)),
        );
    }
    if matches!(item.source_type.as_str(), "media" | "direct") {
        if let Ok(cache) = app.path().app_cache_dir() {
            let kind = if item.source_type == "media" {
                "media"
            } else {
                "direct"
            };
            let _ = fs::remove_dir_all(cache.join(kind).join(&id)).await;
        }
    }
    manager.save();
    Ok(())
}

fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn browser_request_from_deep_link(deep_link: &Url) -> Result<SourceRequest, String> {
    if deep_link.scheme() != "velocity" || deep_link.host_str() != Some("download") {
        return Err("Unsupported Velocity browser route".into());
    }
    let parameters: HashMap<_, _> = deep_link.query_pairs().into_owned().collect();
    let source = parameters
        .get("url")
        .map(|value| value.trim().to_string())
        .ok_or("The browser did not provide a URL")?;
    let parsed = Url::parse(&source).map_err(|_| "The browser provided an invalid media URL")?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("The browser provided an invalid media URL".into());
    }
    let requested_type = parameters
        .get("type")
        .map(String::as_str)
        .filter(|value| matches!(*value, "auto" | "direct" | "media"));
    Ok(SourceRequest {
        source: parsed.to_string(),
        source_type: Some(requested_type.unwrap_or("media").to_string()),
        destination: None,
        filename: None,
        media_mode: parameters.get("mode").cloned(),
        media_resolution: parameters.get("resolution").cloned(),
        audio_format: parameters.get("audio_format").cloned(),
    })
}

fn handle_browser_deep_link(app: &AppHandle, deep_link: &Url) {
    let request = match browser_request_from_deep_link(deep_link) {
        Ok(request) => request,
        Err(error) => {
            let _ = app.emit("browser-handoff", error);
            focus_main_window(app);
            return;
        }
    };
    let parsed = Url::parse(&request.source).expect("validated browser URL");
    let manager = app.state::<Arc<DownloadManager>>().inner().clone();
    {
        let mut recent = manager.recent_browser_links.lock().unwrap();
        recent.retain(|_, seen| seen.elapsed() < std::time::Duration::from_secs(5));
        let request_key = format!(
            "{}|{}|{}|{}",
            parsed,
            request.media_mode.as_deref().unwrap_or("video"),
            request.media_resolution.as_deref().unwrap_or("best"),
            request.audio_format.as_deref().unwrap_or("mp3")
        );
        if recent.contains_key(&request_key) {
            focus_main_window(app);
            return;
        }
        recent.insert(request_key, std::time::Instant::now());
    }
    match create_source(app.clone(), manager, request) {
        Ok(item) => {
            emit_item(app, &item);
            let _ = app.emit("browser-handoff", "Download added from your browser");
        }
        Err(error) => {
            let _ = app.emit(
                "browser-handoff",
                format!("Browser download failed: {error}"),
            );
        }
    }
    focus_main_window(app);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            focus_main_window(app);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let app_data = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            app.manage(Arc::new(DownloadManager::load(
                app_data.join("downloads.json"),
                app_data.join("settings.json"),
            )));
            #[cfg(any(windows, target_os = "linux"))]
            app.deep_link().register_all()?;
            let app_handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    handle_browser_deep_link(&app_handle, &url);
                }
            });
            if let Some(urls) = app.deep_link().get_current()? {
                for url in urls {
                    handle_browser_deep_link(app.handle(), &url);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_downloads,
            add_download,
            add_source,
            get_settings,
            set_default_destination,
            browser_extension_package,
            pause_download,
            resume_download,
            pause_all,
            clear_completed,
            remove_download
        ])
        .run(tauri::generate_context!())
        .expect("error while running Velocity");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn serve_once(
        status: &str,
        headers: &[(&str, &str)],
        body: &'static [u8],
    ) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_string();
        let headers: Vec<(String, String)> = headers
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request_bytes = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                request_bytes.extend_from_slice(&buffer[..read]);
                if request_bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&request_bytes).into_owned();
            let mut response = format!("HTTP/1.1 {status}\r\nConnection: close\r\n");
            for (name, value) in headers {
                response.push_str(&format!("{name}: {value}\r\n"));
            }
            response.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
            request
        });
        (format!("http://{address}/file.bin"), handle)
    }

    #[test]
    fn sanitizes_platform_unsafe_filename_characters() {
        assert_eq!(safe_filename("report: Q4?.pdf"), "report_ Q4_.pdf");
        assert_eq!(safe_filename("<>:\"/\\|?*"), "_________");
    }

    #[test]
    fn extracts_and_decodes_filename_from_url() {
        let url = Url::parse("https://example.com/releases/My%20File.zip?token=secret").unwrap();
        assert_eq!(filename_from_url(&url), "My File.zip");
    }

    #[test]
    fn falls_back_when_url_has_no_filename() {
        let url = Url::parse("https://example.com/").unwrap();
        assert_eq!(filename_from_url(&url), "download");
    }

    #[test]
    fn detects_download_source_types() {
        assert_eq!(detected_source_type("magnet:?xt=urn:btih:abc"), "torrent");
        assert_eq!(detected_source_type("https://youtu.be/example"), "media");
        assert_eq!(
            detected_source_type("https://www.instagram.com/reel/example"),
            "media"
        );
        assert_eq!(
            detected_source_type("https://x.com/example/status/123"),
            "media"
        );
        assert_eq!(
            detected_source_type("https://example.com/video/123"),
            "media"
        );
        assert_eq!(
            detected_source_type("https://example.com/file.zip"),
            "direct"
        );
    }

    #[test]
    fn preserves_a_filename_when_it_is_available() {
        assert_eq!(
            available_filename(Path::new("Z:/path-that-does-not-exist"), "file.zip"),
            "file.zip"
        );
    }

    #[test]
    fn parses_only_valid_content_ranges() {
        assert_eq!(parse_content_range("bytes 0-0/100"), Some((0, 0, 100)));
        assert_eq!(parse_content_range("bytes 25-49/100"), Some((25, 49, 100)));
        assert_eq!(parse_content_range("bytes 50-25/100"), None);
        assert_eq!(parse_content_range("bytes */100"), None);
        assert_eq!(parse_content_range("items 0-0/100"), None);
    }

    #[test]
    fn segment_ranges_cover_the_file_without_gaps() {
        for count in [4, 8] {
            let ranges = build_segment_ranges(101, count);
            assert_eq!(ranges.len(), count);
            assert_eq!(ranges.first().unwrap().start, 0);
            assert_eq!(ranges.last().unwrap().end, 100);
            assert!(ranges
                .windows(2)
                .all(|pair| pair[0].end + 1 == pair[1].start));
            assert_eq!(
                ranges
                    .iter()
                    .map(|range| range.end - range.start + 1)
                    .sum::<u64>(),
                101
            );
        }
    }

    #[test]
    fn chooses_adaptive_connection_count() {
        assert_eq!(
            choose_segment_count(MIN_SEGMENTED_FILE_SIZE - 1, std::time::Duration::ZERO),
            None
        );
        assert_eq!(
            choose_segment_count(MIN_SEGMENTED_FILE_SIZE, std::time::Duration::ZERO),
            Some(4)
        );
        assert_eq!(
            choose_segment_count(EIGHT_CONNECTION_FILE_SIZE, std::time::Duration::ZERO),
            Some(8)
        );
        assert_eq!(
            choose_segment_count(EIGHT_CONNECTION_FILE_SIZE, SLOW_RANGE_PROBE * 2),
            Some(4)
        );
    }

    #[test]
    fn probes_real_range_support_and_rejects_ignored_ranges() {
        let (supported_url, supported_server) = serve_once(
            "206 Partial Content",
            &[("Content-Range", "bytes 0-0/10"), ("ETag", "\"v1\"")],
            b"0",
        );
        let client = reqwest::Client::new();
        let probe = tauri::async_runtime::block_on(probe_range_support(&client, &supported_url));
        assert!(probe.is_some(), "206 response should enable segmentation");
        let probe = probe.unwrap();
        assert_eq!(probe.total, 10);
        assert_eq!(probe.validator.as_deref(), Some("\"v1\""));
        let request = supported_server.join().unwrap();
        assert!(request.contains("range: bytes=0-0") || request.contains("Range: bytes=0-0"));

        let (ignored_url, ignored_server) = serve_once("200 OK", &[], b"0123456789");
        assert!(
            tauri::async_runtime::block_on(probe_range_support(&client, &ignored_url)).is_none()
        );
        let _ = ignored_server.join().unwrap();
    }

    #[test]
    fn downloads_only_the_requested_segment() {
        let (url, server) = serve_once(
            "206 Partial Content",
            &[("Content-Range", "bytes 2-5/10")],
            b"2345",
        );
        let root =
            std::env::temp_dir().join(format!("velocity-range-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("segment.part");
        let downloaded = Arc::new(AtomicU64::new(0));
        let result = tauri::async_runtime::block_on(download_segment(
            SegmentRequest {
                client: reqwest::Client::new(),
                url,
                validator: None,
                expected_total: 10,
                range: ByteRange { start: 2, end: 5 },
                path: path.clone(),
            },
            SegmentControl {
                downloaded: downloaded.clone(),
                stop: Arc::new(AtomicBool::new(false)),
                abort: Arc::new(AtomicBool::new(false)),
            },
        ));
        assert!(result.is_ok(), "segment failed: {result:?}");
        assert_eq!(std::fs::read(path).unwrap(), b"2345");
        assert_eq!(downloaded.load(Ordering::Relaxed), 4);
        let request = server.join().unwrap();
        assert!(request.contains("range: bytes=2-5") || request.contains("Range: bytes=2-5"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn publishes_only_finished_torrent_content() {
        let root = std::env::temp_dir().join(format!("velocity-test-{}", uuid::Uuid::new_v4()));
        let destination = root.join("downloads");
        let staging = destination.join(".velocity-incomplete").join("job");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("example.bin"), b"finished data").unwrap();
        std::fs::write(staging.join("example.bin.aria2"), b"resume metadata").unwrap();

        let published =
            tauri::async_runtime::block_on(publish_torrent(&staging, &destination)).unwrap();

        assert_eq!(published.as_deref(), Some("example.bin"));
        assert_eq!(
            std::fs::read(destination.join("example.bin")).unwrap(),
            b"finished data"
        );
        assert!(!destination.join("example.bin.aria2").exists());
        assert!(!staging.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validates_browser_handoff_urls() {
        let valid = Url::parse(
            "velocity://download?url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3Dabc&type=media",
        )
        .unwrap();
        let request = browser_request_from_deep_link(&valid).unwrap();
        assert_eq!(request.source, "https://www.youtube.com/watch?v=abc");
        assert_eq!(request.source_type.as_deref(), Some("media"));
        assert_eq!(request.media_mode.as_deref(), None);

        let audio = Url::parse(
            "velocity://download?url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3Dabc&type=media&mode=audio&audio_format=flac",
        )
        .unwrap();
        let request = browser_request_from_deep_link(&audio).unwrap();
        assert_eq!(request.media_mode.as_deref(), Some("audio"));
        assert_eq!(request.audio_format.as_deref(), Some("flac"));

        let local_file =
            Url::parse("velocity://download?url=file%3A%2F%2FC%3A%2Fsecret.txt").unwrap();
        assert!(browser_request_from_deep_link(&local_file).is_err());

        let wrong_route = Url::parse("velocity://run?url=https%3A%2F%2Fexample.com").unwrap();
        assert!(browser_request_from_deep_link(&wrong_route).is_err());
    }
}
