use super::{ChessMove, Piece, PieceKind, Position, Side};

const CHINESE_DIGITS: [&str; 10] = ["零", "一", "二", "三", "四", "五", "六", "七", "八", "九"];

fn number(side: Side, value: usize) -> String {
    match side {
        Side::Red if value < CHINESE_DIGITS.len() => CHINESE_DIGITS[value].into(),
        _ => value.to_string(),
    }
}

fn file(side: Side, square: usize) -> usize {
    match side {
        Side::Red => 9 - square % 9,
        Side::Black => square % 9 + 1,
    }
}

fn name(piece: Piece) -> &'static str {
    match (piece.side, piece.kind) {
        (Side::Red, PieceKind::King) => "帅",
        (Side::Black, PieceKind::King) => "将",
        (_, PieceKind::Rook) => "车",
        (_, PieceKind::Horse) => "马",
        (_, PieceKind::Cannon) => "炮",
        (Side::Red, PieceKind::Advisor) => "仕",
        (Side::Black, PieceKind::Advisor) => "士",
        (Side::Red, PieceKind::Elephant) => "相",
        (Side::Black, PieceKind::Elephant) => "象",
        (Side::Red, PieceKind::Pawn) => "兵",
        (Side::Black, PieceKind::Pawn) => "卒",
    }
}

fn front_order(side: Side, square: usize) -> usize {
    match side {
        Side::Red => square / 9,
        Side::Black => 9 - square / 9,
    }
}

fn prefix(position: &Position, from: usize, piece: Piece) -> String {
    let mut peers: Vec<usize> = position.board.iter().enumerate().filter_map(|(square, occupant)| {
        occupant.filter(|other| other.side == piece.side && other.kind == piece.kind)
            .map(|_| square)
    }).collect();
    let mut same_file: Vec<usize> = peers.iter().copied()
        .filter(|square| square % 9 == from % 9).collect();
    if same_file.len() <= 1 {
        return format!("{}{}", name(piece), number(piece.side, file(piece.side, from)));
    }
    same_file.sort_by_key(|square| front_order(piece.side, *square));

    if piece.kind == PieceKind::Pawn {
        let mut file_counts = [0; 9];
        for square in &peers {
            file_counts[square % 9] += 1;
        }
        if file_counts.iter().filter(|count| **count > 1).count() > 1 {
            // Number stacked pawns from the player's right, front to back within each file.
            peers.retain(|square| file_counts[square % 9] > 1);
            peers.sort_by_key(|square| (file(piece.side, *square), front_order(piece.side, *square)));
            let ordinal = peers.iter().position(|square| *square == from).unwrap_or(0) + 1;
            return format!("{}{}", CHINESE_DIGITS[ordinal.min(9)], name(piece));
        }
    }

    let ordinal = same_file.iter().position(|square| *square == from).unwrap_or(0);
    let marker = match (same_file.len(), ordinal) {
        (_, 0) if same_file.len() <= 3 => "前",
        (2, _) => "后",
        (3, 1) => "中",
        (3, _) => "后",
        _ => CHINESE_DIGITS[(ordinal + 1).min(9)],
    };
    format!("{}{}", marker, name(piece))
}

pub(super) fn format_move(position: &Position, mv: ChessMove, piece: Piece) -> String {
    let mut notation = prefix(position, mv.from, piece);
    let from_row = mv.from / 9;
    let to_row = mv.to / 9;
    if from_row == to_row {
        notation.push('平');
        notation.push_str(&number(piece.side, file(piece.side, mv.to)));
    } else {
        let advances = match piece.side {
            Side::Red => to_row < from_row,
            Side::Black => to_row > from_row,
        };
        notation.push(if advances { '进' } else { '退' });
        let value = match piece.kind {
            PieceKind::Horse | PieceKind::Advisor | PieceKind::Elephant => file(piece.side, mv.to),
            _ => from_row.abs_diff(to_row),
        };
        notation.push_str(&number(piece.side, value));
    }
    notation
}
