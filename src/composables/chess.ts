import type { Piece, PieceKind, Side } from '../types'

export const sideName = (side: Side) => side === 'red' ? '红方' : '黑方'

const names: Record<Side, Record<PieceKind, string>> = {
  red: { king: '帅', rook: '车', horse: '马', cannon: '炮', advisor: '仕', elephant: '相', pawn: '兵' },
  black: { king: '将', rook: '车', horse: '马', cannon: '炮', advisor: '士', elephant: '象', pawn: '卒' },
}
const assetKinds: Record<PieceKind, string> = {
  king: 'K', rook: 'R', horse: 'N', cannon: 'C', advisor: 'A', elephant: 'B', pawn: 'P',
}

export const pieceName = (piece: Piece) => names[piece.side][piece.kind]
export const pieceImage = (piece: Pick<Piece, 'side' | 'kind'>) =>
  `/assets/images/WOOD/${piece.side === 'red' ? 'R' : 'B'}${assetKinds[piece.kind]}.GIF`
export const squareName = (square: number) => `${String.fromCharCode(65 + square % 9)}${10 - Math.floor(square / 9)}`

export function clockText(milliseconds: number | null): string {
  if (milliseconds === null) return '不限时'
  const seconds = Math.max(0, Math.ceil(milliseconds / 1000))
  return `${Math.floor(seconds / 60).toString().padStart(2, '0')}:${(seconds % 60).toString().padStart(2, '0')}`
}

export const requestNames = { undo: '悔棋', draw: '和棋', restart: '重新开局' } as const

export function reasonText(reason: string): string {
  const reasons: Record<string, string> = {
    checkmate: '将死', stalemate: '困毙', resign: '认输', resignation: '认输',
    timeout: '行棋超时', draw: '双方和棋', agreement: '双方同意和棋',
    repetition: '重复局面', king_captured: '将帅被吃', disconnect: '连接已断开',
  }
  return reasons[reason] ?? reason
}
