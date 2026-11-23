<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  osFamily: 'windows' | 'linux' | 'macos' | string
  osName?: string
  osVersion?: string
}>()

const normalizedFamily = computed<'windows' | 'macos' | 'linux'>(() => {
  const raw = (props.osFamily || '').toLowerCase().trim()
  if (raw === 'windows') return 'windows'
  if (raw === 'macos' || raw === 'darwin' || raw === 'mac' || raw === 'osx' || raw === 'mac os x') {
    return 'macos'
  }
  return 'linux'
})

const displayText = computed(() => {
  if (props.osName) {
    return props.osVersion ? `${props.osName} (${props.osVersion})` : props.osName
  }
  if (normalizedFamily.value === 'windows') return 'Windows'
  if (normalizedFamily.value === 'macos') return 'macOS'
  return 'Linux'
})

const badgeClasses = computed(() => {
  if (normalizedFamily.value === 'windows') {
    return 'bg-sky-50 text-sky-800 ring-1 ring-inset ring-sky-600/20 dark:bg-sky-500/15 dark:text-sky-300 dark:ring-sky-400/30'
  }
  if (normalizedFamily.value === 'macos') {
    return 'bg-slate-100 text-slate-800 ring-1 ring-inset ring-slate-600/25 dark:bg-zinc-500/15 dark:text-zinc-200 dark:ring-zinc-400/30'
  }
  return 'bg-orange-50 text-orange-800 ring-1 ring-inset ring-orange-600/20 dark:bg-amber-500/15 dark:text-amber-300 dark:ring-amber-400/30'
})
</script>

<template>
  <span
    data-testid="os-badge"
    :data-os-family="normalizedFamily"
    class="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium"
    :class="badgeClasses"
  >
    <!-- Crisp Windows 4-pane glyph -->
    <svg
      v-if="normalizedFamily === 'windows'"
      class="h-3.5 w-3.5 shrink-0 text-sky-600 dark:text-sky-400"
      viewBox="0 0 24 24"
      fill="currentColor"
      aria-hidden="true"
    >
      <path d="M2.5 3h8.6v8.6H2.5V3zm10.4 0h8.6v8.6h-8.6V3zM2.5 13.4h8.6V22H2.5v-8.6zm10.4 0h8.6V22h-8.6v-8.6z" />
    </svg>
    <!-- Crisp Apple / macOS emblem -->
    <svg
      v-else-if="normalizedFamily === 'macos'"
      class="h-3.5 w-3.5 shrink-0 text-slate-700 dark:text-zinc-300"
      viewBox="0 0 24 24"
      fill="currentColor"
      aria-hidden="true"
    >
      <path
        d="M16.365 1.43c0 1.14-.493 2.27-1.177 3.08-.744.9-1.99 1.57-2.987 1.57-.12 0-.23-.02-.3-.03-.01-.06-.04-.22-.04-.39 0-1.15.572-2.27 1.206-2.98.804-.94 2.142-1.64 3.248-1.68.03.13.05.28.05.43zm4.565 15.71c-.03.07-.463 1.58-1.518 3.12-.945 1.34-1.94 2.71-3.43 2.71-1.517 0-1.9-.88-3.63-.88-1.698 0-2.302.91-3.67.91-1.377 0-2.332-1.26-3.428-2.8-1.287-1.82-2.323-4.63-2.323-7.28 0-4.28 2.797-6.55 5.552-6.55 1.448 0 2.675.95 3.6.95.865 0 2.222-1.01 3.902-1.01.613 0 2.886.06 4.374 2.19-.13.09-2.383 1.37-2.383 4.19 0 3.26 2.854 4.42 2.955 4.45z"
      />
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
