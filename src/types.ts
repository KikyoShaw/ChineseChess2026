export type Side = 'red' | 'black'
export type PieceKind = 'king' | 'rook' | 'horse' | 'cannon' | 'advisor' | 'elephant' | 'pawn'
export type GameMode = 'computer' | 'local' | 'lan'
export interface Piece { id: number; side: Side; kind: PieceKind }
export interface ChessMove { from: number; to: number }
export interface MoveRecord {
  ply: number; side: Side; piece: Piece; from: number; to: number
  captured: Piece | null; notation: string; time: string; checking: boolean
}
export interface Outcome { winner: Side | null; reason: string }
export interface Settings {
  mode: GameMode; humanSide: Side; firstSide: Side; difficulty: number
  stepSeconds: number; sound: boolean; showHints: boolean; flipped: boolean
}
export interface NetworkView {
  status: 'offline' | 'hosting' | 'connecting' | 'connected' | 'disconnected'
  role: 'host' | 'guest' | null; localSide: Side | null
  address: string; message: string
}
export interface PendingRequest {
  id: number; kind: 'undo' | 'draw' | 'restart'; from: Side
}
export interface ActionSignal { id: number; sound: string | null }
export interface Snapshot {
  revision: number; gameId: number; board: (Piece | null)[]; turn: Side
  history: MoveRecord[]; result: Outcome | null; inCheck: boolean
  legalMoves: ChessMove[]; settings: Settings; remainingMs: number | null
  thinking: boolean; network: NetworkView; pendingRequest: PendingRequest | null
  action: ActionSignal
}
export type ActionName = 'undo' | 'draw' | 'resign' | 'restart'
