use std::{
    future::{poll_fn, Future},
    io::{self, Write},
    pin::pin,
    sync::Arc,
    task::Poll,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{tcp::OwnedReadHalf, tcp::OwnedWriteHalf, TcpListener, TcpStream},
    sync::{mpsc, watch},
    time::{self, Instant, MissedTickBehavior},
};

use crate::session::{Hub, Snapshot};

pub const PROTOCOL: u16 = 2;

const MAX_FRAME: usize = 64 * 1024;
const MAX_STATE: usize = 8 * 1024 * 1024;
const CHUNK_SIZE: usize = 8192;
const SEND_CAPACITY: usize = 128;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const FRAME_TIMEOUT: Duration = Duration::from_secs(35);
const PING_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Intent {
    Move {
        from: usize,
        to: usize,
        game_id: u64,
        revision: u64,
        request_id: u64,
    },
    Action {
        action: String,
        game_id: u64,
        revision: u64,
        request_id: u64,
    },
    Respond {
        id: u64,
        accept: bool,
        game_id: u64,
        revision: u64,
        request_id: u64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Wire {
    Hello { protocol: u16 },
    State { snapshot: Box<Snapshot> },
    Intent { intent: Intent },
    Error { message: String },
    Ping,
    Pong,
    #[doc(hidden)]
    StateStart { id: u64, total: usize },
    #[doc(hidden)]
    StateChunk {
        id: u64,
        index: usize,
        data: Vec<u8>,
    },
}

#[derive(Clone, Copy)]
enum Role {
    Host,
    Guest,
}

pub fn host(
    hub: Arc<Hub>,
    listener: TcpListener,
    token: u64,
    mut cancel: watch::Receiver<bool>,
) {
    tauri::async_runtime::spawn(async move {
        let result = host_task(&hub, listener, token, &mut cancel).await;
        if let Err(message) = result {
            if is_current(&hub, token, &cancel) {
                hub.network_lost(token, message);
            }
        }
    });
}

pub fn join(
    hub: Arc<Hub>,
    address: String,
    port: u16,
    token: u64,
    mut cancel: watch::Receiver<bool>,
) {
    tauri::async_runtime::spawn(async move {
        let result = join_task(&hub, address, port, token, &mut cancel).await;
        if let Err(message) = result {
            if is_current(&hub, token, &cancel) {
                hub.network_lost(token, message);
            }
        }
    });
}

fn is_current(hub: &Arc<Hub>, token: u64, cancel: &watch::Receiver<bool>) -> bool {
    !*cancel.borrow() && !cancel.has_changed().is_err() && hub.network_is_current(token)
}

async fn wait_for_cancel(cancel: &mut watch::Receiver<bool>) {
    loop {
        if *cancel.borrow_and_update() {
            return;
        }
        if cancel.changed().await.is_err() {
            return;
        }
    }
}

async fn cancellable<F: Future>(cancel: &mut watch::Receiver<bool>, work: F) -> Option<F::Output> {
    let mut work = pin!(work);
    let mut cancelled = pin!(wait_for_cancel(cancel));
    poll_fn(|cx| {
        if cancelled.as_mut().poll(cx).is_ready() {
            Poll::Ready(None)
        } else {
            work.as_mut().poll(cx).map(Some)
        }
    })
    .await
}

async fn host_task(
    hub: &Arc<Hub>,
    listener: TcpListener,
    token: u64,
    cancel: &mut watch::Receiver<bool>,
) -> Result<(), String> {
    loop {
        if !is_current(hub, token, cancel) {
            return Ok(());
        }
        let Some(accepted) = cancellable(cancel, listener.accept()).await else {
            return Ok(());
        };
        let (mut stream, address) = accepted.map_err(|error| format!("接受连接失败：{error}"))?;
        if !is_current(hub, token, cancel) {
            return Ok(());
        }
        if let Err(error) = stream.set_nodelay(true) {
            if is_current(hub, token, cancel) {
                hub.network_notice(token, format!("无法初始化来自 {address} 的连接：{error}"));
            }
            continue;
        }
        let Some(result) = cancellable(
            cancel,
            time::timeout(HANDSHAKE_TIMEOUT, handshake(&mut stream)),
        )
        .await
        else {
            return Ok(());
        };
        let error = match result {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error),
            Err(_) => Some("版本协商超时".to_owned()),
        };
        if let Some(error) = error {
            if is_current(hub, token, cancel) {
                hub.network_notice(token, format!("已忽略来自 {address} 的无效连接：{error}"));
            }
            continue;
        }
        // A room has one peer. Closing the listener also releases queued connections.
        drop(listener);
        return connection(hub, stream, address.to_string(), token, Role::Host, cancel).await;
    }
}

async fn join_task(
    hub: &Arc<Hub>,
    address: String,
    port: u16,
    token: u64,
    cancel: &mut watch::Receiver<bool>,
) -> Result<(), String> {
    if !is_current(hub, token, cancel) {
        return Ok(());
    }
    let address = address.trim();
    if address.is_empty() || port == 0 {
        return Err("请输入有效的房主地址和端口".to_owned());
    }
    let Some(result) = cancellable(
        cancel,
        time::timeout(
            CONNECT_TIMEOUT,
            TcpStream::connect((address, port)),
        ),
    )
    .await
    else {
        return Ok(());
    };
    let mut stream = result
        .map_err(|_| "连接房主超时，请检查地址、端口和网络".to_owned())?
        .map_err(|error| format!("无法连接房主：{error}"))?;
    stream
        .set_nodelay(true)
        .map_err(|error| format!("无法初始化网络连接：{error}"))?;
    let peer_address = stream
        .peer_addr()
        .map(|address| address.to_string())
        .unwrap_or_else(|_| format!("{address}:{port}"));
    let Some(result) = cancellable(
        cancel,
        time::timeout(HANDSHAKE_TIMEOUT, handshake(&mut stream)),
    )
    .await
    else {
        return Ok(());
    };
    result.map_err(|_| "版本协商超时，请确认对方已创建房间".to_owned())??;
    connection(hub, stream, peer_address, token, Role::Guest, cancel).await
}

async fn handshake(stream: &mut TcpStream) -> Result<(), String> {
    write_frame(stream, &Wire::Hello { protocol: PROTOCOL }).await?;
    let message = match read_frame(stream).await? {
        Wire::Hello { protocol } if protocol == PROTOCOL => return Ok(()),
        Wire::Hello { protocol } => format!(
            "协议版本不兼容：本机版本 {PROTOCOL}，对方版本 {protocol}，请使用相同版本的客户端"
        ),
        Wire::Error { message } => return Err(format!("对方拒绝连接：{message}")),
        _ => "版本协商失败：首条消息必须是 Hello".to_owned(),
    };
    let _ = write_frame(
        stream,
        &Wire::Error {
            message: message.clone(),
        },
    )
    .await;
    Err(message)
}

async fn connection(
    hub: &Arc<Hub>,
    stream: TcpStream,
    address: String,
    token: u64,
    role: Role,
    cancel: &mut watch::Receiver<bool>,
) -> Result<(), String> {
    if !is_current(hub, token, cancel) {
        return Ok(());
    }
    let (tx, rx) = mpsc::channel(SEND_CAPACITY);
    if !hub.network_connected(token, tx.clone(), address) {
        return Ok(());
    }
    let (reader, writer) = stream.into_split();
    // Both futures stay alive until disconnection; heartbeat writes never restart a partial read.
    let mut reading = pin!(reader_loop(hub.clone(), reader, tx, token, role, cancel.clone()));
    let mut writing = pin!(writer_loop(writer, rx));
    let io = poll_fn(|cx| {
        if let Poll::Ready(result) = reading.as_mut().poll(cx) {
            return Poll::Ready(result);
        }
        writing.as_mut().poll(cx)
    });
    cancellable(cancel, io).await.unwrap_or(Ok(()))
}

async fn reader_loop(
    hub: Arc<Hub>,
    mut reader: OwnedReadHalf,
    tx: mpsc::Sender<Wire>,
    token: u64,
    role: Role,
    cancel: watch::Receiver<bool>,
) -> Result<(), String> {
    let mut pending: Option<StateAssembly> = None;
    loop {
        let message = time::timeout(FRAME_TIMEOUT, read_frame(&mut reader))
            .await
            .map_err(|_| "接收消息超时，与对方的连接已中断".to_owned())??;
        if !is_current(&hub, token, &cancel) {
            return Ok(());
        }
        let message = match pending.take() {
            Some(mut state) => {
                if !state.push(message)? {
                    pending = Some(state);
                    continue;
                }
                state.finish()?
            }
            None => message,
        };
        match message {
            Wire::Hello { .. } => return Err("协议错误：连接后不能再次协商版本".to_owned()),
            Wire::State { snapshot } => {
                if !matches!(role, Role::Guest) {
                    return Err("协议错误：房主不能接收对方的棋局状态".to_owned());
                }
                if !is_current(&hub, token, &cancel) {
                    return Ok(());
                }
                hub.receive_snapshot(token, *snapshot)
                    .map_err(|message| format!("棋局同步失败：{message}"))?;
            }
            Wire::Intent { intent } => {
                if !matches!(role, Role::Host) {
                    return Err("协议错误：加入方不能接收对方的操作请求".to_owned());
                }
                if !is_current(&hub, token, &cancel) {
                    return Ok(());
                }
                hub.receive_peer(token, intent);
            }
            Wire::Error { message } => {
                if is_current(&hub, token, &cancel) {
                    hub.network_notice(token, message);
                } else {
                    return Ok(());
                }
            }
            Wire::Ping => {
                tx.try_send(Wire::Pong)
                    .map_err(|_| "网络发送队列不可用，连接已中断".to_owned())?;
            }
            Wire::Pong => {}
            Wire::StateStart { id, total } => {
                if !matches!(role, Role::Guest) {
                    return Err("协议错误：房主不能接收对方的棋局状态分片".to_owned());
                }
                pending = Some(StateAssembly::new(id, total)?);
            }
            Wire::StateChunk { .. } => {
                return Err("协议错误：收到未声明的棋局状态分片".to_owned());
            }
        }
    }
}

async fn writer_loop(mut writer: OwnedWriteHalf, mut rx: mpsc::Receiver<Wire>) -> Result<(), String> {
    let mut heartbeat = time::interval_at(Instant::now() + PING_INTERVAL, PING_INTERVAL);
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut next_state_id = 1_u64;
    loop {
        let message = poll_fn(|cx| {
            if heartbeat.poll_tick(cx).is_ready() {
                return Poll::Ready(Some(Wire::Ping));
            }
            rx.poll_recv(cx)
        })
        .await
        .ok_or_else(|| "网络发送队列已关闭".to_owned())?;
        time::timeout(
            FRAME_TIMEOUT,
            write_message(&mut writer, message, &mut next_state_id),
        )
        .await
        .map_err(|_| "发送消息超时，与对方的连接已中断".to_owned())??;
    }
}

async fn write_message(
    writer: &mut OwnedWriteHalf,
    message: Wire,
    next_state_id: &mut u64,
) -> Result<(), String> {
    if !matches!(&message, Wire::State { .. }) {
        if matches!(&message, Wire::StateStart { .. } | Wire::StateChunk { .. }) {
            return Err("状态分片只能由网络传输层生成".to_owned());
        }
        return write_frame(writer, &message).await;
    }
    let bytes = encode(&message, MAX_STATE)?;
    if bytes.len() <= MAX_FRAME {
        return write_bytes(writer, &bytes).await;
    }
    let id = *next_state_id;
    *next_state_id = id
        .checked_add(1)
        .ok_or_else(|| "状态同步编号已耗尽，请重新连接".to_owned())?;
    write_frame(
        writer,
        &Wire::StateStart {
            id,
            total: bytes.len(),
        },
    )
    .await?;
    // One writer owns the stream, so queued messages cannot interleave these chunks.
    for (index, data) in bytes.chunks(CHUNK_SIZE).enumerate() {
        write_frame(
            writer,
            &Wire::StateChunk {
                id,
                index,
                data: data.to_vec(),
            },
        )
        .await?;
    }
    Ok(())
}

async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Wire, String> {
    let length = reader
        .read_u32()
        .await
        .map_err(|error| format!("连接已断开或无法读取消息长度：{error}"))? as usize;
    if length == 0 || length > MAX_FRAME {
        return Err("协议错误：消息长度必须在 1 到 65536 字节之间".to_owned());
    }
    let mut bytes = vec![0_u8; length];
    reader
        .read_exact(&mut bytes)
        .await
        .map_err(|error| format!("连接已断开或消息内容不完整：{error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("协议错误：消息不是有效的 JSON：{error}"))
}

async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, message: &Wire) -> Result<(), String> {
    write_bytes(writer, &encode(message, MAX_FRAME)?).await
}

async fn write_bytes<W: AsyncWrite + Unpin>(writer: &mut W, bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > MAX_FRAME {
        return Err("无法发送消息：消息超过单帧大小限制".to_owned());
    }
    writer
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .await
        .map_err(|error| format!("发送消息长度失败：{error}"))?;
    writer
        .write_all(bytes)
        .await
        .map_err(|error| format!("发送消息内容失败：{error}"))
}

// Bound serialization too, so an oversized local snapshot cannot allocate an unbounded buffer.
struct LimitedBytes {
    bytes: Vec<u8>,
    limit: usize,
}

impl Write for LimitedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit - self.bytes.len() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "消息超过大小限制"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn encode(message: &Wire, limit: usize) -> Result<Vec<u8>, String> {
    let mut output = LimitedBytes {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut output, message)
        .map_err(|error| format!("无法编码网络消息（上限 {limit} 字节）：{error}"))?;
    Ok(output.bytes)
}

struct StateAssembly {
    id: u64,
    total: usize,
    next_index: usize,
    bytes: Vec<u8>,
}

impl StateAssembly {
    fn new(id: u64, total: usize) -> Result<Self, String> {
        if total == 0 || total > MAX_STATE {
            return Err("协议错误：棋局状态大小必须在 1 字节到 8 MiB 之间".to_owned());
        }
        Ok(Self {
            id,
            total,
            next_index: 0,
            bytes: Vec::with_capacity(total.min(CHUNK_SIZE)),
        })
    }

    fn push(&mut self, message: Wire) -> Result<bool, String> {
        let Wire::StateChunk { id, index, data } = message else {
            return Err("协议错误：棋局状态分片被其他消息中断".to_owned());
        };
        if id != self.id || index != self.next_index {
            return Err("协议错误：棋局状态分片编号或顺序不正确".to_owned());
        }
        if data.is_empty() || data.len() > CHUNK_SIZE {
            return Err("协议错误：棋局状态分片大小不正确".to_owned());
        }
        if data.len() > self.total - self.bytes.len() {
            return Err("协议错误：棋局状态分片超过声明大小".to_owned());
        }
        self.bytes.extend_from_slice(&data);
        self.next_index += 1;
        Ok(self.bytes.len() == self.total)
    }

    fn finish(self) -> Result<Wire, String> {
        let message: Wire = serde_json::from_slice(&self.bytes)
            .map_err(|error| format!("协议错误：棋局状态分片不是有效的 JSON：{error}"))?;
        match message {
            Wire::State { .. } => Ok(message),
            _ => Err("协议错误：棋局状态分片只能包含完整的 State 消息".to_owned()),
        }
    }
}
