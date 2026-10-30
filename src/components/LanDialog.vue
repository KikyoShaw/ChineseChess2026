<script setup lang="ts">
import { computed, ref, useId, watch } from 'vue'
import type { GamePhase, NetworkView, Settings, Side } from '../types'
import AppDialog from './AppDialog.vue'
import UiIcon from './UiIcon.vue'

const props = defineProps<{ open: boolean; phase: GamePhase; settings: Settings; network: NetworkView; active: boolean; busy: boolean; error: string }>()
const emit = defineEmits<{
  close: []
  host: [port: number, settings: Settings]
  join: [address: string, port: number]
  leave: []
}>()
const tab = ref<'host' | 'join'>('host')
const hostPort = ref(9527)
const joinPort = ref(9527)
const address = ref('')
const hostSide = ref<Side>('red')
const firstSide = ref<Side>('red')
const seconds = ref(0)
const formId = useId()
const validPort = (value: number) => Number.isInteger(value) && value >= 1 && value <= 65535
const validSeconds = computed(() => Number.isInteger(seconds.value) && seconds.value >= 0 && seconds.value <= 600)
const valid = computed(() => tab.value === 'host'
  ? validPort(hostPort.value) && validSeconds.value
  : validPort(joinPort.value) && address.value.trim().length > 0)
const statusTitle = computed(() => ({
  offline: '房间已就绪', hosting: '等待对手加入', connecting: '正在连接房间',
  connected: '对手已连接', disconnected: '连接已断开',
})[props.network.status])
const roomDescription = computed(() => {
  if (props.network.status === 'connected') {
    if (props.phase === 'ready') return props.network.role === 'host' ? '对手已加入，请回到棋盘点击「开始」开局。' : '已加入房间，等待房主点击「开始」。'
    if (props.phase === 'paused') return '棋局已暂停，等待房主点击「继续」。'
    return '对局进行中，双方按各自的执子颜色行棋。'
  }
  if (props.network.status === 'disconnected') return props.phase === 'ready'
    ? '连接已断开，棋局尚未开始。离开房间后可重新创建或加入。'
    : '连接已断开，棋局已暂停并保留剩余用时。离开房间后可重新创建或加入。'
  return props.network.message || (props.network.status === 'hosting' ? '将下方地址告诉对手，让对方在「加入房间」中输入。' : '正在与房主建立连接，请稍候。')
})
watch(() => props.open, open => {
  if (!open) return
  hostSide.value = props.settings.humanSide
  firstSide.value = props.settings.firstSide
  seconds.value = props.settings.stepSeconds
}, { immediate: true })

function submit() {
  if (props.busy || !valid.value) return
  if (tab.value === 'host') {
    emit('host', hostPort.value, { ...props.settings, mode: 'lan', humanSide: hostSide.value, firstSide: firstSide.value, stepSeconds: seconds.value })
  } else emit('join', address.value.trim(), joinPort.value)
}
</script>

<template>
  <AppDialog :open="open" title="局域网对弈" eyebrow="同一网络，隔空对坐" :dismissible="!busy" @close="emit('close')">
    <div v-if="active" class="room-view">
      <div class="room-emblem" :class="{ connected: network.status === 'connected', disconnected: network.status === 'disconnected' }"><UiIcon name="lan" :size="32" /></div>
      <h3>{{ statusTitle }}</h3>
      <p class="room-description">{{ roomDescription }}</p>
      <div class="room-details">
        <div><span>房间身份</span><strong>{{ network.role === 'host' ? '房主' : network.role === 'guest' ? '访客' : '局域网玩家' }}</strong></div>
        <div><span>我的执子</span><strong :class="network.localSide === 'red' ? 'text-red' : ''">{{ network.localSide === 'red' ? '红方' : network.localSide === 'black' ? '黑方' : '等待分配' }}</strong></div>
        <div v-if="network.address" class="room-address"><span>连接地址</span><code>{{ network.address }}</code></div>
      </div>
      <p class="form-note">{{ network.status === 'hosting' || network.status === 'connecting' ? '连接后等待房主点击「开始」；等待期间不计时。' : '离开房间会断开连接并保留当前棋局。' }}开始、暂停和继续均由房主控制。</p>
      <p v-if="error" class="form-error" role="alert"><UiIcon name="alert" :size="17" />{{ error }}</p>
    </div>
    <form v-else :id="formId" class="lan-form" @submit.prevent="submit">
      <div class="dialog-tabs" role="tablist" aria-label="房间方式">
        <button :id="`${formId}-host-tab`" type="button" role="tab" :aria-selected="tab === 'host'" :aria-controls="`${formId}-host`" :class="{ active: tab === 'host' }" :disabled="busy" @click="tab = 'host'">创建房间</button>
        <button :id="`${formId}-join-tab`" type="button" role="tab" :aria-selected="tab === 'join'" :aria-controls="`${formId}-join`" :class="{ active: tab === 'join' }" :disabled="busy" @click="tab = 'join'">加入房间</button>
      </div>
      <fieldset :disabled="busy" class="lan-fields">
        <div v-if="tab === 'host'" :id="`${formId}-host`" role="tabpanel" :aria-labelledby="`${formId}-host-tab`">
          <p class="dialog-intro">你来摆好棋盘，邀同一局域网内的朋友对弈。</p>
          <label class="stacked-field" :for="`${formId}-host-port`"><span>监听端口</span><input :id="`${formId}-host-port`" v-model.number="hostPort" type="number" min="1" max="65535" step="1" required /><small>对手加入时，填写你的电脑 IP 和这个端口。</small></label>
          <div class="setting-row"><span class="setting-label">房主执子</span><div class="segmented" role="group" aria-label="房主执子"><button type="button" :class="{ active: hostSide === 'red' }" :aria-pressed="hostSide === 'red'" @click="hostSide = 'red'"><i class="side-dot red" />红方</button><button type="button" :class="{ active: hostSide === 'black' }" :aria-pressed="hostSide === 'black'" @click="hostSide = 'black'"><i class="side-dot black" />黑方</button></div></div>
          <div class="setting-row"><span class="setting-label">先行一方</span><div class="segmented" role="group" aria-label="局域网先行一方"><button type="button" :class="{ active: firstSide === 'red' }" :aria-pressed="firstSide === 'red'" @click="firstSide = 'red'">红先</button><button type="button" :class="{ active: firstSide === 'black' }" :aria-pressed="firstSide === 'black'" @click="firstSide = 'black'">黑先</button></div></div>
          <div class="setting-row"><div class="setting-label"><label :for="`${formId}-time`">每步用时</label><small>0 为不限时，最多 600 秒</small></div><div class="number-field"><input :id="`${formId}-time`" v-model.number="seconds" type="number" min="0" max="600" step="1" required /><span>秒 / 步</span></div></div>
          <p v-if="!validPort(hostPort) || !validSeconds" class="field-error">端口须为 1–65535，每步用时须为 0–600 的整数。</p>
        </div>
        <div v-else :id="`${formId}-join`" role="tabpanel" :aria-labelledby="`${formId}-join-tab`">
          <p class="dialog-intro">向房主询问连接地址，输入后即可加入。</p>
          <label class="stacked-field" :for="`${formId}-address`"><span>房主地址</span><input :id="`${formId}-address`" v-model="address" type="text" placeholder="例如 192.168.1.8" autocomplete="off" spellcheck="false" maxlength="253" required /><small>支持 IP 地址或主机名，不需要加 http://。</small></label>
          <label class="stacked-field" :for="`${formId}-join-port`"><span>房间端口</span><input :id="`${formId}-join-port`" v-model.number="joinPort" type="number" min="1" max="65535" step="1" required /></label>
          <p v-if="!validPort(joinPort)" class="field-error">请输入 1 至 65535 的整数端口。</p>
          <p class="notice"><UiIcon name="info" :size="18" />执子颜色、先行一方与用时由房主设定。</p>
        </div>
      </fieldset>
      <p class="form-note">创建或加入房间将准备局域网棋局，双方连接后由房主点击「开始」开局。请确保双方在同一网络。</p>
      <p v-if="error" class="form-error" role="alert"><UiIcon name="alert" :size="17" />{{ error }}</p>
    </form>
    <template #footer>
      <template v-if="active">
        <button class="button button-danger-quiet" type="button" :disabled="busy" @click="emit('leave')"><UiIcon name="exit" :size="17" />{{ busy ? '正在离开' : '离开房间' }}</button>
        <button class="button button-primary" type="button" :disabled="busy" @click="emit('close')">{{ network.status === 'connected' ? '回到棋盘' : '收起窗口' }}</button>
      </template>
      <template v-else>
        <button class="button button-quiet" type="button" :disabled="busy" @click="emit('close')">取消</button>
        <button class="button button-primary" type="submit" :form="formId" :disabled="busy || !valid"><span v-if="busy" class="spinner" /><UiIcon v-else name="lan" :size="18" />{{ busy ? '正在连接' : tab === 'host' ? '创建房间' : '加入房间' }}</button>
      </template>
    </template>
  </AppDialog>
</template>
