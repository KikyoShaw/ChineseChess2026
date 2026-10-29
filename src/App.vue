<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import type { Settings, Side } from './types'
import { useGame } from './composables/useGame'
import { clockText, pieceImage, reasonText, requestNames, sideName } from './composables/chess'
import AppDialog from './components/AppDialog.vue'
import ChessBoard from './components/ChessBoard.vue'
import LanDialog from './components/LanDialog.vue'
import MoveHistory from './components/MoveHistory.vue'
import SettingsDialog from './components/SettingsDialog.vue'
import UiIcon from './components/UiIcon.vue'

const {
  snapshot: game, selected, busy, moving, awaitingPeerAction, connection, startupError, commandError, toasts,
  inLan, networkReady, canPlay, legalTargets, remainingMs, clockRunning,
  initialize, selectSquare, clearSelection, newGame, performAction, updatePreferences,
  openPosition, savePosition, hostLan, joinLan, leaveLan, respondRequest,
  notify, dismissToast, dismissError,
} = useGame()
const dialog = ref<'settings' | 'lan' | 'about' | null>(null)
const confirmation = ref<'restart' | 'resign' | 'draw' | 'open' | null>(null)
const isBusy = computed(() => busy.value !== null || moving.value)
const requestBusy = computed(() => isBusy.value || awaitingPeerAction.value !== null)
const pending = computed(() => game.value?.pendingRequest ?? null)
const incoming = computed(() => !!pending.value && game.value?.network.localSide !== null && pending.value.from !== game.value?.network.localSide)
const dismissedRequestId = ref<number | null>(null)
const requestDialogOpen = computed(() => incoming.value && dismissedRequestId.value !== pending.value?.id)
const canRequest = computed(() => !!game.value && !requestBusy.value && networkReady.value && !pending.value)
const canUndo = computed(() => canRequest.value && (game.value?.history.length ?? 0) > 0)
const canEnd = computed(() => canRequest.value && !game.value?.result)
const canOpen = computed(() => !!game.value && !inLan.value && !isBusy.value)
const modeName = computed(() => !game.value ? '准备棋局' : inLan.value ? '局域网对弈' : game.value.settings.mode === 'computer' ? '人机对弈' : '本地双人')
const difficultyLabels: Record<number, string> = { 1: '入门', 2: '进阶', 3: '挑战' }
const difficultyName = computed(() => difficultyLabels[game.value?.settings.difficulty ?? 2] ?? '进阶')
const roundNumber = computed(() => Math.floor((game.value?.history.length ?? 0) / 2) + 1)
const lastMove = computed(() => game.value?.history[game.value.history.length - 1] ?? null)
const resultTitle = computed(() => !game.value?.result ? '' : game.value.result.winner ? `${sideName(game.value.result.winner)}胜` : '和棋')
const lowTime = computed(() => remainingMs.value !== null && remainingMs.value <= 10000 && !game.value?.result && clockRunning.value)
const resigningSide = computed<Side>(() => {
  if (!game.value) return 'red'
  if (inLan.value) return game.value.network.localSide ?? game.value.turn
  return game.value.settings.mode === 'computer' ? game.value.settings.humanSide : game.value.turn
})
const actionHint = computed(() => pending.value ? '请求处理中，请等待对手回应。' : !networkReady.value ? '等待连接完成后可用。' : '')
const openHint = computed(() => inLan.value ? '请先离开局域网房间，再打开棋局。' : '打开棋局 · Ctrl+O')
const restartHint = computed(() => actionHint.value || (inLan.value ? '向对手申请重新开局' : '新对局 · Ctrl+N'))
const turnHint = computed(() => {
  const state = game.value
  if (!state) return ''
  if (state.result) return reasonText(state.result.reason)
  if (inLan.value && state.network.status !== 'connected') {
    return { offline: '等待建立连接', hosting: '等待对手加入房间', connecting: '正在连接对手', disconnected: '连接中断，棋局已暂停' }[state.network.status] ?? ''
  }
  if (pending.value) return incoming.value ? `对手请求${requestNames[pending.value.kind]}` : `已申请${requestNames[pending.value.kind]}，等待回应`
  if (state.thinking) return '电脑正在思考，请稍候'
  if (state.inCheck) return '将军，请应将'
  if (state.settings.mode === 'local') return '两位棋手轮流行棋'
  return canPlay.value ? '轮到你了，从容落子' : '等待对手落子'
})
const boardHint = computed(() => {
  if (game.value?.result) return '本局已结束，可保存局面或再来一局。'
  if (pending.value) return '请求等待回应中；仍可落子，落子后请求失效。'
  if (awaitingPeerAction.value) return '请求已发送，等待房间同步。'
  if (!networkReady.value || game.value?.thinking) return turnHint.value
  if (selected.value !== null) return '请选择落点；再次点选或按 Esc 取消。'
  return canPlay.value ? '点选棋子，再点落点。' : turnHint.value
})
const networkLabel = computed(() => {
  if (!game.value) return connection.value === 'error' ? '本地服务未连接' : '正在连接本地服务'
  if (!inLan.value) return game.value.settings.mode === 'local' ? '本地双人 · 无需联网' : '本地引擎 · 人机对弈'
  return { offline: '局域网未连接', hosting: '房间已创建 · 等待对手', connecting: '正在连接房间', connected: '局域网已连接', disconnected: '局域网连接已断开' }[game.value.network.status]
})
const networkClass = computed(() => {
  if (!game.value) return connection.value === 'error' ? 'error' : 'waiting'
  if (!inLan.value || game.value.network.status === 'connected') return 'online'
  return game.value.network.status === 'disconnected' ? 'error' : 'waiting'
})
const players = computed(() => {
  const state = game.value
  if (!state) return []
  const sides: Side[] = state.settings.flipped ? ['red', 'black'] : ['black', 'red']
  return sides.map(side => {
    const isMe = inLan.value ? side === state.network.localSide : state.settings.mode === 'computer' && side === state.settings.humanSide
    const isComputer = !inLan.value && state.settings.mode === 'computer' && !isMe
    let name = isComputer ? '电脑棋手' : isMe ? '我方' : `${sideName(side)}棋手`
    let detail = isComputer ? `${difficultyName.value}难度` : '本地玩家'
    if (inLan.value) {
      name = isMe ? '我方' : state.network.status === 'connected' ? '对手' : '等待对手'
      detail = isMe ? state.network.role === 'host' ? '房主' : '访客' : state.network.role === 'host' ? '访客' : '房主'
    }
    return { side, name, detail, isMe, isComputer, active: state.turn === side && !state.result && networkReady.value, image: pieceImage({ side, kind: 'king' }) }
  })
})
const confirmCopy = computed(() => {
  switch (confirmation.value) {
    case 'resign': return { title: '确认认输？', description: `${sideName(resigningSide.value)}将认输，本局对弈随即结束。胜负之外，亦有收获。`, label: '确认认输', icon: 'flag', danger: true }
    case 'draw': return { title: inLan.value ? '向对手提和？' : '以和棋结束本局？', description: inLan.value ? '对手同意后，本局将以和棋结束。等待期间继续计时；落子后请求自动失效。' : game.value?.settings.mode === 'local' ? '请确认两位棋手均同意和棋。确认后，本局对弈结束。' : '电脑将接受和棋，本局对弈随即结束。', label: inLan.value ? '发送和棋请求' : '确认和棋', icon: 'draw', danger: false }
    case 'open': return { title: '打开已保存的棋局？', description: '所选棋局将替换当前对局。取消文件选择会保留现有棋局。', label: '选择棋局文件', icon: 'folder', danger: false }
    default: return { title: inLan.value ? '申请重新开局？' : '重新开一局？', description: inLan.value ? '将向对手发送重开请求。对手同意后，双方沿用当前房间设置重新开始。' : '当前对局将被新局替换，新局沿用当前设置。想留住这盘棋，可以先保存。', label: inLan.value ? '发送重开请求' : '重新开局', icon: 'plus', danger: false }
  }
})

function showDialog(name: 'settings' | 'lan' | 'about') {
  if (isBusy.value || requestDialogOpen.value) return
  dismissError()
  dialog.value = name
}
function ask(kind: 'restart' | 'resign' | 'draw' | 'open') {
  if (kind === 'open' ? !canOpen.value : kind === 'restart' ? !canRequest.value : !canEnd.value) return
  dismissError()
  confirmation.value = kind
}
async function confirmAction() {
  const kind = confirmation.value
  const state = game.value
  if (!kind || !state || isBusy.value) return
  let success: boolean
  if (kind === 'restart') success = inLan.value ? await performAction('restart') : await newGame({ ...state.settings })
  else if (kind === 'open') success = await openPosition()
  else success = await performAction(kind)
  if (success) confirmation.value = null
}
async function undo() {
  if (!canUndo.value) return
  if (await performAction('undo') && !inLan.value) notify('已悔棋，可以重新落子。', 'success')
}
async function flip() {
  if (isBusy.value || !game.value) return
  await updatePreferences({ flipped: !game.value.settings.flipped })
}
async function toggleSound() {
  if (isBusy.value || !game.value) return
  await updatePreferences({ sound: !game.value.settings.sound })
}
async function applySettings(settings: Settings) {
  const current = game.value?.settings
  if (!current) return
  const changed = !inLan.value && (
    settings.mode !== current.mode || settings.humanSide !== current.humanSide || settings.firstSide !== current.firstSide ||
    settings.difficulty !== current.difficulty || settings.stepSeconds !== current.stepSeconds
  )
  const success = changed ? await newGame(settings) : await updatePreferences({ sound: settings.sound, showHints: settings.showHints, flipped: settings.flipped })
  if (success) {
    dialog.value = null
    if (!changed) notify('下棋偏好已保存。', 'success')
  }
}
async function createRoom(port: number, settings: Settings) {
  if (await hostLan(port, settings) && game.value?.network.status === 'connected') dialog.value = null
}
async function enterRoom(address: string, port: number) {
  if (await joinLan(address, port) && game.value?.network.status === 'connected') dialog.value = null
}
async function exitRoom() {
  if (await leaveLan()) dialog.value = null
}
async function answerRequest(accept: boolean) {
  const request = pending.value
  if (!request || !incoming.value) return
  if (await respondRequest(request.id, accept)) notify('回应已发送，棋局将同步更新。', 'info')
}

watch(() => game.value?.pendingRequest?.id, () => {
  if (incoming.value) { dismissedRequestId.value = null; dialog.value = null; confirmation.value = null; dismissError() }
})
watch(() => game.value?.network.status, status => {
  if (status === 'connected' && dialog.value === 'lan') dialog.value = null
})
watch(() => game.value?.gameId, () => { confirmation.value = null })
watch(() => game.value?.result, result => {
  if (result && (confirmation.value === 'draw' || confirmation.value === 'resign')) confirmation.value = null
})

function shortcuts(event: KeyboardEvent) {
  if (event.repeat || dialog.value || confirmation.value || requestDialogOpen.value) return
  const target = event.target
  if (target instanceof HTMLElement && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName))) return
  const key = event.key.toLowerCase()
  if (event.ctrlKey || event.metaKey) {
    if (!['n', 'z', 'o', 's', ','].includes(key)) return
    event.preventDefault()
    if (key === 'n') ask('restart')
    else if (key === 'z') void undo()
    else if (key === 'o') { if (inLan.value) notify('请先离开局域网房间，再打开棋局。'); else ask('open') }
    else if (key === 's') void savePosition()
    else if (key === ',' && game.value) showDialog('settings')
  } else if (key === 'f' && !event.altKey) {
    event.preventDefault()
    void flip()
  } else if (event.key === 'Escape') clearSelection()
}
onMounted(() => document.addEventListener('keydown', shortcuts))
onUnmounted(() => document.removeEventListener('keydown', shortcuts))
</script>

<template>
  <div class="app-shell">
    <header class="app-header">
      <div class="brand"><span class="brand-seal" aria-hidden="true">弈</span><div><h1>弈 <span>·</span> 中国象棋</h1><p>方寸之间，自有乾坤</p></div></div>
      <div class="header-right"><span class="header-motto">以棋会友 · 落子有声</span><button class="header-about" type="button" title="关于、来源与对弈规则" :disabled="isBusy || requestDialogOpen" @click="showDialog('about')"><UiIcon name="info" :size="18" /><span>关于</span></button></div>
    </header>

    <nav class="main-toolbar" aria-label="对局工具栏">
      <div class="toolbar-group">
        <button class="button button-primary toolbar-new" type="button" :disabled="!canRequest" :title="restartHint" @click="ask('restart')"><UiIcon name="plus" :size="19" /><span>{{ inLan ? '申请重开' : '新对局' }}</span></button>
        <button class="toolbar-button" type="button" :disabled="!canUndo" :title="actionHint || '悔棋 · Ctrl+Z'" @click="undo"><UiIcon name="undo" /><span>悔棋</span></button>
        <button class="toolbar-button" type="button" :disabled="!game || isBusy" title="翻转棋盘 · F" :aria-pressed="game?.settings.flipped ?? false" @click="flip"><UiIcon name="flip" /><span>翻转</span></button>
        <span class="toolbar-divider" />
        <button class="toolbar-button" type="button" :disabled="!canOpen" :title="openHint" @click="ask('open')"><UiIcon name="folder" /><span>打开</span></button>
        <button class="toolbar-button" type="button" :disabled="!game || isBusy" title="保存棋局 · Ctrl+S" @click="savePosition"><UiIcon name="save" /><span>保存</span></button>
      </div>
      <div class="toolbar-group">
        <button class="toolbar-button toolbar-sound" type="button" :disabled="!game || isBusy" :title="game?.settings.sound ? '关闭音效' : '开启音效'" :aria-label="game?.settings.sound ? '关闭音效' : '开启音效'" :aria-pressed="game?.settings.sound ?? false" @click="toggleSound"><UiIcon :name="game?.settings.sound ? 'sound' : 'mute'" /></button>
        <button class="toolbar-button" type="button" :disabled="!game || isBusy" title="对局设置 · Ctrl+," @click="showDialog('settings')"><UiIcon name="settings" /><span>设置</span></button>
        <span class="toolbar-divider" />
        <button class="toolbar-button toolbar-lan" :class="{ 'lan-active': inLan }" type="button" :disabled="!game || isBusy" title="创建、加入或离开局域网房间" @click="showDialog('lan')"><UiIcon name="lan" /><span>局域网</span><i v-if="inLan" class="connection-indicator" :class="networkClass" /></button>
      </div>
    </nav>

    <main v-if="game" class="workspace">
      <section class="board-panel" aria-label="对弈区">
        <div class="board-meta"><div><span class="section-caption">棋 局</span><span class="mode-badge">{{ modeName }}</span><span class="first-side">{{ sideName(game.settings.firstSide) }}先行</span></div><span class="round-label">第 <strong>{{ roundNumber.toString().padStart(2, '0') }}</strong> 回合</span></div>
        <div class="board-stage">
          <ChessBoard :board="game.board" :flipped="game.settings.flipped" :selected="selected" :legal-targets="legalTargets" :show-hints="game.settings.showHints" :last-move="lastMove" :in-check="game.inCheck && !game.result" :turn="game.turn" :interactive="canPlay" @select="selectSquare" @cancel="clearSelection" />
        </div>
        <div class="board-foot"><span class="board-instruction" :class="{ 'text-red': game.inCheck && !game.result }"><i class="instruction-dot" />{{ boardHint }}</span><span class="keyboard-hint"><kbd>↑↓←→</kbd> 选位 <kbd>Enter</kbd> 落子</span></div>
      </section>

      <aside class="game-sidebar" aria-label="对局信息">
        <section class="players-panel" aria-label="棋手与当前回合">
          <template v-for="(player, index) in players" :key="player.side">
            <div class="player-card" :class="[{ 'player-active': player.active }, `player-${player.side}`]">
              <div class="player-avatar"><img :src="player.image" :alt="sideName(player.side)" draggable="false" /></div>
              <div class="player-info"><div><strong>{{ player.name }}</strong><span v-if="player.isMe" class="me-badge">我</span></div><p><UiIcon :name="player.isComputer ? 'computer' : inLan ? 'lan' : 'person'" :size="12" />{{ player.detail }}</p></div>
              <div class="player-side"><span :class="player.side === 'red' ? 'text-red' : ''">{{ sideName(player.side) }}</span><small v-if="player.active">行棋中</small><small v-else>{{ index === 0 ? '上方' : '下方' }}</small></div>
            </div>
            <div v-if="index === 0" class="turn-panel" :class="{ 'turn-red': game.turn === 'red', 'turn-result': !!game.result }">
              <div class="turn-main"><div><p class="eyebrow">{{ game.result ? '本局结果' : '当前行棋' }}</p><h2>{{ game.result ? resultTitle : `${sideName(game.turn)}行棋` }}<span v-if="game.thinking && !game.result" class="thinking-dots" aria-label="电脑思考中"><i /><i /><i /></span><span v-else-if="game.inCheck && !game.result" class="check-badge">将军</span></h2></div><div class="turn-clock" :class="{ 'clock-low': lowTime, 'clock-paused': !clockRunning }"><span>{{ game.result ? '已结束' : game.settings.stepSeconds ? '每步剩余' : '从容思考' }}</span><strong :aria-label="`每步剩余 ${clockText(remainingMs)}`">{{ game.result ? '终局' : remainingMs === null ? '∞' : clockText(remainingMs) }}</strong></div></div>
              <p class="turn-hint" aria-live="polite">{{ turnHint }}</p>
            </div>
          </template>
        </section>
        <div v-if="pending || awaitingPeerAction" class="request-waiting" role="status"><span class="spinner" /><span>{{ pending ? `${incoming ? '对手请求' : '已申请'}${requestNames[pending.kind]}` : '请求已发送' }}<small>继续计时；落子后请求失效</small></span><button v-if="incoming" class="request-view-button" type="button" @click="dismissedRequestId = null">查看</button></div>
        <MoveHistory :history="game.history" />
        <div class="match-actions"><button class="button button-quiet" type="button" :disabled="!canEnd" :title="actionHint || '提出和棋'" @click="ask('draw')"><UiIcon name="draw" :size="17" />提和</button><button class="button button-quiet resign-button" type="button" :disabled="!canEnd" :title="actionHint || `${sideName(resigningSide)}认输`" @click="ask('resign')"><UiIcon name="flag" :size="17" />认输</button></div>
      </aside>
    </main>

    <main v-else class="connection-screen" aria-live="polite">
      <div class="connection-art" aria-hidden="true"><span>弈</span></div>
      <template v-if="connection === 'error'">
        <p class="eyebrow">棋盘在这里，等你入局</p><h2>尚未连接本地棋局服务</h2><p>请在原生应用窗口中使用。开发时，请从项目目录运行 <code>npm run tauri dev</code>。</p><p v-if="startupError" class="startup-detail">{{ startupError }}</p><button class="button button-primary" type="button" @click="initialize"><UiIcon name="retry" :size="18" />重新连接</button>
      </template>
      <template v-else><p class="eyebrow">一局好棋，即将开始</p><h2>正在准备棋盘</h2><p class="connecting-label"><span class="spinner" />连接本地棋局服务…</p></template>
    </main>

    <footer class="status-bar"><div class="status-left" :title="game?.network.message || networkLabel"><i class="connection-indicator" :class="networkClass" /><span>{{ networkLabel }}</span><span v-if="game && inLan && (game.network.message || game.network.address)" class="status-address">{{ game.network.message || game.network.address }}</span></div><div class="status-right"><span v-if="busy" class="status-working"><span class="spinner" />正在处理</span><span v-else-if="moving" class="status-working">正在落子</span><span v-else-if="game?.result" class="status-result">{{ resultTitle }} · {{ reasonText(game.result.reason) }}</span><span v-else-if="game">{{ game.settings.showHints ? '落点提示已开启' : '落点提示已关闭' }}<span class="status-separator">·</span>{{ game.settings.stepSeconds ? `每步 ${game.settings.stepSeconds} 秒` : '不限时对局' }}</span><span v-else>中国象棋 · 桌面版</span></div></footer>

    <div v-if="commandError && !dialog && !confirmation && !requestDialogOpen" class="error-banner" role="alert"><UiIcon name="alert" :size="18" /><span>{{ commandError }}</span><button class="icon-button" type="button" aria-label="关闭错误提示" @click="dismissError"><UiIcon name="close" :size="17" /></button></div>
    <div class="toast-stack" aria-live="polite" aria-relevant="additions"><TransitionGroup name="toast"><div v-for="toast in toasts" :key="toast.id" class="toast" :class="`toast-${toast.kind}`"><UiIcon :name="toast.kind === 'success' ? 'check' : toast.kind === 'error' ? 'alert' : 'info'" :size="18" /><span>{{ toast.text }}</span><button class="icon-button" type="button" aria-label="关闭通知" @click="dismissToast(toast.id)"><UiIcon name="close" :size="15" /></button></div></TransitionGroup></div>

    <SettingsDialog v-if="game" :open="dialog === 'settings'" :settings="game.settings" :busy="isBusy" :lan-locked="inLan" :error="commandError" @close="dialog = null" @save="applySettings" />
    <LanDialog v-if="game" :open="dialog === 'lan'" :settings="game.settings" :network="game.network" :active="inLan" :busy="isBusy" :error="commandError" @close="dialog = null" @host="createRoom" @join="enterRoom" @leave="exitRoom" />

    <AppDialog :open="confirmation !== null" :title="confirmCopy.title" eyebrow="落子之前，再想一步" size="small" :dismissible="!isBusy" @close="confirmation = null">
      <div class="confirmation-symbol" :class="{ danger: confirmCopy.danger }"><UiIcon :name="confirmCopy.icon" :size="27" /></div><p class="confirmation-description">{{ confirmCopy.description }}</p><p v-if="commandError" class="form-error" role="alert">{{ commandError }}</p>
      <template #footer><button class="button button-quiet" type="button" :disabled="isBusy" data-autofocus @click="confirmation = null">再想想</button><button class="button" :class="confirmCopy.danger ? 'button-danger' : 'button-primary'" type="button" :disabled="isBusy" @click="confirmAction"><span v-if="isBusy" class="spinner" />{{ isBusy ? '正在处理' : confirmCopy.label }}</button></template>
    </AppDialog>

    <AppDialog :open="requestDialogOpen" :title="pending ? `对手请求${requestNames[pending.kind]}` : '对手的请求'" eyebrow="棋逢对手，以礼相待" size="small" :dismissible="!requestBusy" @close="dismissedRequestId = pending?.id ?? null">
      <div class="confirmation-symbol"><UiIcon :name="pending?.kind === 'undo' ? 'undo' : pending?.kind === 'draw' ? 'draw' : 'plus'" :size="27" /></div><p class="confirmation-description">{{ pending?.kind === 'undo' ? '对手希望退回最近一回合（最多两步）。是否同意此次悔棋？' : pending?.kind === 'draw' ? '对手提议以和棋结束本局。是否同意？' : '对手希望沿用房间设置，重新开始一盘棋。是否同意？' }}</p><p class="form-note">请求期间继续计时。可以收起窗口稍后回应，继续落子会使请求失效。</p><p v-if="commandError" class="form-error" role="alert">{{ commandError }}</p>
      <template #footer><button class="button button-quiet" type="button" :disabled="requestBusy" data-autofocus @click="answerRequest(false)">谢绝</button><button class="button button-primary" type="button" :disabled="requestBusy" @click="answerRequest(true)"><span v-if="requestBusy" class="spinner" /><UiIcon v-else name="check" :size="18" />{{ requestBusy ? '正在同步' : '同意' }}</button></template>
    </AppDialog>

    <AppDialog :open="dialog === 'about'" title="弈 · 中国象棋" eyebrow="一方棋盘，万千变化" @close="dialog = null">
      <div class="about-intro"><span class="about-seal" aria-hidden="true">弈</span><p>一款安静、专注的桌面象棋。<br />与电脑切磋，与朋友对坐，让好棋慢慢发生。</p></div>
      <section class="about-section"><h3>关于本作</h3><p>基于 Tauri 2、Vue 3 与 TypeScript 构建的独立桌面象棋应用，支持离线人机、本地双人与局域网对弈。</p><p>资源许可信息见 <code>THIRD_PARTY_NOTICES.md</code>。</p></section>
      <section class="about-section"><h3>重复局面规则</h3><p>相同的棋子布局与行棋方出现三次，自动判为和棋。这是娱乐对弈规则，未做赛事级长将、长捉裁定。</p></section>
      <section class="about-section"><h3>棋盘标记与快捷键</h3><div class="board-legend"><span><i class="legend-selection" />选中棋子</span><span><i class="legend-target" />合法落点</span><span><i class="legend-last" />上一步</span><span><i class="legend-check" />被将军</span></div><p class="shortcut-line"><kbd>Ctrl N</kbd> 新局 <kbd>Ctrl Z</kbd> 悔棋 <kbd>F</kbd> 翻转</p><p class="shortcut-line"><kbd>Ctrl O</kbd> 打开 <kbd>Ctrl S</kbd> 保存 <kbd>Esc</kbd> 取消选择</p></section>
      <template #footer><button class="button button-primary" type="button" @click="dialog = null">回到棋盘<UiIcon name="arrow" :size="17" /></button></template>
    </AppDialog>
  </div>
</template>
