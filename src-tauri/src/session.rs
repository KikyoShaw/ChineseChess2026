use std::{path::PathBuf, sync::{Arc, Mutex, MutexGuard, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, watch};
use crate::{game::{self, ChessMove, Game, MoveRecord, Outcome, Piece, Position, Side}, network::{self, Intent, Wire}, storage};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub mode: String,
    pub human_side: Side,
    pub first_side: Side,
    pub difficulty: u8,
    pub step_seconds: u32,
    pub sound: bool,
    pub show_hints: bool,
    pub flipped: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { mode: "computer".into(), human_side: Side::Red, first_side: Side::Red,
            difficulty: 2, step_seconds: 30, sound: true, show_hints: true, flipped: false }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !["local", "computer", "lan"].contains(&self.mode.as_str()) { return Err("未知对弈模式".into()); }
        if !(1..=3).contains(&self.difficulty) { return Err("请选择有效的 AI 难度".into()); }
        if self.step_seconds > 3600 { return Err("每步时限应为 0～3600 秒".into()); }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkView {
    pub status: String,
    pub role: Option<String>,
    pub local_side: Option<Side>,
    pub address: String,
    pub message: String,
}

impl Default for NetworkView {
    fn default() -> Self {
        Self { status: "offline".into(), role: None, local_side: None, address: String::new(), message: "本地对局".into() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingRequest { pub id: u64, pub kind: String, pub from: Side }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionSignal { pub id: u64, pub sound: Option<String> }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: u64,
    pub game_id: u64,
    pub board: Vec<Option<Piece>>,
    pub turn: Side,
    pub history: Vec<MoveRecord>,
    pub result: Option<Outcome>,
    pub in_check: bool,
    pub legal_moves: Vec<ChessMove>,
    pub settings: Settings,
    pub remaining_ms: Option<u64>,
    pub thinking: bool,
    pub network: NetworkView,
    pub pending_request: Option<PendingRequest>,
    pub action: ActionSignal,
}

struct Session {
    game: Game,
    settings: Settings,
    revision: u64,
    game_id: u64,
    position_revision: u64,
    deadline: Option<Instant>,
    thinking: bool,
    ai_cancel: Arc<AtomicBool>,
    network: NetworkView,
    connection_id: u64,
    network_cancel: Option<watch::Sender<bool>>,
    sender: Option<mpsc::Sender<Wire>>,
    remote: Option<Snapshot>,
    authority_revision: u64,
    request_counter: u64,
    last_peer_request: u64,
    pending: Option<PendingRequest>,
    pending_since: Option<Instant>,
    action: ActionSignal,
}

impl Session {
    fn new(settings: Settings) -> Self {
        let game = Game::new(settings.first_side);
        let mut session = Self { game, settings, revision: 1, game_id: 1, position_revision: 1,
            deadline: None, thinking: false, ai_cancel: Arc::new(AtomicBool::new(false)),
            network: NetworkView::default(), connection_id: 0, network_cancel: None, sender: None,
            remote: None, authority_revision: 0, request_counter: 0, last_peer_request: 0,
            pending: None, pending_since: None, action: ActionSignal { id: 0, sound: None } };
        session.reset_clock();
        session
    }

    fn guest(&self) -> bool { self.network.role.as_deref() == Some("guest") }
    fn running(&self) -> bool {
        self.game.result.is_none() && (self.settings.mode != "lan" || self.network.status == "connected")
    }
    fn reset_clock(&mut self) {
        self.deadline = if self.running() && self.settings.step_seconds > 0 {
            Some(Instant::now() + Duration::from_secs(self.settings.step_seconds.into()))
        } else { None };
    }
    fn remaining(&self) -> Option<u64> {
        self.deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()).as_millis() as u64)
    }
    fn touch(&mut self, sound: Option<&str>) {
        self.revision += 1;
        if let Some(sound) = sound {
            self.action.id += 1;
            self.action.sound = Some(sound.into());
        }
    }
    fn cancel_ai(&mut self) {
        self.ai_cancel.store(true, Ordering::Relaxed);
        self.thinking = false;
    }
    fn position_changed(&mut self, sound: Option<&str>) {
        self.cancel_ai();
        self.position_revision += 1;
        self.pending = None;
        self.pending_since = None;
        self.reset_clock();
        self.touch(sound);
    }
    fn result_sound(&self) -> &'static str {
        match self.game.result.as_ref().and_then(|result| result.winner) {
            None => "Draw",
            Some(winner) if self.settings.mode == "local" || winner == self.settings.human_side => "Win",
            Some(_) => "Loss",
        }
    }
    fn apply(&mut self, mv: ChessMove) -> Result<(), String> {
        self.game.apply_move(mv)?;
        let sound = if self.game.result.is_some() { self.result_sound() }
            else if self.game.history.last().is_some_and(|record| record.checking) { "AttackKing" }
            else if self.game.history.last().is_some_and(|record| record.captured.is_some()) { "Eat" }
            else { "Move" };
        self.position_changed(Some(sound));
        Ok(())
    }
    fn effective_game_id(&self) -> u64 { self.remote.as_ref().filter(|_| self.guest()).map_or(self.game_id, |remote| remote.game_id) }
    fn validate_version(&self, revision: u64, game_id: u64) -> Result<(), String> {
        if revision != self.revision || game_id != self.effective_game_id() {
            return Err("棋局已更新，请重新操作".into());
        }
        Ok(())
    }
    fn validate_turn(&self) -> Result<(), String> {
        if !self.running() { return Err("当前不能落子".into()); }
        let side = self.game.position.turn;
        if self.settings.mode == "computer" && side != self.settings.human_side { return Err("请等待电脑落子".into()); }
        if self.settings.mode == "lan" && self.network.local_side != Some(side) { return Err("请等待对方落子".into()); }
        Ok(())
    }
    fn snapshot(&self) -> Snapshot {
        if let Some(remote) = self.remote.as_ref().filter(|_| self.guest()) {
            let mut result = remote.clone();
            result.revision = self.revision;
            result.settings = self.settings.clone();
            result.network = self.network.clone();
            result.action = self.action.clone();
            result.remaining_ms = if self.network.status == "connected" && remote.result.is_none() {
                self.remaining()
            } else { remote.remaining_ms };
            return result;
        }
        Snapshot { revision: self.revision, game_id: self.game_id,
            board: self.game.position.board.clone(), turn: self.game.position.turn,
            history: self.game.history.clone(), result: self.game.result.clone(),
            in_check: self.game.position.in_check(self.game.position.turn),
            legal_moves: if self.game.result.is_none() { self.game.position.legal_moves() } else { Vec::new() },
            settings: self.settings.clone(), remaining_ms: self.remaining(), thinking: self.thinking,
            network: self.network.clone(), pending_request: self.pending.clone(), action: self.action.clone() }
    }
    fn stop_network(&mut self) {
        self.connection_id += 1;
        if let Some(cancel) = self.network_cancel.take() { let _ = cancel.send(true); }
        self.sender = None;
        self.authority_revision = 0;
        self.last_peer_request = 0;
    }
    fn expired(&mut self) -> bool {
        if !self.guest() && self.running() && self.deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            self.game.resign(self.game.position.turn, "超时判负");
            let sound = self.result_sound();
            self.position_changed(Some(sound));
            true
        } else { false }
    }
    fn reset_game(&mut self) {
        self.game = Game::new(self.settings.first_side);
        self.game_id += 1;
        self.position_changed(Some("NewGame"));
    }
    fn local_action(&mut self, action: &str, side: Side) -> Result<(), String> {
        match action {
            "undo" => {
                if self.game.history.is_empty() { return Err("还没有可以撤销的着法".into()); }
                let plies = if self.settings.mode == "computer" {
                    if !self.game.history.iter().any(|entry| entry.side == self.settings.human_side) {
                        return Err("尚无可以悔棋的玩家着法".into());
                    }
                    if self.game.position.turn == self.settings.human_side { 2 } else { 1 }
                } else { 2 };
                self.game.undo(plies.min(self.game.history.len()))?;
                self.position_changed(Some("Move"));
            }
            "draw" => {
                if self.game.result.is_some() { return Err("本局已经结束".into()); }
                self.game.draw("双方同意和棋");
                self.position_changed(Some("Draw"));
            }
            "resign" => {
                if self.game.result.is_some() { return Err("本局已经结束".into()); }
                self.game.resign(side, "认输");
                let sound = self.result_sound();
                self.position_changed(Some(sound));
            }
            "restart" => self.reset_game(),
            _ => return Err("未知操作".into()),
        }
        Ok(())
    }
    fn lan_action(&mut self, action: &str, side: Side) -> Result<(), String> {
        if self.network.status != "connected" { return Err("请先连接对手".into()); }
        if action == "resign" { return self.local_action(action, side); }
        if !["undo", "draw", "restart"].contains(&action) { return Err("未知操作".into()); }
        if self.pending.is_some() { return Err("请先处理当前协商请求".into()); }
        if action == "undo" && self.game.history.is_empty() { return Err("还没有可以撤销的着法".into()); }
        if action == "draw" && self.game.result.is_some() { return Err("本局已经结束".into()); }
        self.request_counter += 1;
        self.pending = Some(PendingRequest { id: self.request_counter, kind: action.into(), from: side });
        self.pending_since = Some(Instant::now());
        self.touch(None);
        Ok(())
    }
    fn respond(&mut self, id: u64, accept: bool, side: Side) -> Result<(), String> {
        let pending = self.pending.clone().ok_or("请求已失效")?;
        if pending.id != id { return Err("请求已失效".into()); }
        if pending.from == side { return Err("请等待对方答复".into()); }
        self.pending = None;
        self.pending_since = None;
        if accept {
            self.local_action(&pending.kind, pending.from)?;
            self.network.message = "双方已同意请求".into();
        } else {
            self.network.message = "对方已拒绝请求".into();
            self.touch(None);
        }
        Ok(())
    }
    fn peer_intent(&mut self, intent: Intent) -> Result<(), String> {
        let (revision, game_id, request_id) = match &intent {
            Intent::Move { revision, game_id, request_id, .. }
            | Intent::Action { revision, game_id, request_id, .. }
            | Intent::Respond { revision, game_id, request_id, .. } => (*revision, *game_id, *request_id),
        };
        if request_id <= self.last_peer_request { return Err("重复的操作请求".into()); }
        self.last_peer_request = request_id;
        self.validate_version(revision, game_id)?;
        let side = self.network.local_side.ok_or("房间未准备好")?.other();
        match intent {
            Intent::Move { from, to, .. } => {
                if self.game.result.is_some() || self.game.position.turn != side { return Err("现在还未轮到你落子".into()); }
                self.apply(ChessMove { from, to })
            }
            Intent::Action { action, .. } => self.lan_action(&action, side),
            Intent::Respond { id, accept, .. } => self.respond(id, accept, side),
        }
    }
}

pub struct Hub {
    inner: Mutex<Session>,
    app: AppHandle,
    directory: PathBuf,
}

impl Hub {
    pub fn new(app: AppHandle, directory: PathBuf) -> Arc<Self> {
        let settings = storage::load_settings(&directory);
        Arc::new(Self { inner: Mutex::new(Session::new(settings)), app, directory })
    }
    fn lock(&self) -> MutexGuard<'_, Session> { self.inner.lock().unwrap_or_else(|error| error.into_inner()) }
    pub fn snapshot(&self) -> Snapshot { self.lock().snapshot() }
    pub fn shutdown(&self) {
        let mut session = self.lock();
        session.cancel_ai();
        session.stop_network();
    }
    pub fn fen(&self) -> String {
        let session = self.lock();
        let snapshot = session.snapshot();
        game::fen::format(&Position { board: snapshot.board, turn: snapshot.turn })
    }
    pub fn refresh(self: &Arc<Self>) -> Snapshot {
        let (snapshot, sender, job) = {
            let mut session = self.lock();
            let job = if session.running() && session.settings.mode == "computer"
                && session.game.position.turn != session.settings.human_side && !session.thinking {
                session.ai_cancel = Arc::new(AtomicBool::new(false));
                session.thinking = true;
                session.touch(None);
                let budget = session.remaining().map(|milliseconds| {
                    Duration::from_millis(milliseconds.saturating_sub(80).max(1))
                });
                Some((session.game.position.clone(), session.settings.difficulty, session.ai_cancel.clone(),
                    session.game_id, session.position_revision, budget))
            } else { None };
            let sender = if !session.guest() { session.sender.clone() } else { None };
            (session.snapshot(), sender, job)
        };
        let _ = self.app.emit("game:snapshot", &snapshot);
        if let Some(sender) = sender {
            if sender.try_send(Wire::State { snapshot: Box::new(snapshot.clone()) }).is_err() {
                let token = self.lock().connection_id;
                self.network_lost(token, "连接发送队列已关闭或拥塞，请重新连接".into());
            }
        }
        if let Some((position, difficulty, cancel, game_id, revision, budget)) = job {
            let hub = self.clone();
            tauri::async_runtime::spawn(async move {
                let flag = cancel.clone();
                let result = tauri::async_runtime::spawn_blocking(move || {
                    match budget {
                        Some(budget) => game::ai::search_with_budget(position, difficulty, flag, budget),
                        None => game::ai::search(position, difficulty, flag),
                    }
                }).await;
                let should_publish = {
                    let mut session = hub.lock();
                    if cancel.load(Ordering::Relaxed) || session.game_id != game_id
                        || session.position_revision != revision || session.settings.mode != "computer"
                        || !session.running() || session.game.position.turn == session.settings.human_side {
                        false
                    } else if session.expired() { true }
                    else {
                        session.thinking = false;
                        match result {
                            Ok(Some(mv)) => {
                                if session.apply(mv).is_err() {
                                    if let Some(fallback) = session.game.position.legal_moves().first().copied() {
                                        let _ = session.apply(fallback);
                                    } else {
                                        session.game.result = Game::from_position(session.game.position.clone()).result;
                                        session.position_changed(None);
                                    }
                                }
                            }
                            _ => {
                                // Keep a legal fallback if the worker was interrupted by a runtime failure.
                                if let Some(mv) = session.game.position.legal_moves().first().copied() {
                                    let _ = session.apply(mv);
                                } else {
                                    session.game.result = Game::from_position(session.game.position.clone()).result;
                                    session.position_changed(None);
                                }
                            }
                        }
                        true
                    }
                };
                if should_publish { hub.refresh(); }
            });
        }
        snapshot
    }
    pub fn start_clock(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;
                let Some(hub) = weak.upgrade() else { break; };
                let changed = {
                    let mut session = hub.lock();
                    if session.expired() { true }
                    else if !session.guest() && session.pending_since.is_some_and(|at| at.elapsed() >= Duration::from_secs(30)) {
                        session.pending = None;
                        session.pending_since = None;
                        session.network.message = "协商请求已超时".into();
                        session.touch(None);
                        true
                    } else { false }
                };
                if changed { hub.refresh(); }
            }
        });
    }
    pub fn new_game(self: &Arc<Self>, settings: Settings) -> Result<Snapshot, String> {
        settings.validate()?;
        {
            let mut session = self.lock();
            if session.settings.mode == "lan" { return Err("请先离开联机房间，或向对方请求重开".into()); }
            if settings.mode == "lan" { return Err("请通过联机窗口创建或加入房间".into()); }
            storage::save_settings(&self.directory, &settings)?;
            session.settings = settings;
            session.reset_game();
        }
        Ok(self.refresh())
    }
    pub fn preferences(self: &Arc<Self>, sound: bool, show_hints: bool, flipped: bool) -> Result<Snapshot, String> {
        {
            let mut session = self.lock();
            let mut settings = session.settings.clone();
            settings.sound = sound;
            settings.show_hints = show_hints;
            settings.flipped = flipped;
            storage::save_settings(&self.directory, &settings)?;
            session.settings = settings;
            session.touch(None);
        }
        Ok(self.refresh())
    }
    pub fn import(self: &Arc<Self>, text: &str) -> Result<Snapshot, String> {
        let position = game::fen::parse(text)?;
        {
            let mut session = self.lock();
            if session.settings.mode == "lan" { return Err("请先离开联机房间再打开局面".into()); }
            session.game = Game::from_position(position);
            session.game_id += 1;
            session.position_changed(Some("NewGame"));
        }
        Ok(self.refresh())
    }
    pub fn make_move(self: &Arc<Self>, from: usize, to: usize, revision: u64, game_id: u64) -> Result<Snapshot, String> {
        let result = {
            let mut session = self.lock();
            session.validate_version(revision, game_id)?;
            if session.guest() {
                let remote = session.remote.as_ref().ok_or("请等待房主同步棋局")?;
                if session.network.status != "connected" || remote.result.is_some()
                    || session.network.local_side != Some(remote.turn) { return Err("当前不能落子".into()); }
                if !remote.legal_moves.contains(&ChessMove { from, to }) { return Err("这步棋不合法".into()); }
                session.request_counter += 1;
                let intent = Intent::Move { from, to, game_id, revision: session.authority_revision, request_id: session.request_counter };
                session.sender.as_ref().ok_or("连接已断开")?.try_send(Wire::Intent { intent }).map_err(|_| "连接繁忙，请稍后重试".to_string())?;
                return Ok(session.snapshot());
            }
            if session.expired() { Err("本步已超时".into()) }
            else { session.validate_turn().and_then(|_| session.apply(ChessMove { from, to })) }
        };
        let snapshot = self.refresh();
        result.map(|_| snapshot)
    }
    pub fn action(self: &Arc<Self>, action: &str, revision: u64, game_id: u64) -> Result<Snapshot, String> {
        let result = {
            let mut session = self.lock();
            session.validate_version(revision, game_id)?;
            if session.guest() {
                if session.network.status != "connected" { return Err("请先连接对手".into()); }
                session.request_counter += 1;
                let intent = Intent::Action { action: action.into(), revision: session.authority_revision, game_id, request_id: session.request_counter };
                session.sender.as_ref().ok_or("连接已断开")?.try_send(Wire::Intent { intent }).map_err(|_| "连接繁忙，请稍后重试".to_string())?;
                return Ok(session.snapshot());
            }
            session.expired();
            let side = if session.settings.mode == "local" { session.game.position.turn }
                else { session.settings.human_side };
            if session.settings.mode == "lan" { session.lan_action(action, side) }
            else { session.local_action(action, side) }
        };
        let snapshot = self.refresh();
        result.map(|_| snapshot)
    }
    pub fn respond(self: &Arc<Self>, id: u64, accept: bool, revision: u64, game_id: u64) -> Result<Snapshot, String> {
        let result = {
            let mut session = self.lock();
            session.validate_version(revision, game_id)?;
            if session.network.status != "connected" { return Err("连接已断开".into()); }
            if session.guest() {
                session.request_counter += 1;
                let intent = Intent::Respond { id, accept, revision: session.authority_revision, game_id, request_id: session.request_counter };
                session.sender.as_ref().ok_or("连接已断开")?.try_send(Wire::Intent { intent }).map_err(|_| "连接繁忙，请稍后重试".to_string())?;
                return Ok(session.snapshot());
            }
            session.expired();
            let side = session.network.local_side.ok_or("房间尚未准备好")?;
            session.respond(id, accept, side)
        };
        let snapshot = self.refresh();
        result.map(|_| snapshot)
    }
    pub fn begin_host(self: &Arc<Self>, listener: tokio::net::TcpListener, mut settings: Settings, port: u16) -> Result<Snapshot, String> {
        settings.mode = "lan".into();
        settings.validate()?;
        let (token, receiver) = {
            let mut session = self.lock();
            if session.settings.mode == "lan" { return Err("请先离开当前房间".into()); }
            storage::save_settings(&self.directory, &settings)?;
            session.stop_network();
            session.settings = settings;
            session.remote = None;
            session.network = NetworkView { status: "hosting".into(), role: Some("host".into()),
                local_side: Some(session.settings.human_side), address: format!("0.0.0.0:{port}"),
                message: format!("等待对手加入，请告知对方本机局域网 IP 和端口 {port}") };
            session.reset_game();
            let (cancel, receiver) = watch::channel(false);
            session.network_cancel = Some(cancel);
            (session.connection_id, receiver)
        };
        network::host(self.clone(), listener, token, receiver);
        Ok(self.refresh())
    }
    pub fn begin_join(self: &Arc<Self>, address: String, port: u16) -> Result<Snapshot, String> {
        if address.trim().is_empty() || address.len() > 253 || port == 0 { return Err("请输入有效的主机地址和端口".into()); }
        let (token, receiver) = {
            let mut session = self.lock();
            if session.settings.mode == "lan" { return Err("请先离开当前房间".into()); }
            session.stop_network();
            session.cancel_ai();
            session.settings.mode = "lan".into();
            session.remote = None;
            session.network = NetworkView { status: "connecting".into(), role: Some("guest".into()),
                local_side: None, address: format!("{}:{port}", address.trim()), message: "正在连接房主…".into() };
            session.pending = None;
            session.pending_since = None;
            session.deadline = None;
            session.touch(None);
            let (cancel, receiver) = watch::channel(false);
            session.network_cancel = Some(cancel);
            (session.connection_id, receiver)
        };
        network::join(self.clone(), address.trim().into(), port, token, receiver);
        Ok(self.refresh())
    }
    pub fn leave(self: &Arc<Self>) -> Result<Snapshot, String> {
        {
            let mut session = self.lock();
            let mut settings = session.settings.clone();
            settings.mode = "local".into();
            storage::save_settings(&self.directory, &settings)?;
            let snapshot = session.snapshot();
            if session.guest() && session.remote.is_some() {
                session.game = Game::from_position(Position { board: snapshot.board, turn: snapshot.turn });
                session.game.result = snapshot.result;
            }
            session.stop_network();
            session.remote = None;
            session.settings = settings;
            session.network = NetworkView { message: "已转为本地双人，当前局面保留；访客棋谱从此局面重新记录".into(), ..NetworkView::default() };
            session.game_id += 1;
            session.position_changed(None);
        }
        Ok(self.refresh())
    }
    pub fn network_is_current(&self, token: u64) -> bool { self.lock().connection_id == token }
    pub fn network_connected(self: &Arc<Self>, token: u64, sender: mpsc::Sender<Wire>, address: String) -> bool {
        {
            let mut session = self.lock();
            if session.connection_id != token || session.settings.mode != "lan" { return false; }
            session.sender = Some(sender);
            session.network.status = "connected".into();
            session.network.address = address;
            session.network.message = "已连接，祝对弈愉快".into();
            if !session.guest() { session.reset_clock(); }
            session.touch(None);
        }
        self.refresh();
        true
    }
    pub fn receive_peer(self: &Arc<Self>, token: u64, intent: Intent) {
        let error = {
            let mut session = self.lock();
            if session.connection_id != token || session.guest() || session.network.status != "connected" { return; }
            session.expired();
            session.peer_intent(intent).err().map(|message| (session.sender.clone(), message))
        };
        if let Some((Some(sender), message)) = error { let _ = sender.try_send(Wire::Error { message }); }
        self.refresh();
    }
    pub fn receive_snapshot(self: &Arc<Self>, token: u64, snapshot: Snapshot) -> Result<(), String> {
        {
            let mut session = self.lock();
            if session.connection_id != token || !session.guest() { return Ok(()); }
            if snapshot.revision <= session.authority_revision { return Ok(()); }
            if snapshot.board.len() != 90 || snapshot.legal_moves.iter().any(|mv| mv.from >= 90 || mv.to >= 90)
                || snapshot.history.iter().any(|record| record.from >= 90 || record.to >= 90) {
                return Err("对方发送了无效棋局".into());
            }
            snapshot.settings.validate()?;
            let host_side = snapshot.network.local_side.ok_or("房主未分配阵营")?;
            session.authority_revision = snapshot.revision;
            session.settings.mode = "lan".into();
            session.settings.human_side = host_side.other();
            session.settings.first_side = snapshot.settings.first_side;
            session.settings.step_seconds = snapshot.settings.step_seconds;
            session.settings.difficulty = snapshot.settings.difficulty;
            session.network.local_side = Some(host_side.other());
            session.network.message = snapshot.network.message.clone();
            session.deadline = snapshot.remaining_ms.filter(|_| snapshot.result.is_none())
                .map(|remaining| Instant::now() + Duration::from_millis(remaining));
            let new_sound = session.remote.as_ref().is_some_and(|old| old.action.id != snapshot.action.id);
            if new_sound {
                let sound = if let Some(result) = &snapshot.result {
                    match result.winner {
                        None => Some("Draw".to_string()),
                        Some(winner) if Some(winner) == session.network.local_side => Some("Win".to_string()),
                        Some(_) => Some("Loss".to_string()),
                    }
                } else { snapshot.action.sound.clone() };
                session.touch(sound.as_deref());
            } else { session.touch(None); }
            session.remote = Some(snapshot);
        }
        self.refresh();
        Ok(())
    }
    pub fn network_notice(self: &Arc<Self>, token: u64, message: String) {
        {
            let mut session = self.lock();
            if session.connection_id != token { return; }
            session.network.message = message;
            session.touch(None);
        }
        self.refresh();
    }
    pub fn network_lost(self: &Arc<Self>, token: u64, message: String) {
        {
            let mut session = self.lock();
            if session.connection_id != token || session.network.status == "disconnected" { return; }
            if session.guest() {
                let remaining = session.remaining();
                if let Some(remote) = session.remote.as_mut() { remote.remaining_ms = remaining; remote.pending_request = None; }
            }
            session.stop_network();
            session.network.status = "disconnected".into();
            session.network.message = message;
            session.pending = None;
            session.pending_since = None;
            session.deadline = None;
            session.touch(None);
        }
        let _ = self.app.emit("game:snapshot", self.snapshot());
    }
}
