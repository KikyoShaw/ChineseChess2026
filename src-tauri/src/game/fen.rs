use super::{rules, Piece, PieceKind, Position, Side};

fn kind_index(kind: PieceKind) -> usize {
    match kind {
        PieceKind::King => 0,
        PieceKind::Rook => 1,
        PieceKind::Horse => 2,
        PieceKind::Cannon => 3,
        PieceKind::Advisor => 4,
        PieceKind::Elephant => 5,
        PieceKind::Pawn => 6,
    }
}

pub fn parse(fen: &str) -> Result<Position, String> {
    let fields: Vec<&str> = fen.split_whitespace().collect();
    if fields.len() != 2 && fields.len() != 6 {
        return Err("FEN 应包含棋盘和行棋方，或完整的六个标准字段".into());
    }
    let turn = match fields[1] {
        "w" => Side::Red,
        "b" => Side::Black,
        _ => return Err("FEN 行棋方只能是 w（红方）或 b（黑方）".into()),
    };
    if fields.len() == 6 {
        if fields[2] != "-" || fields[3] != "-"
            || fields[4].parse::<u64>().is_err()
            || !matches!(fields[5].parse::<u64>(), Ok(value) if value > 0)
        {
            return Err("FEN 附加字段无效，象棋不使用王车易位或吃过路兵字段".into());
        }
    }
    let rows: Vec<&str> = fields[0].split('/').collect();
    if rows.len() != 10 {
        return Err("FEN 棋盘必须恰好有十行".into());
    }
    let mut board = vec![None; 90];
    let mut counts = [[0_usize; 7]; 2];
    let limits = [1, 2, 2, 2, 2, 2, 5];
    let mut next_id = 1_u8;
    for (row, text) in rows.iter().enumerate() {
        let mut column = 0;
        for character in text.chars() {
            if ('1'..='9').contains(&character) {
                column += (character as u8 - b'0') as usize;
                if column > 9 {
                    return Err(format!("FEN 第{}行超过九列", row + 1));
                }
                continue;
            }
            let kind = match character.to_ascii_lowercase() {
                'k' => PieceKind::King,
                'r' => PieceKind::Rook,
                'n' | 'h' => PieceKind::Horse,
                'c' => PieceKind::Cannon,
                'a' => PieceKind::Advisor,
                'b' | 'e' => PieceKind::Elephant,
                'p' => PieceKind::Pawn,
                _ => return Err(format!("FEN 包含无效棋子字符：{character}")),
            };
            if column >= 9 {
                return Err(format!("FEN 第{}行超过九列", row + 1));
            }
            let side = if character.is_ascii_uppercase() { Side::Red } else { Side::Black };
            let side_index = if side == Side::Red { 0 } else { 1 };
            let index = kind_index(kind);
            counts[side_index][index] += 1;
            if counts[side_index][index] > limits[index] {
                return Err("FEN 中同一方的棋子数量超过上限".into());
            }
            let square = row * 9 + column;
            match kind {
                PieceKind::King if !rules::in_palace(side, square) => {
                    return Err("将帅必须位于己方九宫内".into());
                }
                PieceKind::Advisor if !rules::advisor_square(side, square) => {
                    return Err("仕士必须位于己方九宫内可到达的位置".into());
                }
                PieceKind::Elephant if !rules::elephant_square(side, square) => {
                    return Err("相象必须位于己方河界内可到达的位置".into());
                }
                _ => {}
            }
            board[square] = Some(Piece { id: next_id, side, kind });
            next_id += 1;
            column += 1;
        }
        if column != 9 {
            return Err(format!("FEN 第{}行必须恰好有九列", row + 1));
        }
    }
    if counts[0][0] != 1 || counts[1][0] != 1 {
        return Err("FEN 必须同时包含一个红帅和一个黑将".into());
    }
    let position = Position { board, turn };
    if rules::kings_facing(&position) {
        return Err("将帅不能直接照面".into());
    }
    if position.in_check(turn.other()) {
        return Err("非行棋方不能处于被将军状态".into());
    }
    Ok(position)
}

pub fn format(position: &Position) -> String {
    let mut fen = String::with_capacity(100);
    for row in 0..10 {
        if row > 0 {
            fen.push('/');
        }
        let mut empty = 0_u8;
        for column in 0..9 {
            let piece = position.board.get(row * 9 + column).copied().flatten();
            let Some(piece) = piece else {
                empty += 1;
                continue;
            };
            if empty > 0 {
                fen.push((b'0' + empty) as char);
                empty = 0;
            }
            let code = match piece.kind {
                PieceKind::King => 'k',
                PieceKind::Rook => 'r',
                PieceKind::Horse => 'n',
                PieceKind::Cannon => 'c',
                PieceKind::Advisor => 'a',
                PieceKind::Elephant => 'b',
                PieceKind::Pawn => 'p',
            };
            fen.push(if piece.side == Side::Red { code.to_ascii_uppercase() } else { code });
        }
        if empty > 0 {
            fen.push((b'0' + empty) as char);
        }
    }
    fen.push(' ');
    fen.push(if position.turn == Side::Red { 'w' } else { 'b' });
    fen
}
