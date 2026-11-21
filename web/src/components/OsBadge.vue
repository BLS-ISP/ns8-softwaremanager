<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  osFamily: 'windows' | 'linux' | string
  osName?: string
  osVersion?: string
}>()

const isWindows = computed(() => (props.osFamily || '').toLowerCase() === 'windows')
const displayText = computed(() => {
  if (props.osName) {
    return props.osVersion ? `${props.osName} (${props.osVersion})` : props.osName
  }
  return isWindows.value ? 'Windows' : 'Linux'
})
</script>

<template>
  <span
    data-testid="os-badge"
    :data-os-family="isWindows ? 'windows' : 'linux'"
    class="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium"
    :class="
      isWindows
        ? 'bg-sky-50 text-sky-800 ring-1 ring-inset ring-sky-600/20 dark:bg-sky-500/15 dark:text-sky-300 dark:ring-sky-400/30'
        : 'bg-orange-50 text-orange-800 ring-1 ring-inset ring-orange-600/20 dark:bg-amber-500/15 dark:text-amber-300 dark:ring-amber-400/30'
    "
  >
    <!-- Crisp Windows 4-pane glyph -->
    <svg
      v-if="isWindows"
      class="h-3.5 w-3.5 shrink-0 text-sky-600 dark:text-sky-400"
      viewBox="0 0 24 24"
      fill="currentColor"
      aria-hidden="true"
    >
      <path d="M2.5 3h8.6v8.6H2.5V3zm10.4 0h8.6v8.6h-8.6V3zM2.5 13.4h8.6V22H2.5v-8.6zm10.4 0h8.6V22h-8.6v-8.6z" />
    </svg>
    <!-- Crisp Linux Tux Penguin emblem -->
    <svg
      v-else
      class="h-3.5 w-3.5 shrink-0 text-orange-600 dark:text-amber-400"
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      <!-- Penguin body & head -->
      <path
        d="M12 2.5C9.5 2.5 8 4.4 8 6.9c0 1.4.4 2.4-.7 3.9-1.3 1.8-2.3 3.7-2.3 5.7 0 2.7 1.8 4.5 4.3 4.5h5.4c2.5 0 4.3-1.8 4.3-4.5 0-2-1-3.9-2.3-5.7-1.1-1.5-.7-2.5-.7-3.9 0-2.5-1.5-4.4-4-4.4Z"
        fill="currentColor"
        fill-opacity="0.18"
        stroke="currentColor"
        stroke-width="1.75"
        stroke-linejoin="round"
      />
      <!-- Belly arch -->
      <path
        d="M9.3 14.2c0-2 1.2-3.5 2.7-3.5s2.7 1.5 2.7 3.5c0 2.1-1 3.8-2.7 3.8s-2.7-1.7-2.7-3.8Z"
        stroke="currentColor"
        stroke-width="1.4"
      />
      <!-- Beak & eyes -->
      <circle cx="10.6" cy="6.6" r="0.85" fill="currentColor" />
      <circle cx="13.4" cy="6.6" r="0.85" fill="currentColor" />
      <path
        d="M10.5 8.5L12 9.6l1.5-1.1"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
    <span class="truncate">{{ displayText }}</span>
  </span>
</template>
