<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { AlertTriangle, BellRing, MessageSquareShare, Sparkles, X } from 'lucide-vue-next'
import type { DeviceNotifyPayload } from '@/api/types'

const props = withDefaults(
  defineProps<{
    open: boolean
    targetLabel: string
    loading?: boolean
  }>(),
  {
    loading: false,
  },
)

const emit = defineEmits<{
  (e: 'cancel'): void
  (e: 'submit', payload: DeviceNotifyPayload): void
}>()

const titleInputRef = ref<HTMLInputElement | null>(null)
const title = ref('')
const message = ref('')
const urgent = ref(false)
const validationError = ref<string | null>(null)

const presets = [
  {
    label: 'Wartungsankündigung',
    title: 'Geplante IT-Wartung',
    message:
      'Bitte speichern Sie Ihre geöffneten Dokumente. In Kürze werden wichtige System-Updates installiert.',
    urgent: false,
  },
  {
    label: 'Kritischer Neustart',
    title: 'Dringendes Sicherheitsupdate',
    message:
      'Ein kritisches Sicherheitsupdate erfordert zeitnah einen Neustart Ihres Arbeitsplatzes. Bitte sichern Sie Ihre Arbeit.',
    urgent: true,
  },
  {
    label: 'App schließen',
    title: 'Software-Aktualisierung steht bereit',
    message:
      'Bitte schließen Sie geöffnete Büro- oder Browser-Anwendungen kurzzeitig, damit das ausstehende Update abgeschlossen werden kann.',
    urgent: false,
  },
]

function applyPreset(preset: (typeof presets)[number]) {
  title.value = preset.title
  message.value = preset.message
  urgent.value = preset.urgent
  validationError.value = null
}

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
        validationError.value = null
        if (!title.value && !message.value) {
          title.value = ''
          message.value = ''
          urgent.value = false
        }
        window.addEventListener('keydown', onKeyDown)
        await nextTick()
        titleInputRef.value?.focus()
      } else {
        window.removeEventListener('keydown', onKeyDown)
        title.value = ''
        message.value = ''
        urgent.value = false
        validationError.value = null
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

function handleSubmit() {
  const cleanTitle = title.value.trim()
  const cleanMessage = message.value.trim()
  if (!cleanTitle || !cleanMessage) {
    validationError.value = 'Bitte geben Sie sowohl einen Titel als auch einen Nachrichtentext ein.'
    return
  }
  validationError.value = null
  emit('submit', {
    title: cleanTitle,
    message: cleanMessage,
    urgent: urgent.value,
  })
}
</script>

<template>
  <div
    v-if="open"
    data-testid="device-notify-modal"
    role="dialog"
    aria-modal="true"
    aria-labelledby="device-notify-modal-title"
    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/65 p-4 backdrop-blur-sm"
    @click.self="!loading && emit('cancel')"
  >
    <div
      class="w-full max-w-lg overflow-hidden rounded-xl border border-slate-200 bg-white shadow-2xl dark:border-slate-800 dark:bg-slate-900"
    >
      <div
        class="flex items-center justify-between border-b border-slate-800 bg-slate-900 px-6 py-4 text-white dark:bg-slate-950"
      >
        <div class="flex items-center gap-2.5">
          <div class="rounded-lg bg-cyan-500/20 p-1.5 text-cyan-400">
            <MessageSquareShare class="h-4 w-4" aria-hidden="true" />
          </div>
          <div>
            <h3 id="device-notify-modal-title" class="text-sm font-bold sm:text-base">
              Desktop-Nachricht senden
            </h3>
            <p class="text-[11px] text-slate-400">Empfänger: {{ targetLabel }}</p>
          </div>
        </div>
        <button
          type="button"
          aria-label="Dialog schließen"
          :disabled="loading"
          class="rounded-lg p-1 text-slate-400 hover:bg-slate-800 hover:text-white disabled:opacity-50"
          @click="emit('cancel')"
        >
          <X class="h-5 w-5" />
        </button>
      </div>

      <form class="space-y-4 p-6" @submit.prevent="handleSubmit">
        <div>
          <div class="mb-1.5 flex items-center gap-1.5 text-[11px] font-semibold text-slate-500 dark:text-slate-400">
            <Sparkles class="h-3.5 w-3.5 text-sky-500" aria-hidden="true" />
            <span>Schnellvorlagen:</span>
          </div>
          <div class="flex flex-wrap gap-1.5">
            <button
              v-for="preset in presets"
              :key="preset.label"
              type="button"
              class="rounded-full border border-slate-200 bg-slate-50 px-2.5 py-1 text-[11px] font-medium text-slate-700 transition hover:border-sky-300 hover:bg-sky-50 hover:text-sky-700 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300 dark:hover:border-sky-500/40 dark:hover:bg-sky-500/15 dark:hover:text-sky-300"
              @click="applyPreset(preset)"
            >
              {{ preset.label }}
            </button>
          </div>
        </div>

        <div
          v-if="validationError"
          role="alert"
          class="rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-xs text-rose-700 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-300"
        >
          {{ validationError }}
        </div>

        <div>
          <label
            for="notify-title-field"
            class="block text-xs font-semibold text-slate-700 dark:text-slate-300"
          >
            Titel *
          </label>
          <input
            id="notify-title-field"
            ref="titleInputRef"
            v-model="title"
            data-testid="notify-title-input"
            type="text"
            maxlength="120"
            placeholder="z. B. Wichtiger IT-Wartungshinweis"
            class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-100"
          />
        </div>

        <div>
          <label
            for="notify-message-field"
            class="block text-xs font-semibold text-slate-700 dark:text-slate-300"
          >
            Nachricht *
          </label>
          <textarea
            id="notify-message-field"
            v-model="message"
            data-testid="notify-message-input"
            rows="3"
            maxlength="1000"
            placeholder="Mitteilung wird als signiertes OS-Toast-Popup und Banner im NS8 Software Center angezeigt…"
            class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-100"
          />
        </div>

        <label
          class="flex cursor-pointer items-start gap-3 rounded-lg border p-3 transition"
          :class="
            urgent
              ? 'border-amber-300 bg-amber-50/80 dark:border-amber-500/40 dark:bg-amber-500/10'
              : 'border-slate-200 bg-slate-50/70 dark:border-slate-800 dark:bg-slate-950/60'
          "
        >
          <input
            v-model="urgent"
            data-testid="notify-urgent-checkbox"
            type="checkbox"
            class="mt-0.5 h-4 w-4 rounded border-slate-300 text-amber-600 focus:ring-amber-500 dark:border-slate-600 dark:bg-slate-950"
          />
          <span class="min-w-0">
            <span class="flex items-center gap-1.5 text-xs font-semibold text-slate-900 dark:text-slate-100">
              <AlertTriangle
                v-if="urgent"
                class="h-3.5 w-3.5 text-amber-600 dark:text-amber-400"
                aria-hidden="true"
              />
              <BellRing v-else class="h-3.5 w-3.5 text-slate-500 dark:text-slate-400" aria-hidden="true" />
              <span>Dringend / Wichtig</span>
            </span>
            <span class="mt-0.5 block text-[11px] text-slate-500 dark:text-slate-400">
              Durchbricht den stillen Modus und hebt die Mitteilung im NS8 Software Center als Prioritätshinweis hervor
            </span>
          </span>
        </label>

        <div class="flex items-center justify-end gap-2.5 border-t border-slate-200 pt-4 dark:border-slate-800">
          <button
            type="button"
            data-testid="notify-cancel-btn"
            :disabled="loading"
            class="rounded-lg border border-slate-300 bg-white px-4 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
            @click="emit('cancel')"
          >
            Abbrechen
          </button>
          <button
            type="submit"
            data-testid="notify-submit-btn"
            :disabled="loading"
            class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white shadow-sm hover:bg-sky-700 disabled:opacity-60"
          >
            <MessageSquareShare class="h-3.5 w-3.5" aria-hidden="true" />
            {{ loading ? 'Wird gesendet…' : 'Nachricht jetzt senden' }}
          </button>
        </div>
      </form>
    </div>
  </div>
</template>
