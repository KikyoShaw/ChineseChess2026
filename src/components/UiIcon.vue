<script setup lang="ts">
withDefaults(defineProps<{ name: string; size?: number }>(), { size: 20 })
const paths: Record<string, string> = {
  plus: 'M12 5v14M5 12h14',
  undo: 'M9 5 4 10l5 5M4 10h9a6 6 0 0 1 0 12',
  flip: 'M7 4 3 8l4 4M3 8h12a5 5 0 0 1 5 5M17 20l4-4-4-4M21 16H9a5 5 0 0 1-5-5',
  folder: 'M3 8V5a1 1 0 0 1 1-1h5l2 3h9a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V8Z',
  save: 'M4 3h13l4 4v13a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1ZM7 3v6h10V3M7 21v-8h10v8',
  sound: 'M11 4 6 8H3v8h3l5 4V4ZM15 8a6 6 0 0 1 0 8M18 4a11 11 0 0 1 0 16',
  mute: 'M11 4 6 8H3v8h3l5 4V4ZM16 9l6 6M22 9l-6 6',
  settings: 'M9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1 1-3ZM15.5 12a3.5 3.5 0 1 1-7 0 3.5 3.5 0 0 1 7 0Z',
  lan: 'M8 3h8v6H8V3ZM3 16h6v5H3v-5ZM15 16h6v5h-6v-5ZM12 9v4M6 16v-3h12v3',
  close: 'M6 6l12 12M18 6 6 18',
  info: 'M12 8h.01M12 11v6M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0Z',
  check: 'm5 12 4 4L19 6',
  arrow: 'M5 12h14m-6-6 6 6-6 6',
  flag: 'M5 21V3m0 1c5-5 9 5 14 0v10c-5 5-9-5-14 0',
  draw: 'M4 5v14M20 5v14M4 8h16M4 16h16M9 8v8M15 8v8',
  clock: 'M12 7v5l3 2M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0Z',
  person: 'M16 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0ZM4 21v-2a8 8 0 0 1 16 0v2',
  computer: 'M4 4h16v12H4V4ZM8 21h8M12 16v5M8 8h8M8 12h4',
  retry: 'M20 7V3m0 4h-4M20 7a9 9 0 1 0 1 8',
  chevron: 'm9 5 7 7-7 7',
  exit: 'M9 4H4v16h5M14 7l5 5-5 5M8 12h12',
  alert: 'M12 3 2 21h20L12 3ZM12 9v5M12 17h.01',
  book: 'M12 5C8 2 4 3 2 4v15c3-2 6-2 10 0 4-2 7-2 10 0V4c-2-1-6-2-10 1ZM12 5v14',
}
</script>

<template>
  <svg :width="size" :height="size" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">
    <path :d="paths[name] ?? paths.info" />
  </svg>
</template>
