<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from 'vue'
import type { ChessMove, Piece } from '../types'
import { pieceImage, pieceName, sideName, squareName } from '../composables/chess'

const props = defineProps<{
  board: (Piece | null)[]
  flipped: boolean
  selected: number | null
  legalTargets: Set<number>
  showHints: boolean
  lastMove: ChessMove | null
  inCheck: boolean
  turn: 'red' | 'black'
  interactive: boolean
}>()
const emit = defineEmits<{ select: [square: number]; cancel: [] }>()
const root = ref<HTMLElement | null>(null)
const focusDisplay = ref(85)
const helpId = useId()
const squares = computed(() => Array.from({ length: 90 }, (_, display) => {
  const actual = props.flipped ? 89 - display : display
  return { display, actual, piece: props.board[actual] ?? null }
}))
const pieces = computed(() => squares.value.flatMap(square => square.piece ? [{ ...square, piece: square.piece }] : []))
const position = (display: number) => ({
  left: `${(32 + 57 * (display % 9)) / 521 * 100}%`,
  top: `${(32 + 57 * Math.floor(display / 9)) / 577 * 100}%`,
})

function label(square: typeof squares.value[number]) {
  const content = square.piece ? `${sideName(square.piece.side)}${pieceName(square.piece)}` : '空位'
  const suffix = props.selected === square.actual ? '，已选中' : props.selected !== null && props.legalTargets.has(square.actual) ? '，可落子' : ''
  return `${squareName(square.actual)}，${content}${suffix}`
}

function clickSquare(display: number, actual: number) {
  focusDisplay.value = display
  if (props.interactive) emit('select', actual)
}

function onKeydown(event: KeyboardEvent, display: number) {
  if (event.key === 'Escape') { event.preventDefault(); emit('cancel'); return }
  let target = display
  const col = display % 9
  const row = Math.floor(display / 9)
  switch (event.key) {
    case 'ArrowLeft': target = row * 9 + Math.max(0, col - 1); break
    case 'ArrowRight': target = row * 9 + Math.min(8, col + 1); break
    case 'ArrowUp': target = Math.max(0, row - 1) * 9 + col; break
    case 'ArrowDown': target = Math.min(9, row + 1) * 9 + col; break
    case 'Home': target = event.ctrlKey ? 0 : row * 9; break
    case 'End': target = event.ctrlKey ? 89 : row * 9 + 8; break
    default: return
  }
  event.preventDefault()
  focusDisplay.value = target
  root.value?.querySelector<HTMLButtonElement>(`[data-display="${target}"]`)?.focus()
}

watch(() => props.flipped, async () => {
  const hadFocus = root.value?.contains(document.activeElement)
  focusDisplay.value = 89 - focusDisplay.value
  if (hadFocus) {
    await nextTick()
    root.value?.querySelector<HTMLButtonElement>(`[data-display="${focusDisplay.value}"]`)?.focus()
  }
})
</script>

<template>
  <div class="board-frame">
    <div ref="root" class="chessboard" :class="{ 'board-inactive': !interactive }" role="group" aria-label="中国象棋棋盘" :aria-describedby="helpId">
      <img class="board-wood" src="/assets/images/WOOD.GIF" alt="" draggable="false" />
      <div class="river-label" aria-hidden="true"><span>楚 河</span><span>汉 界</span></div>
      <div class="piece-layer" aria-hidden="true">
        <div v-for="square in pieces" :key="square.piece.id" class="chess-piece" :class="{ 'piece-check': inCheck && square.piece.kind === 'king' && square.piece.side === turn }" :style="position(square.display)">
          <img :src="pieceImage(square.piece)" alt="" draggable="false" />
        </div>
      </div>
      <button
        v-for="square in squares"
        :key="square.actual"
        class="board-square"
        :class="{
          'square-selected': selected === square.actual,
          'square-last-from': lastMove?.from === square.actual,
          'square-last-to': lastMove?.to === square.actual,
          'square-capture': !!square.piece,
          'square-legal': showHints && selected !== null && legalTargets.has(square.actual),
        }"
        :style="position(square.display)"
        :data-display="square.display"
        :tabindex="focusDisplay === square.display ? 0 : -1"
        :aria-label="label(square)"
        :aria-pressed="selected === square.actual"
        :aria-disabled="!interactive"
        type="button"
        @focus="focusDisplay = square.display"
        @click="clickSquare(square.display, square.actual)"
        @keydown="onKeydown($event, square.display)"
      ><span class="square-marker" aria-hidden="true" /><span class="legal-marker" aria-hidden="true" /></button>
      <p :id="helpId" class="sr-only">方向键移动焦点，回车或空格选择棋子和落点，Escape 取消选择。{{ flipped ? '棋盘已翻转，红方在上。' : '红方在下。' }}</p>
    </div>
  </div>
</template>
