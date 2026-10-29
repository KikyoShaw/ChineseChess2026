use std::sync::Arc;
use tauri::{Manager, State};

mod game;
mod network;
mod session;
mod storage;

use session::{Hub, Settings, Snapshot};

#[tauri::command]
fn get_snapshot(hub: State<'_, Arc<Hub>>) -> Snapshot { hub.snapshot() }

#[tauri::command]
fn new_game(hub: State<'_, Arc<Hub>>, settings: Settings) -> Result<Snapshot, String> {
    hub.new_game(settings)
}

#[tauri::command]
fn make_move(hub: State<'_, Arc<Hub>>, from: usize, to: usize, revision: u64, game_id: u64) -> Result<Snapshot, String> {
    hub.make_move(from, to, revision, game_id)
}

#[tauri::command]
fn game_action(hub: State<'_, Arc<Hub>>, action: String, revision: u64, game_id: u64) -> Result<Snapshot, String> {
    hub.action(&action, revision, game_id)
}

#[tauri::command]
fn update_preferences(hub: State<'_, Arc<Hub>>, sound: bool, show_hints: bool, flipped: bool) -> Result<Snapshot, String> {
    hub.preferences(sound, show_hints, flipped)
}

#[tauri::command]
async fn open_position(hub: State<'_, Arc<Hub>>) -> Result<Snapshot, String> {
    let hub = hub.inner().clone();
    if hub.snapshot().settings.mode == "lan" { return Err("请先离开联机房间再打开局面".into()); }
    match storage::choose_position().await? {
        Some(text) => hub.import(&text),
        None => Ok(hub.snapshot()),
    }
}

#[tauri::command]
async fn save_position(hub: State<'_, Arc<Hub>>) -> Result<Option<String>, String> {
    let fen = hub.fen();
    storage::save_position(fen).await
}

#[tauri::command]
async fn host_lan(hub: State<'_, Arc<Hub>>, port: u16, settings: Settings) -> Result<Snapshot, String> {
    let hub = hub.inner().clone();
    if port == 0 { return Err("端口应为 1～65535".into()); }
    settings.validate()?;
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await
        .map_err(|error| format!("无法创建房间，请检查端口是否被占用：{error}"))?;
    hub.begin_host(listener, settings, port)
}

#[tauri::command]
fn join_lan(hub: State<'_, Arc<Hub>>, address: String, port: u16) -> Result<Snapshot, String> {
    hub.begin_join(address, port)
}

#[tauri::command]
fn leave_lan(hub: State<'_, Arc<Hub>>) -> Result<Snapshot, String> { hub.leave() }

#[tauri::command]
fn respond_request(hub: State<'_, Arc<Hub>>, id: u64, accept: bool, revision: u64, game_id: u64) -> Result<Snapshot, String> {
    hub.respond(id, accept, revision, game_id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let directory = app.path().app_config_dir()?;
            let hub = Hub::new(app.handle().clone(), directory);
            app.manage(hub.clone());
            hub.start_clock();
            hub.refresh();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot, new_game, make_move, game_action, update_preferences,
            open_position, save_position, host_lan, join_lan, leave_lan, respond_request,
        ])
        .build(tauri::generate_context!())
        .expect("无法启动中国象棋")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                app.state::<Arc<Hub>>().shutdown();
            }
        });
}
