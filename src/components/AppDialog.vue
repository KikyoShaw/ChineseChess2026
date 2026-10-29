<script setup lang="ts">
import { nextTick, onUnmounted, ref, useId, watch } from 'vue'
import UiIcon from './UiIcon.vue'

const props = withDefaults(defineProps<{
  open: boolean
  title: string
  eyebrow?: string
  size?: 'small' | 'normal' | 'wide'
  dismissible?: boolean
}>(), { size: 'normal', dismissible: true, eyebrow: '' })
const emit = defineEmits<{ close: [] }>()
const dialog = ref<HTMLElement | null>(null)
const titleId = useId()
let previousFocus: HTMLElement | null = null
let focusVersion = 0
const focusSelector = 'button:not([disabled]), a[href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex="0"]'

function keydown(event: KeyboardEvent) {
  if (!props.open) return
  if (event.key === 'Escape') {
    event.preventDefault()
    event.stopPropagation()
    if (props.dismissible) emit('close')
    return
  }
  if (event.key !== 'Tab' || !dialog.value) return
  const focusable = [...dialog.value.querySelectorAll<HTMLElement>(focusSelector)].filter(element => element.getClientRects().length > 0)
  const first = focusable[0]
  const last = focusable[focusable.length - 1]
  if (!first || !last) { event.preventDefault(); dialog.value.focus(); return }
  if (event.shiftKey && (document.activeElement === first || !dialog.value.contains(document.activeElement))) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && (document.activeElement === last || !dialog.value.contains(document.activeElement))) {
    event.preventDefault()
    first.focus()
  }
}

watch(() => props.open, async open => {
  const version = ++focusVersion
  if (open) {
    previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    document.addEventListener('keydown', keydown)
    await nextTick()
    if (version !== focusVersion || !props.open) return
    const target = dialog.value?.querySelector<HTMLElement>('[data-autofocus]')
      ?? dialog.value?.querySelector<HTMLElement>(focusSelector)
    ;(target ?? dialog.value)?.focus()
  } else {
    document.removeEventListener('keydown', keydown)
    previousFocus?.focus()
    previousFocus = null
  }
}, { immediate: true, flush: 'post' })

onUnmounted(() => {
  focusVersion++
  document.removeEventListener('keydown', keydown)
  previousFocus?.focus()
})
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog">
      <div v-if="open" class="dialog-backdrop" @mousedown.self="dismissible && emit('close')">
        <section ref="dialog" class="dialog-card" :class="`dialog-${size}`" role="dialog" aria-modal="true" :aria-labelledby="titleId" tabindex="-1">
          <header class="dialog-header">
            <div>
              <p v-if="eyebrow" class="eyebrow">{{ eyebrow }}</p>
              <h2 :id="titleId">{{ title }}</h2>
            </div>
            <button class="icon-button dialog-close" type="button" aria-label="关闭对话框" :disabled="!dismissible" @click="emit('close')"><UiIcon name="close" /></button>
          </header>
          <div class="dialog-content"><slot /></div>
          <footer v-if="$slots.footer" class="dialog-footer"><slot name="footer" /></footer>
        </section>
      </div>
    </Transition>
  </Teleport>
</template>
