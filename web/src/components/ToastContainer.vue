<script setup lang="ts">
import { AlertCircle, AlertTriangle, CheckCircle2, Info, X } from 'lucide-vue-next'
import { useToast } from '@/composables/useToast'

const { toasts, dismiss } = useToast()
</script>

<template>
  <div
    data-testid="toast-container"
    role="region"
    aria-label="Systembenachrichtigungen"
    aria-live="polite"
    class="pointer-events-none fixed bottom-4 right-4 z-[60] flex w-full max-w-sm flex-col gap-2.5 px-4 sm:px-0"
  >
    <TransitionGroup
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="translate-y-2 opacity-0"
      enter-to-class="translate-y-0 opacity-100"
      leave-active-class="transition duration-150 ease-in"
      leave-from-class="translate-y-0 opacity-100"
      leave-to-class="translate-y-1 opacity-0"
    >
      <div
        v-for="t in toasts"
        :key="t.id"
        data-testid="toast-item"
        :data-toast-type="t.type"
        role="status"
        class="pointer-events-auto flex items-start gap-3 rounded-xl border px-4 py-3 text-xs shadow-lg backdrop-blur-sm"
        :class="
          t.type === 'success'
            ? 'border-emerald-200 bg-white/95 text-slate-900 dark:border-emerald-500/30 dark:bg-slate-900/95 dark:text-slate-100'
            : t.type === 'error'
              ? 'border-rose-200 bg-white/95 text-slate-900 dark:border-rose-500/30 dark:bg-slate-900/95 dark:text-slate-100'
              : t.type === 'warning'
                ? 'border-amber-200 bg-white/95 text-slate-900 dark:border-amber-500/30 dark:bg-slate-900/95 dark:text-slate-100'
                : 'border-sky-200 bg-white/95 text-slate-900 dark:border-sky-500/30 dark:bg-slate-900/95 dark:text-slate-100'
        "
      >
        <CheckCircle2
          v-if="t.type === 'success'"
          class="mt-0.5 h-4 w-4 shrink-0 text-emerald-600 dark:text-emerald-400"
          aria-hidden="true"
        />
        <AlertCircle
          v-else-if="t.type === 'error'"
          class="mt-0.5 h-4 w-4 shrink-0 text-rose-600 dark:text-rose-400"
          aria-hidden="true"
        />
        <AlertTriangle
          v-else-if="t.type === 'warning'"
          class="mt-0.5 h-4 w-4 shrink-0 text-amber-600 dark:text-amber-400"
          aria-hidden="true"
        />
        <Info
          v-else
          class="mt-0.5 h-4 w-4 shrink-0 text-sky-600 dark:text-sky-400"
          aria-hidden="true"
        />

        <div class="min-w-0 flex-1">
          <div class="font-semibold leading-snug">{{ t.title }}</div>
          <div v-if="t.message" class="mt-0.5 leading-relaxed text-slate-600 dark:text-slate-300">
            {{ t.message }}
          </div>
        </div>

        <button
          type="button"
          data-testid="toast-dismiss-btn"
          aria-label="Benachrichtigung schließen"
          class="rounded p-0.5 text-slate-400 transition hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-slate-800 dark:hover:text-slate-200"
          @click="dismiss(t.id)"
        >
          <X class="h-3.5 w-3.5" />
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>
