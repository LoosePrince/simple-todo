// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use tauri::Manager;
use std::fs;
use std::path::Path;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct TodoItem {
    id: String,
    title: String,
    status: String,
    folder_name: String,
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
    }
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
fn save_todos(data_path: String, todos: Vec<TodoItem>) -> Result<(), String> {
    let data_dir = Path::new(&data_path);
    if !data_dir.exists() {
        fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    }
    let todos_path = data_dir.join("todos.json");
    let content = serde_json::to_string(&todos).map_err(|e| e.to_string())?;
    fs::write(todos_path, content).map_err(|e| e.to_string())?;
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
fn save_todo_detail(data_path: String, folder_name: String, content: String) -> Result<(), String> {
    let detail_path = Path::new(&data_path).join(folder_name).join("content.json");
    fs::write(detail_path, content).map_err(|e| e.to_string())?;
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
fn move_data(old_path: String, new_path: String) -> Result<(), String> {
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
        .invoke_handler(tauri::generate_handler![
            get_app_config,
            save_app_config,
            get_todos,
            save_todos,
            create_todo_folder,
            save_todo_detail,
            get_todo_detail,
            move_data,
            get_file_icon
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
