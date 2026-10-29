use std::cmp::Reverse;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{rules, ChessMove, Piece, PieceKind, Position, Side};

const MATE: i32 = 1_000_000;
const INFINITY: i32 = 2_000_000;

// Compact piece-square tables use black-side orientation.
const ROOK: [[i32; 9]; 10] = [
    [194, 206, 204, 212, 200, 212, 204, 206, 194],
    [200, 208, 206, 212, 200, 212, 206, 208, 200],
    [198, 208, 204, 212, 212, 212, 204, 208, 198],
    [204, 209, 204, 212, 214, 212, 204, 209, 204],
    [208, 212, 212, 214, 215, 214, 212, 212, 208],
    [208, 211, 211, 214, 215, 214, 211, 211, 208],
    [206, 213, 213, 216, 216, 216, 213, 213, 206],
    [206, 208, 207, 214, 216, 214, 207, 208, 206],
    [206, 212, 209, 216, 233, 216, 209, 212, 206],
    [206, 208, 207, 213, 214, 213, 207, 208, 206],
];
const HORSE: [[i32; 9]; 10] = [
    [88, 85, 90, 88, 90, 88, 90, 85, 88],
    [85, 90, 92, 93, 78, 93, 92, 90, 85],
    [93, 92, 94, 95, 92, 95, 94, 92, 93],
    [92, 94, 98, 95, 98, 95, 98, 94, 92],
    [90, 98, 101, 102, 103, 102, 101, 98, 90],
    [90, 100, 99, 103, 104, 103, 99, 100, 90],
    [93, 108, 100, 107, 100, 107, 100, 108, 93],
    [92, 98, 99, 103, 99, 103, 99, 98, 92],
    [90, 96, 103, 97, 94, 97, 103, 96, 90],
    [90, 90, 90, 96, 90, 96, 90, 90, 90],
];
const CANNON: [[i32; 9]; 10] = [
    [96, 96, 97, 99, 99, 99, 97, 96, 96],
    [96, 97, 98, 98, 98, 98, 98, 97, 96],
    [97, 96, 100, 99, 101, 99, 100, 96, 97],
    [96, 96, 96, 96, 96, 96, 96, 96, 96],
    [95, 96, 99, 96, 100, 96, 99, 96, 95],
    [96, 96, 96, 96, 100, 96, 96, 96, 96],
    [96, 99, 99, 98, 100, 98, 99, 99, 96],
    [97, 97, 96, 91, 92, 91, 96, 97, 97],
    [98, 98, 96, 92, 89, 92, 96, 98, 98],
    [100, 100, 96, 91, 90, 91, 96, 100, 100],
];
const PAWN: [[i32; 9]; 10] = [
    [0; 9],
    [0; 9],
    [0; 9],
    [7, 0, 7, 0, 15, 0, 7, 0, 7],
    [7, 0, 13, 0, 16, 0, 13, 0, 7],
    [14, 18, 20, 27, 29, 27, 20, 18, 14],
    [19, 23, 27, 29, 30, 29, 27, 23, 19],
    [19, 24, 32, 37, 37, 37, 32, 24, 19],
    [19, 24, 34, 42, 44, 42, 34, 24, 19],
    [9, 9, 9, 11, 13, 11, 9, 9, 9],
];

fn material(kind: PieceKind) -> i32 {
    match kind {
        PieceKind::King => 10_000,
        PieceKind::Rook => 200,
        PieceKind::Horse => 90,
        PieceKind::Cannon => 96,
        PieceKind::Advisor | PieceKind::Elephant => 20,
        PieceKind::Pawn => 7,
    }
}

fn piece_value(piece: Piece, square: usize) -> i32 {
    let (row, column) = match piece.side {
        Side::Black => (square / 9, square % 9),
        Side::Red => (9 - square / 9, 8 - square % 9),
    };
    let base = material(piece.kind);
    let positional = match piece.kind {
        PieceKind::Rook => ROOK[row][column] - base,
        PieceKind::Horse => HORSE[row][column] - base,
        PieceKind::Cannon => CANNON[row][column] - base,
        PieceKind::Pawn => PAWN[row][column] - base,
        PieceKind::Advisor => if row == 1 && column == 4 { 3 } else { 0 },
        PieceKind::Elephant => match (row, column) {
            (2, 4) => 3,
            (2, 0) | (2, 8) => -2,
            _ => 0,
        },
        PieceKind::King => match (row, column) {
            (0, 3..=5) => 1,
            (1, 3..=5) => 2,
            (2, 4) => 15,
            (2, 3) | (2, 5) => 11,
            _ => 0,
        },
    };
    base + positional
}

fn evaluate(position: &Position) -> i32 {
    let mut score = 0;
    for (square, piece) in position.board.iter().enumerate() {
        if let Some(piece) = piece {
            let value = piece_value(*piece, square);
            score += if piece.side == position.turn { value } else { -value };
        }
    }
    score
}

fn order_moves(position: &Position, moves: &mut [ChessMove], preferred: Option<ChessMove>) {
    moves.sort_unstable_by_key(|mv| {
        let capture_priority = match (position.board[mv.to], position.board[mv.from]) {
            (Some(victim), Some(attacker)) => 100_000 + material(victim.kind) * 16 - material(attacker.kind),
            _ => 0,
        };
        (Reverse(capture_priority), Some(*mv) != preferred, mv.from, mv.to)
    });
}

struct Search {
    deadline: Instant,
    cancel: Arc<AtomicBool>,
}

impl Search {
    fn stopped(&self) -> bool {
        self.cancel.load(Ordering::Relaxed) || Instant::now() >= self.deadline
    }

    fn moves(&self, position: &Position) -> Result<Vec<ChessMove>, ()> {
        rules::legal_moves_with_stop(position, &mut || self.stopped()).ok_or(())
    }

    fn root(&self, position: &mut Position, moves: &[ChessMove], depth: u8) -> Result<(ChessMove, i32), ()> {
        let mut best_move = *moves.first().ok_or(())?;
        let mut best_score = -INFINITY;
        let mut alpha = -INFINITY;
        for &mv in moves {
            if self.stopped() {
                return Err(());
            }
            let captured = rules::make_move(position, mv);
            let score = self.negamax(position, depth - 1, 1, -INFINITY, -alpha);
            rules::unmake_move(position, mv, captured);
            let score = -score?;
            if score > best_score {
                best_move = mv;
                best_score = score;
            }
            alpha = alpha.max(score);
        }
        Ok((best_move, best_score))
    }

    fn negamax(&self, position: &mut Position, depth: u8, ply: i32, mut alpha: i32, beta: i32) -> Result<i32, ()> {
        if self.stopped() {
            return Err(());
        }
        if depth == 0 {
            return self.quiescence(position, ply, alpha, beta, 4);
        }
        let mut moves = self.moves(position)?;
        if moves.is_empty() {
            // Stalemate is a loss in Xiangqi, just as checkmate is.
            return Ok(-MATE + ply);
        }
        order_moves(position, &mut moves, None);
        let mut best = -INFINITY;
        for mv in moves {
            if self.stopped() {
                return Err(());
            }
            let captured = rules::make_move(position, mv);
            let score = self.negamax(position, depth - 1, ply + 1, -beta, -alpha);
            rules::unmake_move(position, mv, captured);
            let score = -score?;
            best = best.max(score);
            alpha = alpha.max(score);
            if alpha >= beta {
                break;
            }
        }
        Ok(best)
    }

    fn quiescence(&self, position: &mut Position, ply: i32, mut alpha: i32, beta: i32, remaining: u8) -> Result<i32, ()> {
        if self.stopped() {
            return Err(());
        }
        let mut moves = self.moves(position)?;
        if moves.is_empty() {
            return Ok(-MATE + ply);
        }
        let standing = evaluate(position);
        if remaining == 0 {
            return Ok(standing);
        }
        let checked = position.in_check(position.turn);
        let mut best = if checked { -INFINITY } else { standing };
        if !checked {
            if standing >= beta {
                return Ok(standing);
            }
            alpha = alpha.max(standing);
            moves.retain(|mv| position.board[mv.to].is_some());
        }
        order_moves(position, &mut moves, None);
        for mv in moves {
            if self.stopped() {
                return Err(());
            }
            let captured = rules::make_move(position, mv);
            let score = self.quiescence(position, ply + 1, -beta, -alpha, remaining - 1);
            rules::unmake_move(position, mv, captured);
            let score = -score?;
            best = best.max(score);
            alpha = alpha.max(score);
            if alpha >= beta {
                break;
            }
        }
        Ok(best)
    }
}

pub fn search(position: Position, difficulty: u8, cancel: Arc<AtomicBool>) -> Option<ChessMove> {
    search_with_budget(position, difficulty, cancel, Duration::MAX)
}

pub fn search_with_budget(
    mut position: Position,
    difficulty: u8,
    cancel: Arc<AtomicBool>,
    budget: Duration,
) -> Option<ChessMove> {
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    let (max_depth, milliseconds) = match difficulty {
        0 | 1 => (2, 200),
        2 => (3, 650),
        _ => (4, 1_500),
    };
    let search = Search {
        deadline: Instant::now() + Duration::from_millis(milliseconds).min(budget),
        cancel: Arc::clone(&cancel),
    };
    // Root generation is bounded by the board size and supplies a legal timeout fallback.
    let mut moves = rules::legal_moves_with_stop(&position, &mut || cancel.load(Ordering::Relaxed))?;
    order_moves(&position, &mut moves, None);
    let mut best = *moves.first()?;
    for depth in 1..=max_depth {
        if search.stopped() {
            break;
        }
        order_moves(&position, &mut moves, Some(best));
        match search.root(&mut position, &moves, depth) {
            Ok((mv, score)) => {
                // An interrupted iteration must never replace the last complete result.
                best = mv;
                if score.abs() >= MATE - 100 {
                    break;
                }
            }
            Err(()) => break,
        }
    }
    if cancel.load(Ordering::Relaxed) { None } else { Some(best) }
}
