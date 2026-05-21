// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use keyring_core::Entry;
use reqwest::blocking::Client;
use reqwest::header::HeaderValue;
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::Color,
    Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use uuid::Uuid;

static WINDOW_COUNTER: AtomicU64 = AtomicU64::new(0);
static SYNC_LOCK: Mutex<()> = Mutex::new(());
const LIGHT_STARTUP_BACKGROUND_COLOR: Color = Color(255, 255, 255, 255);
const DARK_STARTUP_BACKGROUND_COLOR: Color = Color(26, 26, 26, 255);

fn startup_theme(config: &AppConfig) -> &'static str {
    if config.theme == "dark" {
        "dark"
    } else {
        "light"
    }
}

fn startup_background_color(config: &AppConfig) -> Color {
    if config.theme == "dark" {
        DARK_STARTUP_BACKGROUND_COLOR
    } else {
        LIGHT_STARTUP_BACKGROUND_COLOR
    }
}

fn app_url(config: &AppConfig, route: &str) -> String {
    format!("index.html?theme={}{}", startup_theme(config), route)
}

fn quick_record_debug(message: &str) {
    println!("[quick-record-debug] {}", message);
}

#[tauri::command]
fn quick_record_debug_log(message: String) {
    quick_record_debug(&message);
}

fn startup_main_url(config: &AppConfig) -> WebviewUrl {
    WebviewUrl::App(app_url(config, "#/").into())
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct TodoItem {
    id: String,
    title: String,
    status: String,
    folder_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct QuickRecordCache {
    id: String,
    content: String,
    temp_folder_name: String,
    #[serde(default = "default_quick_record_x")]
    x: f64,
    #[serde(default = "default_quick_record_y")]
    y: f64,
    width: f64,
    height: f64,
    pinned: bool,
}

#[derive(Deserialize, Debug)]
struct QuickRecordCacheInput {
    id: Option<String>,
    content: String,
    temp_folder_name: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    pinned: bool,
}

#[derive(Clone, Serialize)]
struct QuickRecordHidePayload {
    cache_id: String,
    window_label: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct AppConfig {
    data_path: String,
    language: String,
    theme: String,
    font_family: String,
    font_size: u32,
    text_color_light: String,
    text_color_dark: String,
    #[serde(default)]
    launch_at_login: bool,
    #[serde(default = "default_quick_record_shortcut")]
    quick_record_shortcut: String,
    #[serde(default = "default_hide_quick_record_shortcut")]
    hide_quick_record_shortcut: String,
    #[serde(default)]
    sync_enabled: bool,
    #[serde(default = "default_sync_provider")]
    sync_provider: String,
    #[serde(default)]
    webdav_url: String,
    #[serde(default)]
    webdav_username: String,
    #[serde(default = "default_webdav_remote_dir")]
    webdav_remote_dir: String,
    #[serde(default = "default_sync_interval_seconds")]
    sync_interval_seconds: u64,
    #[serde(default = "default_true")]
    sync_on_startup: bool,
    #[serde(default = "default_true")]
    sync_on_change: bool,
    #[serde(default = "default_sync_conflict_action")]
    sync_conflict_default_action: String,
}

#[derive(Deserialize, Debug)]
struct WebDavCredentialsInput {
    webdav_url: String,
    webdav_username: String,
    password: String,
}

#[derive(Deserialize, Debug)]
struct WebDavConnectionInput {
    webdav_url: String,
    webdav_username: String,
    webdav_remote_dir: String,
    password: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct PrepareSyncInput {
    data_path: String,
    webdav_url: String,
    webdav_username: String,
    webdav_remote_dir: String,
    password: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct SyncNowInput {
    data_path: String,
    webdav_url: String,
    webdav_username: String,
    webdav_remote_dir: String,
    password: Option<String>,
    conflict_default_action: String,
}

#[derive(Deserialize, Debug)]
struct ResolveSyncConflictInput {
    data_path: String,
    webdav_url: String,
    webdav_username: String,
    webdav_remote_dir: String,
    password: Option<String>,
    conflict_id: String,
    action: String,
}

#[derive(Serialize, Debug)]
struct WebDavConnectionResult {
    ok: bool,
    status: String,
    message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SyncManifest {
    schema_version: u32,
    device_id: String,
    last_sync_at: Option<u64>,
    generated_at: u64,
    files: BTreeMap<String, SyncFileEntry>,
    pending_ops: Vec<SyncPendingOp>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SyncSnapshot {
    schema_version: u32,
    snapshot_id: String,
    device_id: String,
    created_at: u64,
    files: BTreeMap<String, SyncFileEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RemoteSyncState {
    schema_version: u32,
    latest_snapshot: String,
    device_id: String,
    updated_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SyncFileEntry {
    path: String,
    size: u64,
    modified_at: u64,
    sha256: String,
    etag: Option<String>,
    source_device_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SyncPendingOp {
    op: String,
    path: String,
    created_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SyncConflictRecord {
    id: String,
    path: String,
    local_hash: Option<String>,
    remote_hash: Option<String>,
    base_hash: Option<String>,
    local_modified_at: Option<u64>,
    remote_modified_at: Option<u64>,
    reason: String,
    suggested_action: String,
    status: String,
    created_at: u64,
    resolved_at: Option<u64>,
    resolved_action: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SyncConflictStore {
    schema_version: u32,
    conflicts: Vec<SyncConflictRecord>,
}

#[derive(Serialize, Debug, Clone)]
struct SyncNowResult {
    uploaded_count: usize,
    downloaded_count: usize,
    skipped_count: usize,
    conflict_count: usize,
    errors: Vec<String>,
    last_sync_at: u64,
}

#[derive(Serialize, Debug)]
struct PrepareSyncResult {
    local_file_count: usize,
    local_manifest_path: String,
    remote_manifest_url: String,
    remote_initialized: bool,
}

fn default_sync_provider() -> String {
    "webdav".to_string()
}

fn default_webdav_remote_dir() -> String {
    "/simple-todo".to_string()
}

fn default_sync_interval_seconds() -> u64 {
    300
}

fn default_true() -> bool {
    true
}

fn default_sync_conflict_action() -> String {
    "ask".to_string()
}

const WEBDAV_CREDENTIAL_SERVICE: &str = "simple-todo.webdav";

fn webdav_credential_key(webdav_url: &str, webdav_username: &str) -> String {
    format!("{}|{}", webdav_url.trim(), webdav_username.trim())
}

fn webdav_credential_entry(webdav_url: &str, webdav_username: &str) -> Result<Entry, String> {
    keyring::use_native_store(false).map_err(|e| e.to_string())?;
    Entry::new(
        WEBDAV_CREDENTIAL_SERVICE,
        &webdav_credential_key(webdav_url, webdav_username),
    )
    .map_err(|e| e.to_string())
}

fn normalize_webdav_url(webdav_url: &str, remote_dir: &str) -> Result<String, String> {
    let base = webdav_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("WebDAV server URL is required".to_string());
    }
    if !base.starts_with("http://") && !base.starts_with("https://") {
        return Err("WebDAV server URL must start with http:// or https://".to_string());
    }
    let dir = remote_dir.trim().trim_matches('/');
    if dir.is_empty() {
        Ok(format!("{}/", base))
    } else {
        Ok(format!("{}/{}/", base, dir))
    }
}

fn get_webdav_password(input: &WebDavConnectionInput) -> Result<String, String> {
    if let Some(password) = &input.password {
        if !password.is_empty() {
            return Ok(password.clone());
        }
    }
    webdav_credential_entry(&input.webdav_url, &input.webdav_username)?
        .get_password()
        .map_err(|_| "No saved WebDAV credential found".to_string())
}

fn get_prepare_sync_password(input: &PrepareSyncInput) -> Result<String, String> {
    if let Some(password) = &input.password {
        if !password.is_empty() {
            return Ok(password.clone());
        }
    }
    webdav_credential_entry(&input.webdav_url, &input.webdav_username)?
        .get_password()
        .map_err(|_| "No saved WebDAV credential found".to_string())
}

fn get_sync_now_password(input: &SyncNowInput) -> Result<String, String> {
    if let Some(password) = &input.password {
        if !password.is_empty() {
            return Ok(password.clone());
        }
    }
    webdav_credential_entry(&input.webdav_url, &input.webdav_username)?
        .get_password()
        .map_err(|_| "No saved WebDAV credential found".to_string())
}

fn get_resolve_conflict_password(input: &ResolveSyncConflictInput) -> Result<String, String> {
    if let Some(password) = &input.password {
        if !password.is_empty() {
            return Ok(password.clone());
        }
    }
    webdav_credential_entry(&input.webdav_url, &input.webdav_username)?
        .get_password()
        .map_err(|_| "No saved WebDAV credential found".to_string())
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn sync_dir(data_path: &str) -> PathBuf {
    Path::new(data_path).join(".sync")
}

fn sync_manifest_path(data_path: &str) -> PathBuf {
    sync_dir(data_path).join("manifest.json")
}

fn sync_last_manifest_path(data_path: &str) -> PathBuf {
    sync_dir(data_path).join("last-sync-manifest.json")
}

fn sync_conflicts_path(data_path: &str) -> PathBuf {
    sync_dir(data_path).join("conflicts.json")
}

fn sync_base_files_dir(data_path: &str) -> PathBuf {
    sync_dir(data_path).join("base-files")
}

fn sync_base_file_path(data_path: &str, relative_path: &str) -> Result<PathBuf, String> {
    validate_sync_relative_path(relative_path)?;
    Ok(sync_base_files_dir(data_path).join(relative_path))
}

fn sync_device_id_path(data_path: &str) -> PathBuf {
    sync_dir(data_path).join("device_id")
}

fn load_or_create_device_id(data_path: &str) -> Result<String, String> {
    let dir = sync_dir(data_path);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = sync_device_id_path(data_path);
    if path.exists() {
        let id = fs::read_to_string(&path).map_err(|e| e.to_string())?.trim().to_string();
        if !id.is_empty() {
            return Ok(id);
        }
    }
    let id = Uuid::new_v4().to_string();
    fs::write(path, &id).map_err(|e| e.to_string())?;
    Ok(id)
}

fn path_modified_secs(path: &Path) -> Result<u64, String> {
    let modified = fs::metadata(path)
        .map_err(|e| e.to_string())?
        .modified()
        .map_err(|e| e.to_string())?;
    Ok(modified.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<String>())
}

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn should_skip_sync_path(relative: &Path) -> bool {
    if relative.components().next().is_some_and(|part| part.as_os_str() == ".sync") {
        return true;
    }
    relative
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.contains(".remote-conflict-"))
}

fn collect_sync_files(base: &Path, current: &Path, device_id: &str, files: &mut BTreeMap<String, SyncFileEntry>) -> Result<(), String> {
    if !current.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(current).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let relative = path.strip_prefix(base).map_err(|e| e.to_string())?;
        if should_skip_sync_path(relative) {
            continue;
        }
        if path.is_dir() {
            collect_sync_files(base, &path, device_id, files)?;
            continue;
        }
        let relative_path = normalize_relative_path(relative);
        let metadata = fs::metadata(&path).map_err(|e| e.to_string())?;
        files.insert(
            relative_path.clone(),
            SyncFileEntry {
                path: relative_path,
                size: metadata.len(),
                modified_at: path_modified_secs(&path)?,
                sha256: sha256_file(&path)?,
                etag: None,
                source_device_id: device_id.to_string(),
            },
        );
    }
    Ok(())
}

fn scan_local_manifest(data_path: &str) -> Result<SyncManifest, String> {
    let data_dir = Path::new(data_path);
    if !data_dir.exists() {
        fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    }
    let device_id = load_or_create_device_id(data_path)?;
    let mut files = BTreeMap::new();
    collect_sync_files(data_dir, data_dir, &device_id, &mut files)?;
    Ok(SyncManifest {
        schema_version: 1,
        device_id,
        last_sync_at: None,
        generated_at: now_unix_secs(),
        files,
        pending_ops: vec![],
    })
}

fn write_local_manifest(data_path: &str, manifest: &SyncManifest) -> Result<PathBuf, String> {
    let dir = sync_dir(data_path);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = sync_manifest_path(data_path);
    let content = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path)
}

fn sync_http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())
}

fn webdav_mkcol(client: &Client, url: &str, username: &str, password: &str) -> Result<StatusCode, String> {
    let mkcol = Method::from_bytes(b"MKCOL").map_err(|e| e.to_string())?;
    let response = client
        .request(mkcol, url)
        .basic_auth(username.trim(), Some(password.to_string()))
        .send()
        .map_err(|e| e.to_string())?;
    Ok(response.status())
}

fn ensure_webdav_dir(client: &Client, url: &str, username: &str, password: &str) -> Result<(), String> {
    let status = webdav_mkcol(client, url, username, password)?;
    if status.is_success() || status == StatusCode::METHOD_NOT_ALLOWED || status == StatusCode::CONFLICT {
        return Ok(());
    }
    Err(format!("Failed to create WebDAV directory: {}", status.as_u16()))
}

fn upload_webdav_text(client: &Client, url: &str, username: &str, password: &str, content: String) -> Result<(), String> {
    let response = client
        .put(url)
        .basic_auth(username.trim(), Some(password.to_string()))
        .header("Content-Type", "application/json; charset=utf-8")
        .body(content)
        .send()
        .map_err(|e| e.to_string())?;
    if response.status().is_success() || response.status() == StatusCode::CREATED || response.status() == StatusCode::NO_CONTENT {
        Ok(())
    } else {
        Err(format!("Failed to upload WebDAV manifest: {}", response.status().as_u16()))
    }
}

fn append_webdav_segment(base: &str, segment: &str) -> String {
    format!("{}{}/", base.trim_end_matches('/'), format!("/{}", segment.trim_matches('/')))
}

fn append_webdav_file(base: &str, relative_path: &str) -> Result<String, String> {
    validate_sync_relative_path(relative_path)?;
    Ok(format!("{}{}", base, encode_webdav_path(relative_path)))
}

fn encode_webdav_path(relative_path: &str) -> String {
    relative_path
        .split('/')
        .map(|segment| url::form_urlencoded::byte_serialize(segment.as_bytes()).collect::<String>())
        .collect::<Vec<_>>()
        .join("/")
}

fn remote_sync_url(root_url: &str) -> String {
    append_webdav_segment(root_url, ".simple-todo")
}

fn remote_manifest_url(root_url: &str) -> String {
    format!("{}manifest.json", remote_sync_url(root_url))
}

fn remote_state_url(root_url: &str) -> String {
    format!("{}state.json", remote_sync_url(root_url))
}

fn remote_snapshots_url(root_url: &str) -> String {
    append_webdav_segment(&remote_sync_url(root_url), "snapshots")
}

fn remote_blobs_url(root_url: &str) -> String {
    append_webdav_segment(&remote_sync_url(root_url), "blobs")
}

fn remote_snapshot_url(root_url: &str, snapshot_id: &str) -> String {
    format!("{}{}.json", remote_snapshots_url(root_url), encode_webdav_path(snapshot_id))
}

fn remote_blob_url(root_url: &str, sha256: &str) -> String {
    format!("{}sha256-{}", remote_blobs_url(root_url), sha256)
}

fn snapshot_from_manifest(manifest: &SyncManifest, snapshot_id: String) -> SyncSnapshot {
    SyncSnapshot {
        schema_version: 1,
        snapshot_id,
        device_id: manifest.device_id.clone(),
        created_at: now_unix_secs(),
        files: manifest.files.clone(),
    }
}

fn manifest_from_snapshot(snapshot: &SyncSnapshot) -> SyncManifest {
    SyncManifest {
        schema_version: 1,
        device_id: snapshot.device_id.clone(),
        last_sync_at: Some(snapshot.created_at),
        generated_at: snapshot.created_at,
        files: snapshot.files.clone(),
        pending_ops: vec![],
    }
}

fn validate_sync_relative_path(relative_path: &str) -> Result<(), String> {
    if relative_path.trim().is_empty() {
        return Err("Sync path is empty".to_string());
    }
    if relative_path.starts_with('/') || relative_path.starts_with('\\') || relative_path.contains('\\') {
        return Err(format!("Unsafe sync path: {}", relative_path));
    }
    let path = Path::new(relative_path);
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => return Err(format!("Unsafe sync path: {}", relative_path)),
        }
    }
    Ok(())
}

fn local_sync_file_path(data_path: &str, relative_path: &str) -> Result<PathBuf, String> {
    validate_sync_relative_path(relative_path)?;
    Ok(Path::new(data_path).join(relative_path))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension(format!("sync-tmp-{}", Uuid::new_v4()));
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

fn write_manifest_file(path: &Path, manifest: &SyncManifest) -> Result<(), String> {
    let content = serde_json::to_vec_pretty(manifest).map_err(|e| e.to_string())?;
    atomic_write(path, &content)
}

fn write_conflict_store(data_path: &str, store: &SyncConflictStore) -> Result<(), String> {
    let path = sync_conflicts_path(data_path);
    let content = serde_json::to_vec_pretty(store).map_err(|e| e.to_string())?;
    atomic_write(&path, &content)
}

fn read_local_manifest(path: &Path) -> Result<Option<SyncManifest>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map(Some).map_err(|e| e.to_string())
}

fn read_conflict_store(data_path: &str) -> Result<SyncConflictStore, String> {
    let path = sync_conflicts_path(data_path);
    if !path.exists() {
        return Ok(SyncConflictStore { schema_version: 1, conflicts: vec![] });
    }
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn active_conflict_paths(data_path: &str) -> Result<HashSet<String>, String> {
    Ok(read_conflict_store(data_path)?
        .conflicts
        .into_iter()
        .filter(|conflict| conflict.status == "open")
        .map(|conflict| conflict.path)
        .collect())
}

fn upsert_conflict(data_path: &str, conflict: SyncConflictRecord) -> Result<(), String> {
    let mut store = read_conflict_store(data_path)?;
    if store.conflicts.iter().any(|item| item.status == "open" && item.path == conflict.path) {
        return write_conflict_store(data_path, &store);
    }
    store.conflicts.push(conflict);
    write_conflict_store(data_path, &store)
}

fn mark_conflict_resolved(data_path: &str, conflict_id: &str, action: &str) -> Result<(), String> {
    let mut store = read_conflict_store(data_path)?;
    let now = now_unix_secs();
    let conflict = store
        .conflicts
        .iter_mut()
        .find(|item| item.id == conflict_id)
        .ok_or_else(|| "Sync conflict not found".to_string())?;
    conflict.status = "resolved".to_string();
    conflict.resolved_at = Some(now);
    conflict.resolved_action = Some(action.to_string());
    write_conflict_store(data_path, &store)
}

fn get_open_conflict(data_path: &str, conflict_id: &str) -> Result<SyncConflictRecord, String> {
    read_conflict_store(data_path)?
        .conflicts
        .into_iter()
        .find(|item| item.id == conflict_id && item.status == "open")
        .ok_or_else(|| "Sync conflict not found".to_string())
}

fn conflict_copy_path(data_path: &str, relative_path: &str) -> Result<PathBuf, String> {
    let source = local_sync_file_path(data_path, relative_path)?;
    let parent = source.parent().unwrap_or_else(|| Path::new(data_path));
    let file_name = source.file_name().and_then(|name| name.to_str()).unwrap_or("file");
    let stamp = now_unix_secs();
    if let Some((stem, ext)) = file_name.rsplit_once('.') {
        Ok(parent.join(format!("{}.remote-conflict-{}.{}", stem, stamp, ext)))
    } else {
        Ok(parent.join(format!("{}.remote-conflict-{}", file_name, stamp)))
    }
}

fn read_remote_manifest(client: &Client, url: &str, username: &str, password: &str) -> Result<Option<SyncManifest>, String> {
    let response = client
        .get(url)
        .basic_auth(username.trim(), Some(password.to_string()))
        .send()
        .map_err(|e| e.to_string())?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(format!("Failed to download WebDAV manifest: {}", response.status().as_u16()));
    }
    let content = response.text().map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map(Some).map_err(|e| e.to_string())
}

fn read_remote_state(client: &Client, root_url: &str, username: &str, password: &str) -> Result<Option<RemoteSyncState>, String> {
    let response = client
        .get(&remote_state_url(root_url))
        .basic_auth(username.trim(), Some(password.to_string()))
        .send()
        .map_err(|e| e.to_string())?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(format!("Failed to download sync state: {}", response.status().as_u16()));
    }
    let content = response.text().map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map(Some).map_err(|e| e.to_string())
}

fn read_remote_snapshot(client: &Client, root_url: &str, username: &str, password: &str, snapshot_id: &str) -> Result<SyncSnapshot, String> {
    let response = client
        .get(&remote_snapshot_url(root_url, snapshot_id))
        .basic_auth(username.trim(), Some(password.to_string()))
        .send()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Failed to download snapshot {}: {}", snapshot_id, response.status().as_u16()));
    }
    let content = response.text().map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn read_remote_snapshot_manifest(client: &Client, root_url: &str, username: &str, password: &str) -> Result<Option<SyncManifest>, String> {
    if let Some(state) = read_remote_state(client, root_url, username, password)? {
        return read_remote_snapshot(client, root_url, username, password, &state.latest_snapshot).map(|snapshot| Some(manifest_from_snapshot(&snapshot)));
    }
    read_remote_manifest(client, &remote_manifest_url(root_url), username, password)
}

fn upload_webdav_bytes(client: &Client, url: &str, username: &str, password: &str, bytes: Vec<u8>, content_type: &str) -> Result<(), String> {
    let response = client
        .put(url)
        .basic_auth(username.trim(), Some(password.to_string()))
        .header("Content-Type", content_type)
        .body(bytes)
        .send()
        .map_err(|e| e.to_string())?;
    if response.status().is_success() || response.status() == StatusCode::CREATED || response.status() == StatusCode::NO_CONTENT {
        Ok(())
    } else {
        Err(format!("Failed to upload WebDAV resource: {}", response.status().as_u16()))
    }
}

fn remote_resource_exists(client: &Client, url: &str, username: &str, password: &str) -> Result<bool, String> {
    let response = client
        .head(url)
        .basic_auth(username.trim(), Some(password.to_string()))
        .send()
        .map_err(|e| e.to_string())?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(false);
    }
    if response.status().is_success() {
        return Ok(true);
    }
    if response.status() == StatusCode::METHOD_NOT_ALLOWED {
        let get_response = client
            .get(url)
            .basic_auth(username.trim(), Some(password.to_string()))
            .send()
            .map_err(|e| e.to_string())?;
        if get_response.status() == StatusCode::NOT_FOUND {
            return Ok(false);
        }
        if get_response.status().is_success() {
            return Ok(true);
        }
        return Err(format!("Failed to check remote resource: {}", get_response.status().as_u16()));
    }
    Err(format!("Failed to check remote resource: {}", response.status().as_u16()))
}

fn upload_snapshot_blobs(client: &Client, root_url: &str, username: &str, password: &str, data_path: &str, manifest: &SyncManifest) -> Result<usize, String> {
    let mut uploaded = 0;
    for entry in manifest.files.values() {
        let blob_url = remote_blob_url(root_url, &entry.sha256);
        if remote_resource_exists(client, &blob_url, username, password)? {
            continue;
        }
        let bytes = fs::read(local_sync_file_path(data_path, &entry.path)?).map_err(|e| e.to_string())?;
        upload_webdav_bytes(client, &blob_url, username, password, bytes, "application/octet-stream")?;
        uploaded += 1;
    }
    Ok(uploaded)
}

fn commit_snapshot_state(client: &Client, root_url: &str, username: &str, password: &str, data_path: &str, manifest: &SyncManifest) -> Result<String, String> {
    let snapshot_id = format!("{}-{}", now_unix_secs(), Uuid::new_v4());
    let snapshot = snapshot_from_manifest(manifest, snapshot_id.clone());
    let snapshot_content = serde_json::to_vec_pretty(&snapshot).map_err(|e| e.to_string())?;
    upload_webdav_bytes(
        client,
        &remote_snapshot_url(root_url, &snapshot_id),
        username,
        password,
        snapshot_content,
        "application/json; charset=utf-8",
    )?;
    let state = RemoteSyncState {
        schema_version: 1,
        latest_snapshot: snapshot_id.clone(),
        device_id: manifest.device_id.clone(),
        updated_at: now_unix_secs(),
    };
    let state_content = serde_json::to_vec_pretty(&state).map_err(|e| e.to_string())?;
    upload_webdav_bytes(
        client,
        &remote_state_url(root_url),
        username,
        password,
        state_content,
        "application/json; charset=utf-8",
    )?;
    let legacy_manifest = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    upload_webdav_text(client, &remote_manifest_url(root_url), username, password, legacy_manifest)?;
    write_manifest_file(&sync_manifest_path(data_path), manifest)?;
    write_manifest_file(&sync_last_manifest_path(data_path), manifest)?;
    write_base_file_snapshots(data_path, manifest)?;
    Ok(snapshot_id)
}

fn upload_webdav_file(client: &Client, root_url: &str, username: &str, password: &str, data_path: &str, relative_path: &str) -> Result<(), String> {
    let local_path = local_sync_file_path(data_path, relative_path)?;
    let bytes = fs::read(&local_path).map_err(|e| e.to_string())?;
    let parts: Vec<&str> = relative_path.split('/').collect();
    if parts.len() > 1 {
        let mut current = root_url.to_string();
        for part in &parts[..parts.len() - 1] {
            current = append_webdav_segment(&current, part);
            ensure_webdav_dir(client, &current, username, password)?;
        }
    }
    let url = append_webdav_file(root_url, relative_path)?;
    let response = client
        .put(&url)
        .basic_auth(username.trim(), Some(password.to_string()))
        .body(bytes)
        .send()
        .map_err(|e| e.to_string())?;
    if response.status().is_success() || response.status() == StatusCode::CREATED || response.status() == StatusCode::NO_CONTENT {
        Ok(())
    } else {
        Err(format!("Failed to upload {}: {}", relative_path, response.status().as_u16()))
    }
}

fn download_webdav_file(client: &Client, root_url: &str, username: &str, password: &str, data_path: &str, relative_path: &str) -> Result<(), String> {
    let url = append_webdav_file(root_url, relative_path)?;
    let response = client
        .get(&url)
        .basic_auth(username.trim(), Some(password.to_string()))
        .send()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Failed to download {}: {}", relative_path, response.status().as_u16()));
    }
    let bytes = response.bytes().map_err(|e| e.to_string())?;
    let local_path = local_sync_file_path(data_path, relative_path)?;
    atomic_write(&local_path, &bytes)
}

fn download_snapshot_blob(client: &Client, root_url: &str, username: &str, password: &str, data_path: &str, entry: &SyncFileEntry) -> Result<(), String> {
    let response = client
        .get(&remote_blob_url(root_url, &entry.sha256))
        .basic_auth(username.trim(), Some(password.to_string()))
        .send()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Failed to download blob for {}: {}", entry.path, response.status().as_u16()));
    }
    let bytes = response.bytes().map_err(|e| e.to_string())?;
    let local_path = local_sync_file_path(data_path, &entry.path)?;
    atomic_write(&local_path, &bytes)
}

fn save_remote_blob_as_conflict_copy(client: &Client, root_url: &str, username: &str, password: &str, data_path: &str, entry: &SyncFileEntry) -> Result<String, String> {
    let response = client
        .get(&remote_blob_url(root_url, &entry.sha256))
        .basic_auth(username.trim(), Some(password.to_string()))
        .send()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Failed to download conflict blob {}: {}", entry.path, response.status().as_u16()));
    }
    let bytes = response.bytes().map_err(|e| e.to_string())?;
    let copy_path = conflict_copy_path(data_path, &entry.path)?;
    atomic_write(&copy_path, &bytes)?;
    let relative = copy_path
        .strip_prefix(Path::new(data_path))
        .map_err(|e| e.to_string())?;
    Ok(normalize_relative_path(relative))
}

fn save_remote_as_conflict_copy(client: &Client, root_url: &str, username: &str, password: &str, data_path: &str, relative_path: &str) -> Result<String, String> {
    let url = append_webdav_file(root_url, relative_path)?;
    let response = client
        .get(&url)
        .basic_auth(username.trim(), Some(password.to_string()))
        .send()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Failed to download conflict copy {}: {}", relative_path, response.status().as_u16()));
    }
    let bytes = response.bytes().map_err(|e| e.to_string())?;
    let copy_path = conflict_copy_path(data_path, relative_path)?;
    atomic_write(&copy_path, &bytes)?;
    let relative = copy_path
        .strip_prefix(Path::new(data_path))
        .map_err(|e| e.to_string())?;
    Ok(normalize_relative_path(relative))
}

fn write_base_file_snapshot(data_path: &str, relative_path: &str) -> Result<(), String> {
    let source = local_sync_file_path(data_path, relative_path)?;
    if !source.exists() {
        return Ok(());
    }
    let target = sync_base_file_path(data_path, relative_path)?;
    let bytes = fs::read(source).map_err(|e| e.to_string())?;
    atomic_write(&target, &bytes)
}

fn write_base_file_snapshots(data_path: &str, manifest: &SyncManifest) -> Result<(), String> {
    for path in manifest.files.keys() {
        write_base_file_snapshot(data_path, path)?;
    }
    Ok(())
}

fn upload_manifest_pair_preserving_conflicts(client: &Client, root_url: &str, username: &str, password: &str, data_path: &str, remote_manifest: &SyncManifest) -> Result<SyncManifest, String> {
    let mut manifest = scan_local_manifest(data_path)?;
    let now = now_unix_secs();
    manifest.last_sync_at = Some(now);
    manifest.generated_at = now;

    let mut remote_manifest_to_upload = manifest.clone();
    for path in active_conflict_paths(data_path)? {
        if let Some(remote_entry) = remote_manifest.files.get(&path) {
            remote_manifest_to_upload.files.insert(path, remote_entry.clone());
        } else {
            remote_manifest_to_upload.files.remove(&path);
        }
    }

    upload_snapshot_blobs(client, root_url, username, password, data_path, &remote_manifest_to_upload)?;
    let _ = commit_snapshot_state(client, root_url, username, password, data_path, &remote_manifest_to_upload)?;
    Ok(manifest)
}

fn upload_manifest_pair(client: &Client, root_url: &str, username: &str, password: &str, data_path: &str) -> Result<SyncManifest, String> {
    let mut manifest = scan_local_manifest(data_path)?;
    let now = now_unix_secs();
    manifest.last_sync_at = Some(now);
    manifest.generated_at = now;
    upload_snapshot_blobs(client, root_url, username, password, data_path, &manifest)?;
    let _ = commit_snapshot_state(client, root_url, username, password, data_path, &manifest)?;
    Ok(manifest)
}

fn create_sync_conflict(path: &str, local: Option<&SyncFileEntry>, remote: Option<&SyncFileEntry>, base: Option<&SyncFileEntry>, reason: &str, suggested_action: &str) -> SyncConflictRecord {
    SyncConflictRecord {
        id: Uuid::new_v4().to_string(),
        path: path.to_string(),
        local_hash: local.map(|entry| entry.sha256.clone()),
        remote_hash: remote.map(|entry| entry.sha256.clone()),
        base_hash: base.map(|entry| entry.sha256.clone()),
        local_modified_at: local.map(|entry| entry.modified_at),
        remote_modified_at: remote.map(|entry| entry.modified_at),
        reason: reason.to_string(),
        suggested_action: suggested_action.to_string(),
        status: "open".to_string(),
        created_at: now_unix_secs(),
        resolved_at: None,
        resolved_action: None,
    }
}

fn should_apply_default_action(action: &str) -> bool {
    matches!(action, "keep_local" | "use_remote" | "duplicate_remote")
}

fn apply_sync_action(client: &Client, root_url: &str, username: &str, password: &str, data_path: &str, relative_path: &str, action: &str) -> Result<(), String> {
    match action {
        "keep_local" => upload_webdav_file(client, root_url, username, password, data_path, relative_path),
        "use_remote" => download_webdav_file(client, root_url, username, password, data_path, relative_path),
        "duplicate_remote" => {
            let _ = save_remote_as_conflict_copy(client, root_url, username, password, data_path, relative_path)?;
            upload_webdav_file(client, root_url, username, password, data_path, relative_path)
        }
        "mark_resolved" => upload_webdav_file(client, root_url, username, password, data_path, relative_path),
        "later" => Ok(()),
        _ => Err(format!("Unsupported sync conflict action: {}", action)),
    }
}

fn apply_sync_action_with_remote_entry(client: &Client, root_url: &str, username: &str, password: &str, data_path: &str, relative_path: &str, remote: Option<&SyncFileEntry>, action: &str) -> Result<(), String> {
    match action {
        "use_remote" => {
            let entry = remote.ok_or_else(|| "Remote conflict version is missing".to_string())?;
            download_snapshot_blob(client, root_url, username, password, data_path, entry)
        }
        "duplicate_remote" => {
            if let Some(entry) = remote {
                let _ = save_remote_blob_as_conflict_copy(client, root_url, username, password, data_path, entry)?;
            }
            upload_webdav_file(client, root_url, username, password, data_path, relative_path)
        }
        "keep_local" | "mark_resolved" => upload_webdav_file(client, root_url, username, password, data_path, relative_path),
        _ => apply_sync_action(client, root_url, username, password, data_path, relative_path, action),
    }
}

fn auto_merge_todos(client: &Client, root_url: &str, username: &str, password: &str, data_path: &str, local: Option<&SyncFileEntry>, remote: Option<&SyncFileEntry>, base: Option<&SyncFileEntry>) -> Result<bool, String> {
    if local.is_none() || remote.is_none() || base.is_none() {
        return Ok(false);
    }
    let base_path = sync_base_file_path(data_path, "todos.json")?;
    if !base_path.exists() {
        return Ok(false);
    }
    let local_content = fs::read_to_string(local_sync_file_path(data_path, "todos.json")?).map_err(|e| e.to_string())?;
    let base_content = fs::read_to_string(base_path).map_err(|e| e.to_string())?;
    let remote_entry = remote.ok_or_else(|| "Remote todos snapshot is missing".to_string())?;
    let remote_response = client
        .get(&remote_blob_url(root_url, &remote_entry.sha256))
        .basic_auth(username.trim(), Some(password.to_string()))
        .send()
        .map_err(|e| e.to_string())?;
    if !remote_response.status().is_success() {
        return Err(format!("Failed to download todos.json for merge: {}", remote_response.status().as_u16()));
    }
    let remote_content = remote_response.text().map_err(|e| e.to_string())?;

    let local_todos: Vec<TodoItem> = serde_json::from_str(&local_content).map_err(|e| e.to_string())?;
    let remote_todos: Vec<TodoItem> = serde_json::from_str(&remote_content).map_err(|e| e.to_string())?;
    let base_todos: Vec<TodoItem> = serde_json::from_str(&base_content).map_err(|e| e.to_string())?;

    let local_map: BTreeMap<String, TodoItem> = local_todos.into_iter().map(|todo| (todo.id.clone(), todo)).collect();
    let remote_map: BTreeMap<String, TodoItem> = remote_todos.into_iter().map(|todo| (todo.id.clone(), todo)).collect();
    let base_map: BTreeMap<String, TodoItem> = base_todos.into_iter().map(|todo| (todo.id.clone(), todo)).collect();
    let mut ids: Vec<String> = vec![];
    for id in local_map.keys().chain(remote_map.keys()).chain(base_map.keys()) {
        if !ids.contains(id) {
            ids.push(id.clone());
        }
    }

    let mut merged = vec![];
    for id in ids {
        let local_item = local_map.get(&id);
        let remote_item = remote_map.get(&id);
        let base_item = base_map.get(&id);
        if local_item == remote_item {
            if let Some(todo) = local_item {
                merged.push(todo.clone());
            }
            continue;
        }
        if base_item.is_some() && (local_item.is_none() || remote_item.is_none()) {
            return Ok(false);
        }
        let local_changed = local_item != base_item;
        let remote_changed = remote_item != base_item;
        match (local_changed, remote_changed, local_item, remote_item, base_item) {
            (true, false, Some(todo), _, _) => merged.push(todo.clone()),
            (false, true, _, Some(todo), _) => merged.push(todo.clone()),
            (false, false, _, _, Some(todo)) => merged.push(todo.clone()),
            (true, true, Some(todo), None, None) | (true, true, None, Some(todo), None) => merged.push(todo.clone()),
            _ => return Ok(false),
        }
    }

    let merged_content = serde_json::to_vec(&merged).map_err(|e| e.to_string())?;
    atomic_write(&local_sync_file_path(data_path, "todos.json")?, &merged_content)?;
    upload_webdav_file(client, root_url, username, password, data_path, "todos.json")?;
    Ok(true)
}

#[tauri::command]
fn list_sync_conflicts(data_path: String) -> Result<Vec<SyncConflictRecord>, String> {
    Ok(read_conflict_store(&data_path)?
        .conflicts
        .into_iter()
        .filter(|conflict| conflict.status == "open")
        .collect())
}

fn ensure_snapshot_remote_dirs(client: &Client, root_url: &str, username: &str, password: &str) -> Result<(), String> {
    ensure_webdav_dir(client, root_url, username, password)?;
    let sync_url = remote_sync_url(root_url);
    ensure_webdav_dir(client, &sync_url, username, password)?;
    ensure_webdav_dir(client, &remote_snapshots_url(root_url), username, password)?;
    ensure_webdav_dir(client, &remote_blobs_url(root_url), username, password)
}

#[tauri::command]
fn sync_now(app: tauri::AppHandle, input: SyncNowInput) -> Result<SyncNowResult, String> {
    let _guard = SYNC_LOCK.try_lock().map_err(|_| "Sync is already running".to_string())?;
    if input.webdav_username.trim().is_empty() {
        return Err("WebDAV username is required".to_string());
    }
    let password = get_sync_now_password(&input)?;
    let root_url = normalize_webdav_url(&input.webdav_url, &input.webdav_remote_dir)?;
    let client = sync_http_client()?;
    ensure_snapshot_remote_dirs(&client, &root_url, &input.webdav_username, &password)?;

    let local_manifest = scan_local_manifest(&input.data_path)?;
    let remote_manifest = read_remote_snapshot_manifest(&client, &root_url, &input.webdav_username, &password)?
        .unwrap_or_else(|| SyncManifest {
            schema_version: 1,
            device_id: "remote-empty".to_string(),
            last_sync_at: None,
            generated_at: now_unix_secs(),
            files: BTreeMap::new(),
            pending_ops: vec![],
        });
    let base_manifest = read_local_manifest(&sync_last_manifest_path(&input.data_path))?;
    let active_conflicts = active_conflict_paths(&input.data_path)?;

    let mut result = SyncNowResult {
        uploaded_count: 0,
        downloaded_count: 0,
        skipped_count: 0,
        conflict_count: 0,
        errors: vec![],
        last_sync_at: now_unix_secs(),
    };

    let mut paths: HashSet<String> = HashSet::new();
    paths.extend(local_manifest.files.keys().cloned());
    paths.extend(remote_manifest.files.keys().cloned());
    if let Some(base) = &base_manifest {
        paths.extend(base.files.keys().cloned());
    }

    for path in paths {
        if active_conflicts.contains(&path) {
            result.skipped_count += 1;
            continue;
        }
        let local = local_manifest.files.get(&path);
        let remote = remote_manifest.files.get(&path);
        let base = base_manifest.as_ref().and_then(|manifest| manifest.files.get(&path));

        if local.map(|entry| &entry.sha256) == remote.map(|entry| &entry.sha256) {
            if local.is_some() {
                match remote_resource_exists(&client, &remote_blob_url(&root_url, &local.unwrap().sha256), &input.webdav_username, &password) {
                    Ok(true) => {
                        result.skipped_count += 1;
                    }
                    Ok(false) => match upload_snapshot_blobs(&client, &root_url, &input.webdav_username, &password, &input.data_path, &local_manifest) {
                        Ok(count) => result.uploaded_count += count,
                        Err(error) => result.errors.push(error),
                    },
                    Err(error) => result.errors.push(error),
                }
            } else {
                result.skipped_count += 1;
            }
            continue;
        }

        let base_hash = base.map(|entry| entry.sha256.as_str());
        let local_changed = local.map(|entry| entry.sha256.as_str()) != base_hash;
        let remote_changed = remote.map(|entry| entry.sha256.as_str()) != base_hash;

        let operation = match (local, remote, base) {
            (Some(_), None, None) => Some("upload"),
            (None, Some(_), None) => Some("download"),
            (Some(_), Some(_), None) => None,
            (Some(_), Some(_), Some(_)) if local_changed && !remote_changed => Some("upload"),
            (Some(_), Some(_), Some(_)) if !local_changed && remote_changed => Some("download"),
            (Some(_), None, Some(_)) | (None, Some(_), Some(_)) => None,
            _ => None,
        };

        if let Some(operation) = operation {
            let applied = if operation == "upload" {
                match local {
                    Some(entry) => {
                        let blob_url = remote_blob_url(&root_url, &entry.sha256);
                        if remote_resource_exists(&client, &blob_url, &input.webdav_username, &password).unwrap_or(false) {
                            Ok(())
                        } else {
                            let bytes = fs::read(local_sync_file_path(&input.data_path, &entry.path)?).map_err(|e| e.to_string())?;
                            upload_webdav_bytes(&client, &blob_url, &input.webdav_username, &password, bytes, "application/octet-stream")
                        }
                    }
                    None => Ok(()),
                }
            } else {
                let entry = remote.ok_or_else(|| "Remote file entry is missing".to_string())?;
                download_snapshot_blob(&client, &root_url, &input.webdav_username, &password, &input.data_path, entry)
            };
            match applied {
                Ok(()) => {
                    if operation == "upload" {
                        result.uploaded_count += 1;
                    } else {
                        result.downloaded_count += 1;
                    }
                }
                Err(error) => result.errors.push(error),
            }
            continue;
        }

        if path == "todos.json" && local.is_some() && remote.is_some() && base.is_some() {
            match auto_merge_todos(&client, &root_url, &input.webdav_username, &password, &input.data_path, local, remote, base) {
                Ok(true) => {
                    result.uploaded_count += 1;
                    result.downloaded_count += 1;
                    continue;
                }
                Ok(false) => {}
                Err(error) => result.errors.push(error),
            }
        }

        if should_apply_default_action(&input.conflict_default_action) && local.is_some() && remote.is_some() {
            match apply_sync_action_with_remote_entry(&client, &root_url, &input.webdav_username, &password, &input.data_path, &path, remote, &input.conflict_default_action) {
                Ok(()) => match input.conflict_default_action.as_str() {
                    "keep_local" => result.uploaded_count += 1,
                    "use_remote" => result.downloaded_count += 1,
                    "duplicate_remote" => {
                        result.uploaded_count += 1;
                        result.downloaded_count += 1;
                    }
                    _ => {}
                },
                Err(error) => result.errors.push(error),
            }
            continue;
        }

        let conflict = create_sync_conflict(
            &path,
            local,
            remote,
            base,
            if local.is_none() || remote.is_none() { "delete_conflict" } else { "both_changed" },
            if local.is_none() || remote.is_none() { "keep_local" } else { "duplicate_remote" },
        );
        upsert_conflict(&input.data_path, conflict)?;
        result.conflict_count += 1;
    }

    let final_manifest = upload_manifest_pair_preserving_conflicts(&client, &root_url, &input.webdav_username, &password, &input.data_path, &remote_manifest)?;
    result.last_sync_at = final_manifest.last_sync_at.unwrap_or_else(now_unix_secs);
    result.conflict_count = list_sync_conflicts(input.data_path.clone())?.len();
    if result.downloaded_count > 0 {
        let _ = app.emit("sync-local-updated", ());
    }
    Ok(result)
}

#[tauri::command]
fn resolve_sync_conflict(app: tauri::AppHandle, input: ResolveSyncConflictInput) -> Result<SyncNowResult, String> {
    let _guard = SYNC_LOCK.try_lock().map_err(|_| "Sync is already running".to_string())?;
    if input.action == "later" {
        return Ok(SyncNowResult {
            uploaded_count: 0,
            downloaded_count: 0,
            skipped_count: 1,
            conflict_count: list_sync_conflicts(input.data_path.clone())?.len(),
            errors: vec![],
            last_sync_at: now_unix_secs(),
        });
    }
    if input.webdav_username.trim().is_empty() {
        return Err("WebDAV username is required".to_string());
    }
    let conflict = get_open_conflict(&input.data_path, &input.conflict_id)?;
    let password = get_resolve_conflict_password(&input)?;
    let root_url = normalize_webdav_url(&input.webdav_url, &input.webdav_remote_dir)?;
    let client = sync_http_client()?;
    ensure_snapshot_remote_dirs(&client, &root_url, &input.webdav_username, &password)?;

    let remote_manifest = read_remote_snapshot_manifest(&client, &root_url, &input.webdav_username, &password)?;
    let remote_entry = remote_manifest
        .as_ref()
        .and_then(|manifest| manifest.files.get(&conflict.path));
    apply_sync_action_with_remote_entry(
        &client,
        &root_url,
        &input.webdav_username,
        &password,
        &input.data_path,
        &conflict.path,
        remote_entry,
        &input.action,
    )?;
    mark_conflict_resolved(&input.data_path, &input.conflict_id, &input.action)?;
    let final_manifest = upload_manifest_pair(&client, &root_url, &input.webdav_username, &password, &input.data_path)?;
    let mut result = SyncNowResult {
        uploaded_count: usize::from(input.action == "keep_local" || input.action == "duplicate_remote" || input.action == "mark_resolved"),
        downloaded_count: usize::from(input.action == "use_remote" || input.action == "duplicate_remote"),
        skipped_count: 0,
        conflict_count: list_sync_conflicts(input.data_path.clone())?.len(),
        errors: vec![],
        last_sync_at: final_manifest.last_sync_at.unwrap_or_else(now_unix_secs),
    };
    if input.action == "duplicate_remote" {
        result.downloaded_count = 1;
    }
    if result.downloaded_count > 0 {
        let _ = app.emit("sync-local-updated", ());
    }
    Ok(result)
}

#[tauri::command]
fn prepare_sync_manifest(input: PrepareSyncInput) -> Result<PrepareSyncResult, String> {
    if input.webdav_username.trim().is_empty() {
        return Err("WebDAV username is required".to_string());
    }
    let password = get_prepare_sync_password(&input)?;
    let manifest = scan_local_manifest(&input.data_path)?;
    let local_manifest_path = write_local_manifest(&input.data_path, &manifest)?;
    let root_url = normalize_webdav_url(&input.webdav_url, &input.webdav_remote_dir)?;
    let manifest_url = remote_state_url(&root_url);
    let client = sync_http_client()?;
    ensure_snapshot_remote_dirs(&client, &root_url, &input.webdav_username, &password)?;
    upload_snapshot_blobs(&client, &root_url, &input.webdav_username, &password, &input.data_path, &manifest)?;
    let _ = commit_snapshot_state(&client, &root_url, &input.webdav_username, &password, &input.data_path, &manifest)?;
    Ok(PrepareSyncResult {
        local_file_count: manifest.files.len(),
        local_manifest_path: local_manifest_path.to_string_lossy().to_string(),
        remote_manifest_url: manifest_url,
        remote_initialized: true,
    })
}

#[tauri::command]
fn save_webdav_credentials(input: WebDavCredentialsInput) -> Result<(), String> {
    if input.webdav_url.trim().is_empty() || input.webdav_username.trim().is_empty() {
        return Err("WebDAV server URL and username are required".to_string());
    }
    if input.password.is_empty() {
        return Err("WebDAV password or token is required".to_string());
    }
    webdav_credential_entry(&input.webdav_url, &input.webdav_username)?
        .set_password(&input.password)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_webdav_credentials(webdav_url: String, webdav_username: String) -> Result<(), String> {
    let entry = webdav_credential_entry(&webdav_url, &webdav_username)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(_) => Ok(()),
    }
}

#[tauri::command]
fn test_webdav_connection(input: WebDavConnectionInput) -> Result<WebDavConnectionResult, String> {
    if input.webdav_username.trim().is_empty() {
        return Err("WebDAV username is required".to_string());
    }
    let password = get_webdav_password(&input)?;
    let url = normalize_webdav_url(&input.webdav_url, &input.webdav_remote_dir)?;
    let propfind = Method::from_bytes(b"PROPFIND").map_err(|e| e.to_string())?;
    let response = Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())?
        .request(propfind, &url)
        .basic_auth(input.webdav_username.trim(), Some(password))
        .header("Depth", HeaderValue::from_static("0"))
        .send()
        .map_err(|e| e.to_string())?;

    let status = response.status();
    let message = match status {
        StatusCode::OK | StatusCode::MULTI_STATUS => "WebDAV connection is ready".to_string(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => "WebDAV authentication failed".to_string(),
        StatusCode::NOT_FOUND => "WebDAV remote directory does not exist".to_string(),
        _ => format!("WebDAV server returned {}", status.as_u16()),
    };

    Ok(WebDavConnectionResult {
        ok: status == StatusCode::OK || status == StatusCode::MULTI_STATUS,
        status: status.as_u16().to_string(),
        message,
    })
}

fn default_quick_record_shortcut() -> String {
    "Ctrl+Shift+N".to_string()
}

fn default_hide_quick_record_shortcut() -> String {
    "Ctrl+Shift+H".to_string()
}

fn default_quick_record_x() -> f64 {
    100.0
}

fn default_quick_record_y() -> f64 {
    100.0
}

fn default_config(handle: &tauri::AppHandle) -> AppConfig {
    AppConfig {
        data_path: handle.path().app_data_dir().unwrap().to_str().unwrap().to_string(),
        language: "zh-CN".to_string(),
        theme: "light".to_string(),
        font_family: "Arial".to_string(),
        font_size: 14,
        text_color_light: "#333333".to_string(),
        text_color_dark: "#e5e5e5".to_string(),
        launch_at_login: false,
        quick_record_shortcut: default_quick_record_shortcut(),
        hide_quick_record_shortcut: default_hide_quick_record_shortcut(),
        sync_enabled: false,
        sync_provider: default_sync_provider(),
        webdav_url: String::new(),
        webdav_username: String::new(),
        webdav_remote_dir: default_webdav_remote_dir(),
        sync_interval_seconds: default_sync_interval_seconds(),
        sync_on_startup: true,
        sync_on_change: true,
        sync_conflict_default_action: default_sync_conflict_action(),
    }
}

fn quick_record_cache_dir(handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(handle
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("quick-records"))
}

fn quick_record_cache_path(handle: &tauri::AppHandle, id: &str) -> Result<PathBuf, String> {
    Ok(quick_record_cache_dir(handle)?.join(format!("{}.json", id)))
}

fn read_quick_record_caches(handle: &tauri::AppHandle) -> Vec<QuickRecordCache> {
    let Ok(cache_dir) = quick_record_cache_dir(handle) else {
        return vec![];
    };
    let Ok(entries) = fs::read_dir(cache_dir) else {
        return vec![];
    };

    let mut caches: Vec<QuickRecordCache> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .filter_map(|content| serde_json::from_str::<QuickRecordCache>(&content).ok())
        .collect();
    caches.sort_by(|a, b| a.id.cmp(&b.id));
    caches
}

#[tauri::command]
fn save_quick_record_cache(handle: tauri::AppHandle, cache: QuickRecordCacheInput) -> Result<String, String> {
    let id = cache.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let cache_dir = quick_record_cache_dir(&handle)?;
    fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    let data = QuickRecordCache {
        id: id.clone(),
        content: cache.content,
        temp_folder_name: cache.temp_folder_name,
        x: cache.x,
        y: cache.y,
        width: cache.width,
        height: cache.height,
        pinned: cache.pinned,
    };
    let content = serde_json::to_string(&data).map_err(|e| e.to_string())?;
    fs::write(cache_dir.join(format!("{}.json", id)), content).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
fn get_quick_record_cache(handle: tauri::AppHandle, id: String) -> Result<Option<QuickRecordCache>, String> {
    let path = quick_record_cache_path(&handle, &id)?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map(Some).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_quick_record_cache(handle: tauri::AppHandle, id: String) -> Result<(), String> {
    let path = quick_record_cache_path(&handle, &id)?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_app_config(handle: tauri::AppHandle) -> AppConfig {
    let config_path = handle.path().app_config_dir().unwrap().join("config.json");
    if !config_path.exists() {
        return default_config(&handle);
    }
    let content = match fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return default_config(&handle),
    };
    serde_json::from_str(&content).unwrap_or_else(|_| default_config(&handle))
}

#[tauri::command]
fn save_app_config(handle: tauri::AppHandle, config: AppConfig) -> Result<(), String> {
    let config_dir = handle.path().app_config_dir().unwrap();
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    }
    let config_path = config_dir.join("config.json");
    let content = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    fs::write(config_path, content).map_err(|e| e.to_string())?;
    register_global_shortcuts(&handle, &config).map_err(|e| e.to_string())?;
    let _ = handle.emit("config-changed", ());
    Ok(())
}

#[tauri::command]
fn get_todos(data_path: String) -> Vec<TodoItem> {
    let todos_path = Path::new(&data_path).join("todos.json");
    if todos_path.exists() {
        let content = fs::read_to_string(todos_path).unwrap();
        serde_json::from_str(&content).unwrap_or_else(|_| vec![])
    } else {
        vec![]
    }
}

#[tauri::command]
fn save_todos(app: tauri::AppHandle, data_path: String, todos: Vec<TodoItem>) -> Result<(), String> {
    let data_dir = Path::new(&data_path);
    if !data_dir.exists() {
        fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    }
    let todos_path = data_dir.join("todos.json");
    let content = serde_json::to_string(&todos).map_err(|e| e.to_string())?;
    fs::write(todos_path, content).map_err(|e| e.to_string())?;
    let _ = app.emit("todos-changed", ());
    Ok(())
}

#[tauri::command]
fn create_todo_folder(data_path: String) -> Result<String, String> {
    let folder_name = Uuid::new_v4().to_string();
    let folder_path = Path::new(&data_path).join(&folder_name);
    fs::create_dir_all(&folder_path).map_err(|e| e.to_string())?;
    fs::create_dir_all(folder_path.join("assets")).map_err(|e| e.to_string())?;
    Ok(folder_name)
}

#[tauri::command]
fn delete_todo_folder(data_path: String, folder_name: String) -> Result<(), String> {
    let folder_path = Path::new(&data_path).join(&folder_name);
    if folder_path.exists() {
        fs::remove_dir_all(&folder_path).map_err(|e| format!("删除文件夹失败: {}", e))?;
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
struct OrphanFolder {
    folder_name: String,
    size: u64,
}

fn calculate_dir_size(path: &Path) -> u64 {
    let mut total_size = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    total_size += calculate_dir_size(&entry_path);
                } else if let Ok(metadata) = entry_path.metadata() {
                    total_size += metadata.len();
                }
            }
        }
    }
    total_size
}

#[tauri::command]
fn find_orphan_todo_folders(data_path: String) -> Result<Vec<OrphanFolder>, String> {
    let data_dir = Path::new(&data_path);
    if !data_dir.exists() {
        return Ok(vec![]);
    }

    // 获取所有有效的待办文件夹名
    let todos = get_todos(data_path.clone());
    let valid_folders: std::collections::HashSet<String> = todos
        .iter()
        .map(|todo| todo.folder_name.clone())
        .collect();

    let mut orphan_folders = Vec::new();

    // 扫描数据目录下的所有文件夹
    if let Ok(entries) = fs::read_dir(data_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(folder_name) = path.file_name().and_then(|n| n.to_str()) {
                        // 检查是否在有效文件夹列表中
                        if !valid_folders.contains(folder_name) {
                            let size = calculate_dir_size(&path);
                            orphan_folders.push(OrphanFolder {
                                folder_name: folder_name.to_string(),
                                size,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(orphan_folders)
}

#[tauri::command]
fn save_todo_detail(app: tauri::AppHandle, data_path: String, folder_name: String, content: String) -> Result<(), String> {
    let detail_path = Path::new(&data_path).join(&folder_name).join("content.json");
    fs::write(detail_path, content).map_err(|e| e.to_string())?;
    #[derive(Clone, Serialize)]
    struct Payload { folder_name: String }
    let _ = app.emit("todo-detail-changed", Payload { folder_name });
    Ok(())
}

#[tauri::command]
fn get_todo_detail(data_path: String, folder_name: String) -> Result<String, String> {
    let detail_path = Path::new(&data_path).join(folder_name).join("content.json");
    if detail_path.exists() {
        fs::read_to_string(detail_path).map_err(|e| e.to_string())
    } else {
        Ok("{}".to_string())
    }
}

#[tauri::command]
fn move_data(app: tauri::AppHandle, old_path: String, new_path: String) -> Result<(), String> {
    if old_path == new_path || old_path.is_empty() || new_path.is_empty() {
        return Ok(());
    }

    let old_p = Path::new(&old_path);
    let new_p = Path::new(&new_path);
    
    if !old_p.exists() {
        return Ok(());
    }
    
    if !new_p.exists() {
        fs::create_dir_all(new_p).map_err(|e| format!("Failed to create new directory: {}", e))?;
    }
    
    // 遍历旧路径下的所有文件和文件夹（不迁移 config.json，其属于应用配置）
    for entry in fs::read_dir(old_p).map_err(|e| format!("Failed to read old directory: {}", e))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = path.file_name().ok_or("Invalid file name")?;
        if name == "config.json" {
            continue;
        }
        let dest = new_p.join(name);
        
        if path.is_dir() {
            // 递归移动文件夹
            copy_dir_all(&path, &dest).map_err(|e| format!("Failed to copy directory: {}", e))?;
            fs::remove_dir_all(&path).map_err(|e| format!("Failed to remove old directory: {}", e))?;
        } else {
            fs::copy(&path, &dest).map_err(|e| format!("Failed to copy file: {}", e))?;
            fs::remove_file(&path).map_err(|e| format!("Failed to remove old file: {}", e))?;
        }
    }
    let _ = app.emit("todos-changed", ());
    Ok(())
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

#[tauri::command]
fn get_file_icon(extension: String) -> Result<String, String> {
    #[cfg(windows)]
    {
        use std::env;
        use std::io::Write;
        let ext = extension.trim().to_lowercase();
        if ext.is_empty() {
            return Ok(String::new());
        }
        let safe_ext: String = ext
            .chars()
            .take(20)
            .filter(|c| c.is_ascii_alphanumeric() || *c == '.')
            .collect();
        if safe_ext.is_empty() {
            return Ok(String::new());
        }
        let dummy_path = env::temp_dir().join(format!("tauri_icon_dummy.{}", safe_ext));
        let path_str = dummy_path.to_str().unwrap_or("");
        let created = if !dummy_path.exists() {
            fs::File::create(&dummy_path).ok().map(|mut f| {
                let _ = f.write_all(b"");
                true
            })
        } else {
            Some(true)
        };
        let result = windows_icons::get_icon_base64_by_path(path_str);
        if created == Some(true) && dummy_path.exists() {
            let _ = fs::remove_file(&dummy_path);
        }
        match result {
            Ok(b64) => Ok(b64),
            Err(_) => Ok(String::new()),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = extension;
        Ok(String::new())
    }
}

#[tauri::command]
fn create_new_window(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let n = WINDOW_COUNTER.fetch_add(1, Ordering::Relaxed);
    let label = format!("main-{}", n);
    let parsed = url.parse::<url::Url>().map_err(|e| e.to_string())?;
    let webview_url = WebviewUrl::External(parsed);
    let config = get_app_config(app.clone());
    WebviewWindowBuilder::new(&app, &label, webview_url)
        .title("简易代办")
        .inner_size(800.0, 600.0)
        .background_color(startup_background_color(&config))
        .decorations(false)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn build_todo_quick_record_window(
    app: &tauri::AppHandle,
    config: &AppConfig,
    todo_id: &str,
    todo_title: &str,
    folder_name: &str,
) -> Result<(), String> {
    let n = WINDOW_COUNTER.fetch_add(1, Ordering::Relaxed);
    let label = format!("quick-record-{}", n);
    let url = format!(
        "{}?todoId={}&todoTitle={}&folderName={}",
        app_url(config, "#/quick-record"),
        url::form_urlencoded::byte_serialize(todo_id.as_bytes()).collect::<String>(),
        url::form_urlencoded::byte_serialize(todo_title.as_bytes()).collect::<String>(),
        url::form_urlencoded::byte_serialize(folder_name.as_bytes()).collect::<String>()
    );
    quick_record_debug(&format!("build_todo_window:start label={} todo_id={} folder_name={} url={}", label, todo_id, folder_name, url));
    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title("快捷记录")
        .inner_size(420.0, 520.0)
        .position(100.0, 100.0)
        .min_inner_size(160.0, 120.0)
        .background_color(startup_background_color(config))
        .decorations(false)
        .always_on_top(true)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;
    quick_record_debug(&format!("build_todo_window:built label={}", window.label()));
    Ok(())
}

#[tauri::command]
fn open_todo_quick_record_window(app: tauri::AppHandle, todo_id: String) -> Result<(), String> {
    quick_record_debug(&format!("open_todo_command:start todo_id={}", todo_id));
    let config = get_app_config(app.clone());
    quick_record_debug(&format!("open_todo_command:config theme={} data_path={}", config.theme, config.data_path));
    let todo = get_todos(config.data_path.clone())
        .into_iter()
        .find(|item| item.id == todo_id)
        .ok_or_else(|| "Todo not found".to_string())?;
    quick_record_debug(&format!("open_todo_command:todo_found id={} title={} folder={}", todo.id, todo.title, todo.folder_name));
    app.clone()
        .run_on_main_thread(move || {
            quick_record_debug(&format!("open_todo_command:main_thread_enter id={}", todo.id));
            match build_todo_quick_record_window(&app, &config, &todo.id, &todo.title, &todo.folder_name) {
                Ok(()) => quick_record_debug(&format!("open_todo_command:main_thread_done id={}", todo.id)),
                Err(error) => quick_record_debug(&format!("open_todo_command:main_thread_error id={} error={}", todo.id, error)),
            }
        })
        .map_err(|e| e.to_string())
}

fn build_quick_record_window(app: &tauri::AppHandle, config: &AppConfig, cache: Option<&QuickRecordCache>) -> Result<(), String> {
    let n = WINDOW_COUNTER.fetch_add(1, Ordering::Relaxed);
    let label = format!("quick-record-{}", n);
    let url = match cache {
        Some(cache) => format!("{}?cacheId={}", app_url(config, "#/quick-record"), cache.id),
        None => app_url(config, "#/quick-record"),
    };
    let width = cache.map(|cache| cache.width).unwrap_or(260.0).max(160.0);
    let height = cache.map(|cache| cache.height).unwrap_or(180.0).max(120.0);
    let pinned = cache.map(|cache| cache.pinned).unwrap_or(true);

    WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title("快捷记录")
        .inner_size(width, height)
        .position(
            cache.map(|cache| cache.x).unwrap_or(100.0),
            cache.map(|cache| cache.y).unwrap_or(100.0),
        )
        .min_inner_size(160.0, 120.0)
        .background_color(startup_background_color(config))
        .decorations(false)
        .always_on_top(pinned)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn open_quick_record_window(app: &tauri::AppHandle) -> Result<(), String> {
    let config = get_app_config(app.clone());
    build_quick_record_window(app, &config, None)
}

fn restore_quick_record_windows(app: &tauri::AppHandle) {
    let config = get_app_config(app.clone());
    let caches = read_quick_record_caches(app);
    if caches.is_empty() {
        let _ = build_quick_record_window(app, &config, None);
        return;
    }

    for cache in &caches {
        let _ = build_quick_record_window(app, &config, Some(cache));
    }
}

fn toggle_quick_record_windows(app: &tauri::AppHandle) {
    let mut has_visible_quick_record = false;

    for (_, window) in app.webview_windows() {
        if window.label().starts_with("quick-record-") && window.is_visible().unwrap_or(false) {
            has_visible_quick_record = true;
            let cache_id = Uuid::new_v4().to_string();
            let window_label = window.label().to_string();
            let _ = window.emit("quick-record-cache-and-close", QuickRecordHidePayload { cache_id, window_label });
        }
    }

    if !has_visible_quick_record {
        restore_quick_record_windows(app);
    }
}

fn register_global_shortcuts(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    let _ = app.global_shortcut().unregister_all();

    let quick_record_shortcut = config.quick_record_shortcut.parse::<Shortcut>()?;
    app.global_shortcut().on_shortcut(quick_record_shortcut, |app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            let _ = open_quick_record_window(app);
        }
    })?;

    let hide_quick_record_shortcut = config.hide_quick_record_shortcut.parse::<Shortcut>()?;
    app.global_shortcut().on_shortcut(hide_quick_record_shortcut, |app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            toggle_quick_record_windows(app);
        }
    })?;

    Ok(())
}

fn show_or_create_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }

    let config = get_app_config(app.clone());
    if let Ok(window) = WebviewWindowBuilder::new(app, "main", startup_main_url(&config))
        .title("简易代办")
        .inner_size(800.0, 600.0)
        .background_color(startup_background_color(&config))
        .decorations(false)
        .resizable(true)
        .build()
    {
        attach_destroy_on_close(&window);
        let _ = window.set_focus();
    }
}

fn attach_destroy_on_close(window: &tauri::WebviewWindow) {
    let window_to_destroy = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window_to_destroy.destroy();
        }
    });
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("简易代办")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                show_or_create_main_window(app);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                show_or_create_main_window(app);
            }
        })
        .build(app)?;

    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_or_create_main_window(app);
        }))
        .setup(|app| {
            setup_tray(app)?;
            show_or_create_main_window(app.handle());
            let config = get_app_config(app.handle().clone());
            register_global_shortcuts(app.handle(), &config)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_config,
            save_app_config,
            get_todos,
            save_todos,
            create_todo_folder,
            delete_todo_folder,
            save_todo_detail,
            get_todo_detail,
            move_data,
            get_file_icon,
            create_new_window,
            open_todo_quick_record_window,
            quick_record_debug_log,
            save_quick_record_cache,
            get_quick_record_cache,
            delete_quick_record_cache,
            find_orphan_todo_folders,
            save_webdav_credentials,
            clear_webdav_credentials,
            test_webdav_connection,
            prepare_sync_manifest,
            sync_now,
            list_sync_conflicts,
            resolve_sync_conflict
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
