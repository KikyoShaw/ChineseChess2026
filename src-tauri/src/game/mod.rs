use chrono::Local;
use serde::{Deserialize, Serialize};

pub mod ai;
pub mod fen;
mod notation;
mod rules;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Red,
    Black,
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Self::Red => Self::Black,
            Self::Black => Self::Red,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PieceKind {
    King,
    Rook,
    Horse,
    Cannon,
    Advisor,
    Elephant,
    Pawn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Piece {
    pub id: u8,
    pub side: Side,
    pub kind: PieceKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChessMove {
    pub from: usize,
    pub to: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub board: Vec<Option<Piece>>,
    pub turn: Side,
}

impl Position {
    pub fn initial(first: Side) -> Self {
        use PieceKind::*;

        let back_rank = [Rook, Horse, Elephant, Advisor, King, Advisor, Elephant, Horse, Rook];
        let mut board = vec![None; 90];
        for (column, kind) in back_rank.into_iter().enumerate() {
            board[column] = Some(Piece { id: 0, side: Side::Black, kind });
            board[81 + column] = Some(Piece { id: 0, side: Side::Red, kind });
        }
        for column in [1, 7] {
            board[18 + column] = Some(Piece { id: 0, side: Side::Black, kind: Cannon });
            board[63 + column] = Some(Piece { id: 0, side: Side::Red, kind: Cannon });
        }
        for column in [0, 2, 4, 6, 8] {
            board[27 + column] = Some(Piece { id: 0, side: Side::Black, kind: Pawn });
            board[54 + column] = Some(Piece { id: 0, side: Side::Red, kind: Pawn });
        }
        let mut id = 1;
        for piece in board.iter_mut().flatten() {
            piece.id = id;
            id += 1;
        }
        Self { board, turn: first }
    }

    pub fn legal_moves(&self) -> Vec<ChessMove> {
        rules::legal_moves(self)
    }

    pub fn in_check(&self, side: Side) -> bool {
        rules::in_check(self, side)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveRecord {
    pub ply: usize,
    pub side: Side,
    pub piece: Piece,
    pub from: usize,
    pub to: usize,
    pub captured: Option<Piece>,
    pub notation: String,
    pub time: String,
    pub checking: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub winner: Option<Side>,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepetitionKey {
    squares: Vec<u8>,
    turn: Side,
}

impl RepetitionKey {
    fn of(position: &Position) -> Self {
        let squares = position.board.iter().map(|square| {
            square.map_or(0, |piece| {
                let kind = match piece.kind {
                    PieceKind::King => 1,
                    PieceKind::Rook => 2,
                    PieceKind::Horse => 3,
                    PieceKind::Cannon => 4,
                    PieceKind::Advisor => 5,
                    PieceKind::Elephant => 6,
                    PieceKind::Pawn => 7,
                };
                kind + if piece.side == Side::Red { 7 } else { 0 }
            })
        }).collect();
        Self { squares, turn: position.turn }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub position: Position,
    pub history: Vec<MoveRecord>,
    pub result: Option<Outcome>,
    position_keys: Vec<RepetitionKey>,
}

impl Game {
    pub fn new(first: Side) -> Self {
        Self::from_position(Position::initial(first))
    }

    pub fn from_position(position: Position) -> Self {
        let key = RepetitionKey::of(&position);
        let result = adjudicate(&position);
        Self { position, history: Vec::new(), result, position_keys: vec![key] }
    }

    pub fn apply_move(&mut self, mv: ChessMove) -> Result<(), String> {
        if self.result.is_some() {
            return Err("对局已结束".into());
        }
        if self.position.board.len() != 90 {
            return Err("棋盘必须包含九十个位置".into());
        }
        if mv.from >= 90 || mv.to >= 90 {
            return Err("落子位置超出棋盘".into());
        }
        if mv.from == mv.to {
            return Err("起点和终点不能相同".into());
        }
        let piece = self.position.board[mv.from].ok_or("起点没有棋子")?;
        if piece.side != self.position.turn {
            return Err("尚未轮到该方行棋".into());
        }
        if let Some(target) = self.position.board[mv.to] {
            if target.side == piece.side {
                return Err("不能吃掉己方棋子".into());
            }
            if target.kind == PieceKind::King {
                return Err("将帅不能直接被吃，请通过将死结束对局".into());
            }
        }
        if !rules::is_legal_move(&self.position, mv) {
            return Err("不合法的走法，或走后己方将帅受攻击".into());
        }

        let notation = notation::format_move(&self.position, mv, piece);
        let captured = rules::make_move(&mut self.position, mv);
        let checking = self.position.in_check(self.position.turn);
        self.history.push(MoveRecord {
            ply: self.history.len() + 1,
            side: piece.side,
            piece,
            from: mv.from,
            to: mv.to,
            captured,
            notation,
            time: Local::now().format("%H:%M:%S").to_string(),
            checking,
        });
        let key = RepetitionKey::of(&self.position);
        self.position_keys.push(key.clone());
        self.result = adjudicate(&self.position);
        if self.result.is_none()
            && self.position_keys.iter().filter(|previous| **previous == key).count() >= 3
        {
            self.result = Some(Outcome { winner: None, reason: "三次重复".into() });
        }
        Ok(())
    }

    pub fn undo(&mut self, plies: usize) -> Result<(), String> {
        if plies > self.history.len() {
            return Err("可悔棋的步数不足".into());
        }
        if plies == 0 {
            return Ok(());
        }
        if self.position.board.len() != 90 {
            return Err("棋盘必须包含九十个位置".into());
        }
        if self.history[self.history.len() - plies..].iter()
            .any(|record| record.from >= 90 || record.to >= 90 || record.from == record.to)
        {
            return Err("行棋记录中的位置无效".into());
        }
        for _ in 0..plies {
            if let Some(record) = self.history.pop() {
                self.position.board[record.from] = Some(record.piece);
                self.position.board[record.to] = record.captured;
                self.position.turn = record.side;
                self.position_keys.pop();
            }
        }
        self.result = adjudicate(&self.position);
        let key = RepetitionKey::of(&self.position);
        if self.result.is_none()
            && self.position_keys.iter().filter(|previous| **previous == key).count() >= 3
        {
            self.result = Some(Outcome { winner: None, reason: "三次重复".into() });
        }
        Ok(())
    }

    pub fn resign(&mut self, side: Side, reason: &str) {
        if self.result.is_none() {
            self.result = Some(Outcome {
                winner: Some(side.other()),
                reason: if reason.trim().is_empty() { "认输".into() } else { reason.into() },
            });
        }
    }

    pub fn draw(&mut self, reason: &str) {
        if self.result.is_none() {
            self.result = Some(Outcome {
                winner: None,
                reason: if reason.trim().is_empty() { "双方同意和棋".into() } else { reason.into() },
            });
        }
    }
}

fn adjudicate(position: &Position) -> Option<Outcome> {
    if position.board.len() != 90 {
        return Some(Outcome { winner: None, reason: "无效局面".into() });
    }
    let mut red_kings = 0;
    let mut black_kings = 0;
    for piece in position.board.iter().flatten() {
        if piece.kind == PieceKind::King {
            match piece.side {
                Side::Red => red_kings += 1,
                Side::Black => black_kings += 1,
            }
        }
    }
    match (red_kings, black_kings) {
        (1, 0) => return Some(Outcome { winner: Some(Side::Red), reason: "黑方将帅缺失".into() }),
        (0, 1) => return Some(Outcome { winner: Some(Side::Black), reason: "红方将帅缺失".into() }),
        (1, 1) => {},
        _ => return Some(Outcome { winner: None, reason: "无效局面：将帅数量错误".into() }),
    }
    if position.legal_moves().is_empty() {
        Some(Outcome {
            winner: Some(position.turn.other()),
            reason: if position.in_check(position.turn) { "将死".into() } else { "困毙".into() },
        })
    } else {
        None
    }
}
