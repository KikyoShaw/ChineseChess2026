use super::{ChessMove, Piece, PieceKind, Position, Side};

const ORTHOGONAL: [(i32, i32); 4] = [(-1, 0), (0, -1), (0, 1), (1, 0)];
const DIAGONAL: [(i32, i32); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];
const HORSE: [(i32, i32); 8] = [
    (-2, -1), (-2, 1), (-1, -2), (-1, 2),
    (1, -2), (1, 2), (2, -1), (2, 1),
];

pub(super) fn offset(square: usize, row_delta: i32, column_delta: i32) -> Option<usize> {
    if square >= 90 {
        return None;
    }
    let row = (square / 9) as i32 + row_delta;
    let column = (square % 9) as i32 + column_delta;
    if (0..10).contains(&row) && (0..9).contains(&column) {
        Some((row * 9 + column) as usize)
    } else {
        None
    }
}

pub(super) fn in_palace(side: Side, square: usize) -> bool {
    if square >= 90 || !(3..=5).contains(&(square % 9)) {
        return false;
    }
    match side {
        Side::Red => square / 9 >= 7,
        Side::Black => square / 9 <= 2,
    }
}

pub(super) fn advisor_square(side: Side, square: usize) -> bool {
    if !in_palace(side, square) {
        return false;
    }
    let row = match side {
        Side::Black => square / 9,
        Side::Red => 9 - square / 9,
    };
    matches!((row, square % 9), (0, 3) | (0, 5) | (1, 4) | (2, 3) | (2, 5))
}

pub(super) fn elephant_square(side: Side, square: usize) -> bool {
    if square >= 90 {
        return false;
    }
    let row = match side {
        Side::Black => square / 9,
        Side::Red => 9 - square / 9,
    };
    matches!((row, square % 9),
        (0, 2) | (0, 6) | (2, 0) | (2, 4) | (2, 8) | (4, 2) | (4, 6))
}

fn forward(side: Side) -> i32 {
    match side {
        Side::Red => -1,
        Side::Black => 1,
    }
}

fn crossed_river(side: Side, square: usize) -> bool {
    match side {
        Side::Red => square / 9 <= 4,
        Side::Black => square / 9 >= 5,
    }
}

pub(super) fn king_square(position: &Position, side: Side) -> Option<usize> {
    if position.board.len() != 90 {
        return None;
    }
    position.board.iter().position(|square| {
        matches!(square, Some(piece) if piece.side == side && piece.kind == PieceKind::King)
    })
}

pub(super) fn kings_facing(position: &Position) -> bool {
    let (Some(red), Some(black)) = (
        king_square(position, Side::Red), king_square(position, Side::Black),
    ) else {
        return false;
    };
    if red % 9 != black % 9 {
        return false;
    }
    let top = red.min(black);
    let bottom = red.max(black);
    ((top + 9)..bottom).step_by(9).all(|square| position.board[square].is_none())
}

pub(super) fn in_check(position: &Position, side: Side) -> bool {
    let Some(king) = king_square(position, side) else {
        return true;
    };
    let enemy = side.other();

    // From the king, the first occupied square can be a rook and the second a cannon.
    for (dr, dc) in ORTHOGONAL {
        let mut square = king;
        let mut screened = false;
        let mut distance = 0;
        while let Some(next) = offset(square, dr, dc) {
            square = next;
            distance += 1;
            let Some(piece) = position.board[square] else {
                continue;
            };
            if screened {
                if piece.side == enemy && piece.kind == PieceKind::Cannon {
                    return true;
                }
                break;
            }
            if piece.side == enemy {
                match piece.kind {
                    PieceKind::Rook => return true,
                    PieceKind::King if dc == 0 || distance == 1 => return true,
                    PieceKind::Pawn if distance == 1 => {
                        if (dc == 0 && -dr == forward(enemy))
                            || (dr == 0 && crossed_river(enemy, square))
                        {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            screened = true;
        }
    }

    for (dr, dc) in HORSE {
        let Some(source) = offset(king, dr, dc) else {
            continue;
        };
        if !matches!(position.board[source], Some(piece)
            if piece.side == enemy && piece.kind == PieceKind::Horse)
        {
            continue;
        }
        let leg = if dr.abs() == 2 {
            offset(source, -dr.signum(), 0)
        } else {
            offset(source, 0, -dc.signum())
        };
        if leg.is_some_and(|square| position.board[square].is_none()) {
            return true;
        }
    }

    for (dr, dc) in DIAGONAL {
        if advisor_square(enemy, king) {
            if let Some(source) = offset(king, dr, dc) {
                if matches!(position.board[source], Some(piece)
                    if piece.side == enemy && piece.kind == PieceKind::Advisor)
                    && advisor_square(enemy, source)
                {
                    return true;
                }
            }
        }
        if elephant_square(enemy, king) {
            if let (Some(source), Some(eye)) = (offset(king, dr * 2, dc * 2), offset(king, dr, dc)) {
                if matches!(position.board[source], Some(piece)
                    if piece.side == enemy && piece.kind == PieceKind::Elephant)
                    && elephant_square(enemy, source)
                    && position.board[eye].is_none()
                {
                    return true;
                }
            }
        }
    }
    false
}

fn push_target(position: &Position, from: usize, to: usize, side: Side, moves: &mut Vec<ChessMove>) {
    match position.board[to] {
        Some(piece) if piece.side == side || piece.kind == PieceKind::King => {}
        _ => moves.push(ChessMove { from, to }),
    }
}

fn pseudo_moves_from(position: &Position, from: usize, piece: Piece, moves: &mut Vec<ChessMove>) {
    match piece.kind {
        PieceKind::King => {
            if !in_palace(piece.side, from) {
                return;
            }
            for (dr, dc) in ORTHOGONAL {
                if let Some(to) = offset(from, dr, dc) {
                    if in_palace(piece.side, to) {
                        push_target(position, from, to, piece.side, moves);
                    }
                }
            }
        }
        PieceKind::Advisor => {
            if !advisor_square(piece.side, from) {
                return;
            }
            for (dr, dc) in DIAGONAL {
                if let Some(to) = offset(from, dr, dc) {
                    if advisor_square(piece.side, to) {
                        push_target(position, from, to, piece.side, moves);
                    }
                }
            }
        }
        PieceKind::Elephant => {
            if !elephant_square(piece.side, from) {
                return;
            }
            for (dr, dc) in DIAGONAL {
                if let (Some(to), Some(eye)) = (offset(from, dr * 2, dc * 2), offset(from, dr, dc)) {
                    if elephant_square(piece.side, to) && position.board[eye].is_none() {
                        push_target(position, from, to, piece.side, moves);
                    }
                }
            }
        }
        PieceKind::Horse => {
            for (dr, dc) in HORSE {
                let leg = if dr.abs() == 2 {
                    offset(from, dr.signum(), 0)
                } else {
                    offset(from, 0, dc.signum())
                };
                if let (Some(to), Some(leg)) = (offset(from, dr, dc), leg) {
                    if position.board[leg].is_none() {
                        push_target(position, from, to, piece.side, moves);
                    }
                }
            }
        }
        PieceKind::Rook => {
            for (dr, dc) in ORTHOGONAL {
                let mut square = from;
                while let Some(to) = offset(square, dr, dc) {
                    push_target(position, from, to, piece.side, moves);
                    if position.board[to].is_some() {
                        break;
                    }
                    square = to;
                }
            }
        }
        PieceKind::Cannon => {
            for (dr, dc) in ORTHOGONAL {
                let mut square = from;
                let mut screened = false;
                while let Some(to) = offset(square, dr, dc) {
                    square = to;
                    if position.board[to].is_some() {
                        if screened {
                            push_target(position, from, to, piece.side, moves);
                            break;
                        }
                        screened = true;
                    } else if !screened {
                        moves.push(ChessMove { from, to });
                    }
                }
            }
        }
        PieceKind::Pawn => {
            if let Some(to) = offset(from, forward(piece.side), 0) {
                push_target(position, from, to, piece.side, moves);
            }
            if crossed_river(piece.side, from) {
                for dc in [-1, 1] {
                    if let Some(to) = offset(from, 0, dc) {
                        push_target(position, from, to, piece.side, moves);
                    }
                }
            }
        }
    }
}

pub(super) fn make_move(position: &mut Position, mv: ChessMove) -> Option<Piece> {
    let piece = position.board[mv.from].take();
    let captured = std::mem::replace(&mut position.board[mv.to], piece);
    position.turn = position.turn.other();
    captured
}

pub(super) fn unmake_move(position: &mut Position, mv: ChessMove, captured: Option<Piece>) {
    position.board[mv.from] = position.board[mv.to].take();
    position.board[mv.to] = captured;
    position.turn = position.turn.other();
}

pub(super) fn is_legal_move(position: &Position, mv: ChessMove) -> bool {
    if position.board.len() != 90 || mv.from >= 90 || mv.to >= 90 || mv.from == mv.to {
        return false;
    }
    let Some(piece) = position.board[mv.from] else {
        return false;
    };
    if piece.side != position.turn || king_square(position, piece.side.other()).is_none() {
        return false;
    }
    let mut candidates = Vec::with_capacity(20);
    pseudo_moves_from(position, mv.from, piece, &mut candidates);
    if !candidates.contains(&mv) {
        return false;
    }
    let mut next = position.clone();
    make_move(&mut next, mv);
    !in_check(&next, piece.side)
}

pub(super) fn legal_moves(position: &Position) -> Vec<ChessMove> {
    legal_moves_with_stop(position, &mut || false).unwrap_or_default()
}

pub(super) fn legal_moves_with_stop<F>(position: &Position, should_stop: &mut F) -> Option<Vec<ChessMove>>
where
    F: FnMut() -> bool,
{
    if should_stop() {
        return None;
    }
    if position.board.len() != 90
        || king_square(position, position.turn).is_none()
        || king_square(position, position.turn.other()).is_none()
    {
        return Some(Vec::new());
    }
    let mut candidates = Vec::with_capacity(64);
    for (from, square) in position.board.iter().enumerate() {
        if should_stop() {
            return None;
        }
        if let Some(piece) = square {
            if piece.side == position.turn {
                pseudo_moves_from(position, from, *piece, &mut candidates);
            }
        }
    }
    let mut scratch = position.clone();
    let mut legal = Vec::with_capacity(candidates.len());
    for mv in candidates {
        if should_stop() {
            return None;
        }
        let captured = make_move(&mut scratch, mv);
        let safe = !in_check(&scratch, position.turn);
        unmake_move(&mut scratch, mv, captured);
        if safe {
            legal.push(mv);
        }
    }
    Some(legal)
}
