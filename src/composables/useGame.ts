import { computed, onMounted, onUnmounted, ref, shallowRef } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { ActionName, Settings, Snapshot } from '../types'

export interface Toast { id: number; text: string; kind: 'success' | 'error' | 'info' }
type Preference = Pick<Settings, 'sound' | 'showHints' | 'flipped'>

const soundFiles: Record<string, string> = {
  choose: 'Choose', forbid: 'Forbid', newgame: 'NewGame', move: 'Move', eat: 'Eat',
  attackking: 'AttackKing', win: 'Win', loss: 'Loss', draw: 'Draw',
}

function errorMessage(error: unknown): string {
  if (typeof error === 'string') return error
  if (error instanceof Error) return error.message
  if (error && typeof error === 'object' && 'message' in error) return String(error.message)
  return '操作未完成，请稍后重试。'
}

export function useGame() {
  const snapshot = shallowRef<Snapshot | null>(null)
  const selected = ref<number | null>(null)
  const busy = ref<string | null>(null)
  const moving = ref(false)
  const awaitingPeerAction = ref<ActionName | 'response' | null>(null)
  let moveWait: { revision: number; gameId: number } | null = null
  let peerWait: { revision: number; gameId: number } | null = null
  const connection = ref<'loading' | 'ready' | 'error'>('loading')
  const startupError = ref('')
  const commandError = ref('')
  const toasts = ref<Toast[]>([])
  const clockNow = ref(performance.now())
  const clockAnchor = ref(clockNow.value)
  let unlisten: UnlistenFn | null = null
  let disposed = false
  let initializing = false
  let initializedSound = false
  let lastSoundKey = ''
  let toastId = 0
  const toastTimers = new Map<number, ReturnType<typeof setTimeout>>()
  const audios = new Map<string, HTMLAudioElement>()
  let clockTimer: ReturnType<typeof setInterval> | undefined

  const inLan = computed(() => {
    const state = snapshot.value
    return !!state && (state.settings.mode === 'lan' || state.network.role !== null || state.network.status !== 'offline')
  })
  const networkReady = computed(() => !inLan.value || snapshot.value?.network.status === 'connected')
  const clockRunning = computed(() => {
    const state = snapshot.value
    return !!state && !state.result && state.remainingMs !== null && networkReady.value
  })
  const remainingMs = computed(() => {
    const remaining = snapshot.value?.remainingMs ?? null
    if (remaining === null) return null
    return Math.max(0, remaining - (clockRunning.value ? clockNow.value - clockAnchor.value : 0))
  })
  const canPlay = computed(() => {
    const state = snapshot.value
    if (!state || state.result || state.thinking || awaitingPeerAction.value || busy.value || moving.value || !networkReady.value) return false
    if (state.settings.mode === 'computer') return state.turn === state.settings.humanSide
    if (inLan.value) return state.turn === state.network.localSide
    return true
  })
  const legalTargets = computed(() => new Set(
    snapshot.value?.legalMoves.filter(move => move.from === selected.value).map(move => move.to) ?? [],
  ))

  function dismissToast(id: number) {
    toasts.value = toasts.value.filter(toast => toast.id !== id)
    const timer = toastTimers.get(id)
    if (timer) clearTimeout(timer)
    toastTimers.delete(id)
  }

  function notify(text: string, kind: Toast['kind'] = 'info') {
    if (disposed) return
    const id = ++toastId
    toasts.value = [...toasts.value.slice(-2), { id, text, kind }]
    toastTimers.set(id, setTimeout(() => dismissToast(id), kind === 'error' ? 6500 : 3800))
  }

  function playSound(rawName: string) {
    if (!snapshot.value?.settings.sound || disposed) return
    const name = rawName.split(/[\\/]/).pop()?.replace(/\.wav$/i, '').toLowerCase() ?? ''
    const file = soundFiles[name]
    if (!file) return
    let audio = audios.get(file)
    if (!audio) {
      audio = new Audio(`/assets/audios/${file}.wav`)
      audio.preload = 'auto'
      audio.volume = 0.65
      audios.set(file, audio)
    }
    audio.currentTime = 0
    void audio.play().catch(() => { /* A disabled audio device must not interrupt play. */ })
  }

  function acceptSnapshot(next: Snapshot) {
    if (disposed || (snapshot.value && next.revision < snapshot.value.revision)) return
    const previous = snapshot.value
    if (moveWait && (next.revision > moveWait.revision || next.gameId !== moveWait.gameId)) {
      moveWait = null
      moving.value = false
    }
    if (peerWait && (next.revision > peerWait.revision || next.gameId !== peerWait.gameId)) {
      peerWait = null
      awaitingPeerAction.value = null
    }
    if (!previous || next.revision !== previous.revision || next.gameId !== previous.gameId) selected.value = null
    snapshot.value = next
    clockAnchor.value = performance.now()
    clockNow.value = clockAnchor.value
    connection.value = 'ready'
    startupError.value = ''
    const soundKey = String(next.action.id)
    if (initializedSound && soundKey !== lastSoundKey && next.action.sound) playSound(next.action.sound)
    initializedSound = true
    lastSoundKey = soundKey
    if (previous && previous.network.status !== next.network.status) {
      if (next.network.status === 'connected') notify('对手已连接，可以开始对弈。', 'success')
      else if (next.network.status === 'disconnected') notify(next.network.message || '与对手的连接已断开。', 'error')
    } else if (previous && next.network.message && next.network.message !== previous.network.message) {
      notify(next.network.message, 'info')
    }
  }

  async function initialize() {
    if (initializing || disposed) return
    initializing = true
    connection.value = 'loading'
    startupError.value = ''
    try {
      unlisten?.()
      unlisten = null
      const stop = await listen<Snapshot>('game:snapshot', event => acceptSnapshot(event.payload))
      if (disposed) { stop(); return }
      unlisten = stop
      const state = await invoke<Snapshot>('get_snapshot')
      acceptSnapshot(state)
    } catch (error) {
      if (!disposed) {
        connection.value = snapshot.value ? 'ready' : 'error'
        startupError.value = errorMessage(error)
      }
    } finally {
      initializing = false
    }
  }

  async function runSnapshot(command: string, args?: Record<string, unknown>, success?: string): Promise<boolean> {
    if (busy.value || disposed || !snapshot.value) return false
    busy.value = command
    commandError.value = ''
    try {
      const next = await invoke<Snapshot>(command, args)
      if (disposed) return false
      acceptSnapshot(next)
      if (success) notify(success, 'success')
      return true
    } catch (error) {
      if (!disposed) {
        commandError.value = errorMessage(error)
        playSound('Forbid')
      }
      return false
    } finally {
      busy.value = null
    }
  }

  async function makeMove(from: number, to: number) {
    const state = snapshot.value
    if (!state || !canPlay.value || moving.value) return
    moving.value = true
    moveWait = { revision: state.revision, gameId: state.gameId }
    commandError.value = ''
    try {
      const next = await invoke<Snapshot>('make_move', { from, to, revision: state.revision, gameId: state.gameId })
      // LAN guests receive an acknowledgement first; the next revision confirms the move.
      acceptSnapshot(next)
    } catch (error) {
      moveWait = null
      moving.value = false
      if (!disposed) {
        commandError.value = errorMessage(error)
        selected.value = null
        playSound('Forbid')
      }
    }
  }

  function selectSquare(square: number) {
    const state = snapshot.value
    if (!state || !canPlay.value) return
    const piece = state.board[square]
    if (selected.value === square) {
      selected.value = null
      return
    }
    if (selected.value !== null && legalTargets.value.has(square)) {
      void makeMove(selected.value, square)
      return
    }
    if (piece?.side === state.turn) {
      selected.value = square
      playSound('Choose')
      return
    }
    playSound('Forbid')
  }

  function newGame(settings: Settings) {
    return runSnapshot('new_game', { settings }, '新的一局，落子从容。')
  }

  async function peerCommand(command: string, args: Record<string, unknown>, action: ActionName | 'response') {
    const state = snapshot.value
    if (!state || busy.value || awaitingPeerAction.value) return false
    const wait = inLan.value && state.network.role === 'guest'
      ? { revision: state.revision, gameId: state.gameId } : null
    if (wait) { peerWait = wait; awaitingPeerAction.value = action }
    const success = await runSnapshot(command, args)
    if (!success && peerWait === wait) { peerWait = null; awaitingPeerAction.value = null }
    return success
  }

  function performAction(action: ActionName) {
    const state = snapshot.value
    if (!state) return Promise.resolve(false)
    return peerCommand('game_action', { action, revision: state.revision, gameId: state.gameId }, action)
  }

  function updatePreferences(changes: Partial<Preference>) {
    const settings = snapshot.value?.settings
    if (!settings) return Promise.resolve(false)
    return runSnapshot('update_preferences', {
      sound: changes.sound ?? settings.sound,
      showHints: changes.showHints ?? settings.showHints,
      flipped: changes.flipped ?? settings.flipped,
    })
  }

  function openPosition() {
    return runSnapshot('open_position')
  }

  async function savePosition() {
    if (busy.value || disposed || !snapshot.value) return false
    busy.value = 'save_position'
    commandError.value = ''
    try {
      const path = await invoke<string | null>('save_position')
      if (path) notify(`棋局已保存 · ${path.split(/[\\/]/).pop() ?? path}`, 'success')
      return !!path
    } catch (error) {
      if (!disposed) commandError.value = errorMessage(error)
      return false
    } finally {
      busy.value = null
    }
  }

  const hostLan = (port: number, settings: Settings) => runSnapshot('host_lan', { port, settings })
  const joinLan = (address: string, port: number) => runSnapshot('join_lan', { address, port })
  const leaveLan = () => runSnapshot('leave_lan', undefined, '已离开局域网房间。')
  function respondRequest(id: number, accept: boolean) {
    const state = snapshot.value
    if (!state) return Promise.resolve(false)
    return peerCommand('respond_request', { id, accept, revision: state.revision, gameId: state.gameId }, 'response')
  }

  onMounted(() => {
    clockTimer = setInterval(() => { clockNow.value = performance.now() }, 100)
    void initialize()
  })
  onUnmounted(() => {
    disposed = true
    unlisten?.()
    if (clockTimer !== undefined) clearInterval(clockTimer)
    for (const timer of toastTimers.values()) clearTimeout(timer)
    toastTimers.clear()
    for (const audio of audios.values()) {
      audio.pause()
      audio.removeAttribute('src')
      audio.load()
    }
    audios.clear()
  })

  return {
    snapshot, selected, busy, moving, awaitingPeerAction, connection, startupError, commandError, toasts,
    inLan, networkReady, canPlay, legalTargets, remainingMs, clockRunning,
    initialize, selectSquare, newGame, performAction, updatePreferences,
    openPosition, savePosition, hostLan, joinLan, leaveLan, respondRequest,
    notify, dismissToast,
    clearSelection: () => { selected.value = null },
    dismissError: () => { commandError.value = '' },
  }
}
