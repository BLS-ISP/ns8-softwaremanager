<script setup lang="ts">
import type { Component } from 'vue'

defineProps<{
  icon?: Component
  title: string
  description: string
}>()
</script>

<template>
  <div
    role="status"
    class="flex flex-col items-center justify-center rounded-xl border border-dashed border-slate-300 bg-white px-6 py-12 text-center dark:border-slate-800 dark:bg-slate-900/70"
  >
    <div
      v-if="icon"
      class="mb-3.5 flex h-12 w-12 items-center justify-center rounded-xl bg-slate-100 text-slate-600 ring-1 ring-slate-200/80 dark:bg-slate-800 dark:text-cyan-400 dark:ring-slate-700"
    >
      <component :is="icon" class="h-6 w-6" aria-hidden="true" />
    </div>
    <h3 class="text-sm font-bold text-slate-900 dark:text-slate-100">
      {{ title }}
    </h3>
    <p class="mt-1 max-w-md text-xs leading-relaxed text-slate-500 dark:text-slate-400">
      {{ description }}
    </p>
    <div v-if="$slots.actions" class="mt-5 flex flex-wrap items-center justify-center gap-2.5">
      <slot name="actions" />
    </div>
  </div>
</template>
