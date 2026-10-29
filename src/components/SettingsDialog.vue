<script setup lang="ts">
import { computed, reactive, useId, watch } from 'vue'
import type { Settings } from '../types'
import AppDialog from './AppDialog.vue'
import UiIcon from './UiIcon.vue'

const props = defineProps<{ open: boolean; settings: Settings; busy: boolean; lanLocked: boolean; error: string }>()
const emit = defineEmits<{ close: []; save: [settings: Settings] }>()
const draft = reactive<Settings>({ ...props.settings })
const formId = useId()
const difficulties = [
  { value: 1, name: '入门', detail: '轻松练习' },
  { value: 2, name: '进阶', detail: '见招拆招' },
  { value: 3, name: '挑战', detail: '认真一局' },
]
const matchChanged = computed(() => !props.lanLocked && (
  draft.mode !== props.settings.mode || draft.humanSide !== props.settings.humanSide ||
  draft.firstSide !== props.settings.firstSide || draft.difficulty !== props.settings.difficulty ||
  draft.stepSeconds !== props.settings.stepSeconds
))
const validTime = computed(() => Number.isInteger(draft.stepSeconds) && draft.stepSeconds >= 0 && draft.stepSeconds <= 600)
watch(() => props.open, open => { if (open) Object.assign(draft, props.settings) }, { immediate: true })
function submit() {
  if (!props.busy && validTime.value) emit('save', { ...draft })
}
</script>

<template>
  <AppDialog :open="open" title="对局设置" eyebrow="让每一局，都合心意" :dismissible="!busy" @close="emit('close')">
    <form :id="formId" class="settings-form" @submit.prevent="submit">
      <p v-if="lanLocked" class="notice"><UiIcon name="lan" :size="18" /> 局域网房间内可调整提示与声音；更换对局选项，请先离开房间。</p>
      <fieldset class="setting-section" :disabled="busy || lanLocked">
        <legend>对弈方式</legend>
        <div class="mode-options">
          <button type="button" class="mode-option" :class="{ active: draft.mode === 'computer' }" :aria-pressed="draft.mode === 'computer'" @click="draft.mode = 'computer'">
            <UiIcon name="computer" :size="24" /><span><strong>人机对弈</strong><small>与电脑切磋棋艺</small></span><span class="option-radio" />
          </button>
          <button type="button" class="mode-option" :class="{ active: draft.mode === 'local' }" :aria-pressed="draft.mode === 'local'" @click="draft.mode = 'local'">
            <UiIcon name="person" :size="24" /><span><strong>本地双人</strong><small>同一棋盘，面对面</small></span><span class="option-radio" />
          </button>
        </div>
        <div class="setting-row">
          <div class="setting-label"><span>我执哪方</span><small>{{ draft.mode === 'local' ? '本地双人可操作双方' : '仅用于人机对弈' }}</small></div>
          <div class="segmented" role="group" aria-label="玩家执子颜色">
            <button type="button" :disabled="draft.mode !== 'computer'" :class="{ active: draft.humanSide === 'red' }" :aria-pressed="draft.humanSide === 'red'" @click="draft.humanSide = 'red'"><i class="side-dot red" />红方</button>
            <button type="button" :disabled="draft.mode !== 'computer'" :class="{ active: draft.humanSide === 'black' }" :aria-pressed="draft.humanSide === 'black'" @click="draft.humanSide = 'black'"><i class="side-dot black" />黑方</button>
          </div>
        </div>
        <div class="setting-row">
          <div class="setting-label"><span>先行一方</span><small>新局开始后立即计时</small></div>
          <div class="segmented" role="group" aria-label="先行一方">
            <button type="button" :class="{ active: draft.firstSide === 'red' }" :aria-pressed="draft.firstSide === 'red'" @click="draft.firstSide = 'red'">红先</button>
            <button type="button" :class="{ active: draft.firstSide === 'black' }" :aria-pressed="draft.firstSide === 'black'" @click="draft.firstSide = 'black'">黑先</button>
          </div>
        </div>
        <div class="difficulty-setting" :class="{ muted: draft.mode !== 'computer' }">
          <label class="field-label">电脑难度</label>
          <div class="difficulty-options" role="group" aria-label="电脑难度">
            <button v-for="difficulty in difficulties" :key="difficulty.value" type="button" :disabled="draft.mode !== 'computer'" :class="{ active: draft.difficulty === difficulty.value }" :aria-pressed="draft.difficulty === difficulty.value" @click="draft.difficulty = difficulty.value"><strong>{{ difficulty.name }}</strong><small>{{ difficulty.detail }}</small></button>
          </div>
        </div>
        <div class="setting-row time-setting">
          <div class="setting-label"><label :for="`${formId}-seconds`">每步用时</label><small>0 为不限时，最多 600 秒</small></div>
          <div class="number-field"><input :id="`${formId}-seconds`" v-model.number="draft.stepSeconds" type="number" min="0" max="600" step="1" required :aria-invalid="!validTime" /><span>秒 / 步</span></div>
        </div>
        <p v-if="!validTime" class="field-error" role="alert">每步用时须为 0 至 600 的整数。</p>
      </fieldset>
      <fieldset class="setting-section preference-section" :disabled="busy">
        <legend>下棋偏好</legend>
        <label class="switch-row"><span class="setting-label"><span>落点提示</span><small>选中棋子后，显示合法落点</small></span><input v-model="draft.showHints" class="switch-input" type="checkbox" /><span class="switch-track" aria-hidden="true" /></label>
        <label class="switch-row"><span class="setting-label"><span>对局音效</span><small>落子、吃子与胜负声音</small></span><input v-model="draft.sound" class="switch-input" type="checkbox" /><span class="switch-track" aria-hidden="true" /></label>
      </fieldset>
      <p v-if="error" class="form-error" role="alert"><UiIcon name="alert" :size="17" />{{ error }}</p>
      <p class="form-note">{{ matchChanged ? '对局选项已改变，应用后会开始一盘新棋。' : '声音与落点提示会用于当前对局。' }}</p>
    </form>
    <template #footer>
      <button class="button button-quiet" type="button" :disabled="busy" @click="emit('close')">取消</button>
      <button class="button button-primary" type="submit" :form="formId" :disabled="busy || !validTime"><span v-if="busy" class="spinner" />{{ busy ? '正在应用' : matchChanged ? '应用并开始新局' : '保存设置' }}</button>
    </template>
  </AppDialog>
</template>
