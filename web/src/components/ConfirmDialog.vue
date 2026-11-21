<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { AlertTriangle, Trash2, X } from 'lucide-vue-next'

const props = withDefaults(
  defineProps<{
    open: boolean
    title: string
    message: string
    confirmLabel?: string
    cancelLabel?: string
    variant?: 'danger' | 'warning'
    loading?: boolean
  }>(),
  {
    confirmLabel: 'Bestätigen',
    cancelLabel: 'Abbrechen',
    variant: 'danger',
    loading: false,
  },
)

const emit = defineEmits<{
  (e: 'confirm'): void
  (e: 'cancel'): void
}>()

const cancelBtnRef = ref<HTMLButtonElement | null>(null)

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.open && !props.loading) {
    e.preventDefault()
    emit('cancel')
  }
}

watch(
  () => props.open,
  async (isOpen) => {
    if (typeof window !== 'undefined') {
      if (isOpen) {
        window.addEventListener('keydown', onKeyDown)
        await nextTick()
        cancelBtnRef.value?.focus()
      } else {
        window.removeEventListener('keydown', onKeyDown)
      }
    }
  },
  { immediate: true },
)

onBeforeUnmount(() => {
  if (typeof window !== 'undefined') {
    window.removeEventListener('keydown', onKeyDown)
  }
})
</script>

<template>
  <div
    v-if="open"
    data-testid="confirm-dialog"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="confirm-dialog-title"
    aria-describedby="confirm-dialog-desc"
    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/65 p-4 backdrop-blur-sm"
    @click.self="!loading && emit('cancel')"
  >
    <div
      class="w-full max-w-md overflow-hidden rounded-xl border border-slate-200 bg-white shadow-2xl dark:border-slate-800 dark:bg-slate-900"
    >
      <div class="flex items-start justify-between gap-3 p-6 pb-4">
        <div class="flex items-start gap-3.5">
          <div
            class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl"
            :class="
              variant === 'warning'
                ? 'bg-amber-50 text-amber-600 ring-1 ring-amber-500/20 dark:bg-amber-500/15 dark:text-amber-400'
                : 'bg-rose-50 text-rose-600 ring-1 ring-rose-500/20 dark:bg-rose-500/15 dark:text-rose-400'
            "
          >
            <AlertTriangle v-if="variant === 'warning'" class="h-5 w-5" aria-hidden="true" />
            <Trash2 v-else class="h-5 w-5" aria-hidden="true" />
          </div>
          <div>
            <h3
              id="confirm-dialog-title"
              class="text-base font-bold text-slate-900 dark:text-slate-50"
            >
              {{ title }}
            </h3>
            <p
              id="confirm-dialog-desc"
              class="mt-1.5 text-xs leading-relaxed text-slate-600 dark:text-slate-300"
            >
              {{ message }}
            </p>
          </div>
        </div>
        <button
          type="button"
          aria-label="Dialog schließen"
          :disabled="loading"
          class="rounded-lg p-1 text-slate-400 transition hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-slate-800 dark:hover:text-slate-200"
          @click="emit('cancel')"
        >
          <X class="h-4 w-4" />
        </button>
      </div>

      <div
        class="flex items-center justify-end gap-2.5 border-t border-slate-100 bg-slate-50/70 px-6 py-3.5 dark:border-slate-800 dark:bg-slate-900/60"
      >
        <button
          ref="cancelBtnRef"
          type="button"
          data-testid="confirm-dialog-cancel"
          :disabled="loading"
          class="rounded-lg border border-slate-300 bg-white px-3.5 py-2 text-xs font-semibold text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
          @click="emit('cancel')"
        >
          {{ cancelLabel }}
        </button>
        <button
          type="button"
          data-testid="confirm-dialog-confirm"
          :disabled="loading"
          class="rounded-lg px-4 py-2 text-xs font-semibold text-white shadow-sm transition disabled:opacity-60"
          :class="
            variant === 'warning'
              ? 'bg-amber-600 hover:bg-amber-700 dark:bg-amber-500 dark:hover:bg-amber-600'
              : 'bg-rose-600 hover:bg-rose-700 dark:bg-rose-600 dark:hover:bg-rose-500'
          "
          @click="emit('confirm')"
        >
          {{ loading ? 'Wird ausgeführt…' : confirmLabel }}
        </button>
      </div>
    </div>
  </div>
</template>
