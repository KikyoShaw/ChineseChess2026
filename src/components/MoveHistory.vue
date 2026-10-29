<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from 'vue'
import type { MoveRecord, Piece } from '../types'
import { pieceImage, pieceName } from '../composables/chess'
import UiIcon from './UiIcon.vue'

const props = defineProps<{ history: MoveRecord[] }>()
const tab = ref<'history' | 'captured'>('history')
const scrollArea = ref<HTMLElement | null>(null)
const tabId = useId()
const rows = computed(() => {
  const result: { number: number; red: MoveRecord | null; black: MoveRecord | null }[] = []
  for (let index = 0; index < props.history.length; index += 2) {
    const pair = props.history.slice(index, index + 2)
    result.push({ number: index / 2 + 1, red: pair.find(move => move.side === 'red') ?? null, black: pair.find(move => move.side === 'black') ?? null })
  }
  return result
})
const captures = computed(() => props.history.map(move => move.captured).filter((piece): piece is Piece => piece !== null))
const redCaptures = computed(() => captures.value.filter(piece => piece.side === 'red'))
const blackCaptures = computed(() => captures.value.filter(piece => piece.side === 'black'))
const latestPly = computed(() => props.history[props.history.length - 1]?.ply)
watch([() => props.history.length, tab], async () => {
  await nextTick()
  if (tab.value === 'history' && scrollArea.value) scrollArea.value.scrollTop = scrollArea.value.scrollHeight
}, { flush: 'post' })
</script>

<template>
  <section class="history-panel" aria-label="对局记录">
    <header class="history-tabs" role="tablist" aria-label="记录类型">
      <button :id="`${tabId}-history-tab`" type="button" role="tab" :aria-selected="tab === 'history'" :aria-controls="`${tabId}-history-panel`" :class="{ active: tab === 'history' }" @click="tab = 'history'">棋谱<span class="tab-count">{{ history.length }}</span></button>
      <button :id="`${tabId}-captured-tab`" type="button" role="tab" :aria-selected="tab === 'captured'" :aria-controls="`${tabId}-captured-panel`" :class="{ active: tab === 'captured' }" @click="tab = 'captured'">吃子<span class="tab-count">{{ captures.length }}</span></button>
      <UiIcon name="book" :size="17" />
    </header>
    <div v-if="tab === 'history'" :id="`${tabId}-history-panel`" class="history-tab-content" role="tabpanel" :aria-labelledby="`${tabId}-history-tab`" tabindex="0">
      <template v-if="history.length">
        <div class="history-columns"><span>回合</span><span>红方</span><span>黑方</span></div>
        <div ref="scrollArea" class="history-scroll">
          <div v-for="row in rows" :key="row.number" class="history-row">
            <span class="move-number">{{ row.number.toString().padStart(2, '0') }}</span>
            <div class="move-cell red-move" :class="{ 'latest-move': row.red?.ply === latestPly }"><template v-if="row.red"><strong :title="row.red.notation">{{ row.red.notation }}<i v-if="row.red.checking" class="check-mark" title="将军">将</i></strong><time>{{ row.red.time }}</time></template><span v-else class="move-empty">—</span></div>
            <div class="move-cell" :class="{ 'latest-move': row.black?.ply === latestPly }"><template v-if="row.black"><strong :title="row.black.notation">{{ row.black.notation }}<i v-if="row.black.checking" class="check-mark" title="将军">将</i></strong><time>{{ row.black.time }}</time></template><span v-else class="move-empty">—</span></div>
          </div>
        </div>
      </template>
      <div v-else class="history-empty"><span class="empty-seal" aria-hidden="true">起</span><strong>好棋，从这一手开始</strong><p>落子后，棋谱会记录在这里。</p></div>
    </div>
    <div v-else :id="`${tabId}-captured-panel`" class="captured-tab-content" role="tabpanel" :aria-labelledby="`${tabId}-captured-tab`" tabindex="0">
      <div class="captured-group"><h3><i class="side-dot red" />红方损失<span>{{ redCaptures.length }} 枚</span></h3><div v-if="redCaptures.length" class="captured-pieces"><img v-for="(piece, index) in redCaptures" :key="`${piece.id}-${index}`" :src="pieceImage(piece)" :alt="`红方${pieceName(piece)}`" :title="`红方${pieceName(piece)}`" draggable="false" /></div><p v-else>尚无棋子被吃</p></div>
      <div class="captured-group"><h3><i class="side-dot black" />黑方损失<span>{{ blackCaptures.length }} 枚</span></h3><div v-if="blackCaptures.length" class="captured-pieces"><img v-for="(piece, index) in blackCaptures" :key="`${piece.id}-${index}`" :src="pieceImage(piece)" :alt="`黑方${pieceName(piece)}`" :title="`黑方${pieceName(piece)}`" draggable="false" /></div><p v-else>尚无棋子被吃</p></div>
    </div>
  </section>
</template>
