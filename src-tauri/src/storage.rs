use std::{fs, path::{Path, PathBuf}};
use serde::{Deserialize, Serialize};
use crate::session::Settings;

#[derive(Serialize, Deserialize)]
struct Preferences {
    version: u32,
    settings: Settings,
}

pub fn load_settings(directory: &Path) -> Settings {
    let loaded = fs::read(directory.join("settings.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Preferences>(&bytes).ok())
        .filter(|value| value.version == 1)
        .map(|value| value.settings)
        .filter(|value| value.validate().is_ok());
    let mut settings = loaded.unwrap_or_default();
    // A persisted LAN preference cannot restore a live connection.
    if settings.mode == "lan" {
        settings.mode = "computer".into();
    }
    settings
}

pub fn save_settings(directory: &Path, settings: &Settings) -> Result<(), String> {
    settings.validate()?;
    fs::create_dir_all(directory).map_err(|e| format!("无法创建设置目录：{e}"))?;
    let bytes = serde_json::to_vec_pretty(&Preferences { version: 1, settings: settings.clone() })
        .map_err(|e| format!("无法保存设置：{e}"))?;
    let temporary = directory.join("settings.json.tmp");
    fs::write(&temporary, bytes).map_err(|e| format!("无法写入设置：{e}"))?;
    fs::rename(&temporary, directory.join("settings.json"))
        .map_err(|e| format!("无法更新设置：{e}"))
}

pub async fn choose_position() -> Result<Option<String>, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("打开象棋局面")
        .add_filter("象棋局面", &["fen", "txt", "chess"])
        .add_filter("所有文件", &["*"])
        .pick_file().await;
    let Some(file) = file else { return Ok(None); };
    let path: PathBuf = file.path().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let metadata = fs::metadata(&path).map_err(|e| format!("无法打开文件：{e}"))?;
        if metadata.len() > 16 * 1024 {
            return Err("局面文件过大，请选择 FEN 文本文件".into());
        }
        fs::read_to_string(path)
            .map(|text| Some(text.trim_start_matches('\u{feff}').trim().to_owned()))
            .map_err(|e| format!("无法读取 UTF-8 局面文件：{e}"))
    }).await.map_err(|e| format!("读取文件失败：{e}"))?
}

pub async fn save_position(fen: String) -> Result<Option<String>, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("保存当前象棋局面")
        .set_file_name("象棋局面.fen")
        .add_filter("象棋局面", &["fen"])
        .save_file().await;
    let Some(file) = file else { return Ok(None); };
    let path = file.path().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        fs::write(&path, format!("{fen}\n")).map_err(|e| format!("无法保存局面：{e}"))?;
        Ok(Some(path.to_string_lossy().into_owned()))
    }).await.map_err(|e| format!("保存文件失败：{e}"))?
}
