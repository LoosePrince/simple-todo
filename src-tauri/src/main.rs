// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::Color,
    Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use uuid::Uuid;

static WINDOW_COUNTER: AtomicU64 = AtomicU64::new(0);
const STARTUP_BACKGROUND_COLOR: Color = Color(255, 255, 255, 255);

#[derive(Serialize, Deserialize, Debug, Clone)]
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
    WebviewWindowBuilder::new(&app, &label, webview_url)
        .title("简易代办")
        .inner_size(800.0, 600.0)
        .background_color(STARTUP_BACKGROUND_COLOR)
        .decorations(false)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn build_quick_record_window(app: &tauri::AppHandle, cache: Option<&QuickRecordCache>) -> Result<(), String> {
    let n = WINDOW_COUNTER.fetch_add(1, Ordering::Relaxed);
    let label = format!("quick-record-{}", n);
    let url = match cache {
        Some(cache) => format!("index.html#/quick-record?cacheId={}", cache.id),
        None => "index.html#/quick-record".to_string(),
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
        .background_color(STARTUP_BACKGROUND_COLOR)
        .decorations(false)
        .always_on_top(pinned)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn open_quick_record_window(app: &tauri::AppHandle) -> Result<(), String> {
    build_quick_record_window(app, None)
}

fn restore_quick_record_windows(app: &tauri::AppHandle) {
    let caches = read_quick_record_caches(app);
    if caches.is_empty() {
        let _ = open_quick_record_window(app);
        return;
    }

    for cache in &caches {
        let _ = build_quick_record_window(app, Some(cache));
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

    if let Ok(window) = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html#/".into()))
        .title("简易代办")
        .inner_size(800.0, 600.0)
        .background_color(STARTUP_BACKGROUND_COLOR)
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
            if let Some(window) = app.get_webview_window("main") {
                attach_destroy_on_close(&window);
            }
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
            save_quick_record_cache,
            get_quick_record_cache,
            delete_quick_record_cache,
            find_orphan_todo_folders
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
