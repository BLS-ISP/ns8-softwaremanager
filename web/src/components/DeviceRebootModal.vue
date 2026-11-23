<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import { AlertTriangle, Ban, Clock, Power, RotateCcw, X } from 'lucide-vue-next'
import type { DeviceRebootMode, DeviceRebootPayload } from '@/api/types'

const props = withDefaults(
  defineProps<{
    open: boolean
    hostname: string
    rebootRequired?: boolean
    loading?: boolean
  }>(),
  {
    rebootRequired: false,
    loading: false,
  },
)

const emit = defineEmits<{
  (e: 'cancel'): void
  (e: 'submit', payload: DeviceRebootPayload): void
}>()

const mode = ref<DeviceRebootMode>('grace_period')
const graceMinutes = ref<number>(60)

const graceOptions = [
  { minutes: 15, label: '15 Min.' },
  { minutes: 60, label: '60 Min. (1 Std.)' },
  { minutes: 240, label: '240 Min. (4 Std.)' },
]

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.open && !props.loading) {
    e.preventDefault()
    emit('cancel')
  }
}

watch(
  () => props.open,
  (isOpen) => {
    if (typeof window !== 'undefined') {
      if (isOpen) {
        mode.value = 'grace_period'
        graceMinutes.value = 60
        window.addEventListener('keydown', onKeyDown)
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

function handleSubmit() {
  if (mode.value === 'grace_period') {
    emit('submit', {
      mode: 'grace_period',
      grace_minutes: graceMinutes.value,
    })
  } else {
    emit('submit', {
      mode: mode.value,
    })
  }
}
</script>

<template>
  <div
    v-if="open"
    data-testid="device-reboot-modal"
    role="dialog"
    aria-modal="true"
    aria-labelledby="device-reboot-modal-title"
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
          <div class="rounded-lg bg-amber-500/20 p-1.5 text-amber-400">
            <RotateCcw class="h-4 w-4" aria-hidden="true" />
          </div>
          <div>
            <h3 id="device-reboot-modal-title" class="text-sm font-bold sm:text-base">
              Neustart steuern
            </h3>
            <p class="text-[11px] text-slate-400">
              Zielsystem: <span class="font-mono text-slate-200">{{ hostname }}</span>
              <span v-if="rebootRequired" class="ml-1.5 text-amber-300">· Neustart ausstehend</span>
            </p>
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
        <div class="space-y-2.5">
          <!-- Mode 1: Grace Period Countdown -->
          <div
            class="rounded-xl border p-3.5 transition"
            :class="
              mode === 'grace_period'
                ? 'border-sky-400 bg-sky-50/70 dark:border-sky-500/50 dark:bg-sky-500/10'
                : 'border-slate-200 bg-white hover:border-slate-300 dark:border-slate-800 dark:bg-slate-950/50 dark:hover:border-slate-700'
            "
          >
            <label class="flex cursor-pointer items-start gap-3">
              <input
                v-model="mode"
                data-testid="reboot-mode-grace_period"
                type="radio"
                name="reboot-mode"
                value="grace_period"
                class="mt-0.5 h-4 w-4 border-slate-300 text-sky-600 focus:ring-sky-500"
              />
              <span class="min-w-0 flex-1">
                <span class="flex items-center gap-1.5 text-xs font-bold text-slate-900 dark:text-slate-100">
                  <Clock class="h-3.5 w-3.5 text-sky-600 dark:text-sky-400" aria-hidden="true" />
                  <span>Karenzzeit-Countdown starten</span>
                </span>
                <span class="mt-0.5 block text-[11px] text-slate-500 dark:text-slate-400">
                  Zeigt angemeldeten Benutzern einen Countdown-Dialog mit Speicherhinweis bis zur gewählten Deadline
                </span>
              </span>
            </label>

            <div v-if="mode === 'grace_period'" class="mt-3 flex flex-wrap items-center gap-2 pl-7">
              <span class="text-[11px] font-semibold text-slate-600 dark:text-slate-300">Karenzzeit:</span>
              <button
                v-for="opt in graceOptions"
                :key="opt.minutes"
                type="button"
                :data-testid="`reboot-grace-${opt.minutes}`"
                class="rounded-lg border px-3 py-1 text-xs font-semibold transition"
                :class="
                  graceMinutes === opt.minutes
                    ? 'border-sky-600 bg-sky-600 text-white shadow-xs'
                    : 'border-slate-300 bg-white text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800'
                "
                @click="graceMinutes = opt.minutes"
              >
                {{ opt.label }}
              </button>
            </div>
          </div>

          <!-- Mode 2: Cancel Pending Reboot -->
          <label
            class="flex cursor-pointer items-start gap-3 rounded-xl border p-3.5 transition"
            :class="
              mode === 'cancel'
                ? 'border-emerald-400 bg-emerald-50/70 dark:border-emerald-500/50 dark:bg-emerald-500/10'
                : 'border-slate-200 bg-white hover:border-slate-300 dark:border-slate-800 dark:bg-slate-950/50 dark:hover:border-slate-700'
            "
          >
            <input
              v-model="mode"
              data-testid="reboot-mode-cancel"
              type="radio"
              name="reboot-mode"
              value="cancel"
              class="mt-0.5 h-4 w-4 border-slate-300 text-emerald-600 focus:ring-emerald-500"
            />
            <span class="min-w-0 flex-1">
              <span class="flex items-center gap-1.5 text-xs font-bold text-slate-900 dark:text-slate-100">
                <Ban class="h-3.5 w-3.5 text-emerald-600 dark:text-emerald-400" aria-hidden="true" />
                <span>Ausstehenden Neustart abbrechen</span>
              </span>
              <span class="mt-0.5 block text-[11px] text-slate-500 dark:text-slate-400">
                Hebt eine geplante Agent-Neustart-Deadline auf und blendet aktive Countdown-Warnungen aus
              </span>
            </span>
          </label>

          <!-- Mode 3: Immediate Reboot -->
          <label
            class="flex cursor-pointer items-start gap-3 rounded-xl border p-3.5 transition"
            :class="
              mode === 'immediate'
                ? 'border-rose-400 bg-rose-50/70 dark:border-rose-500/50 dark:bg-rose-500/10'
                : 'border-slate-200 bg-white hover:border-slate-300 dark:border-slate-800 dark:bg-slate-950/50 dark:hover:border-slate-700'
            "
          >
            <input
              v-model="mode"
              data-testid="reboot-mode-immediate"
              type="radio"
              name="reboot-mode"
              value="immediate"
              class="mt-0.5 h-4 w-4 border-slate-300 text-rose-600 focus:ring-rose-500"
            />
            <span class="min-w-0 flex-1">
              <span class="flex items-center gap-1.5 text-xs font-bold text-slate-900 dark:text-slate-100">
                <Power class="h-3.5 w-3.5 text-rose-600 dark:text-rose-400" aria-hidden="true" />
                <span>Sofort neu starten</span>
              </span>
              <span class="mt-0.5 block text-[11px] text-slate-500 dark:text-slate-400">
                Führt ohne Wartezeit unmittelbar einen erzwungenen System-Neustart aus
              </span>
            </span>
          </label>
        </div>

        <div
          v-if="mode === 'immediate'"
          data-testid="reboot-immediate-warning"
          role="alert"
          class="flex items-start gap-2.5 rounded-xl border border-rose-300 bg-rose-50 p-3.5 text-xs text-rose-800 dark:border-rose-500/40 dark:bg-rose-500/15 dark:text-rose-200"
        >
          <AlertTriangle class="mt-0.5 h-4 w-4 shrink-0 text-rose-600 dark:text-rose-400" aria-hidden="true" />
          <div>
            <div class="font-bold">Sicherheitswarnung: Sofortiger Neustart ohne Karenzzeit</div>
            <p class="mt-0.5 leading-relaxed">
              Angemeldete Benutzer auf <strong class="font-mono">{{ hostname }}</strong> können geöffnete Dateien
              nicht mehr speichern. Laufende Sitzungen und Dienste werden sofort beendet.
            </p>
          </div>
        </div>

        <div class="flex items-center justify-end gap-2.5 border-t border-slate-200 pt-4 dark:border-slate-800">
          <button
            type="button"
            data-testid="reboot-cancel-btn"
            :disabled="loading"
            class="rounded-lg border border-slate-300 bg-white px-4 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
            @click="emit('cancel')"
          >
            Abbrechen
          </button>
          <button
            type="submit"
            data-testid="reboot-submit-btn"
            :disabled="loading"
            class="inline-flex items-center gap-1.5 rounded-lg px-4 py-2 text-xs font-semibold text-white shadow-sm transition disabled:opacity-60"
            :class="
              mode === 'immediate'
                ? 'bg-rose-600 hover:bg-rose-700'
                : mode === 'cancel'
                  ? 'bg-emerald-600 hover:bg-emerald-700'
                  : 'bg-sky-600 hover:bg-sky-700'
            "
          >
            <Power v-if="mode === 'immediate'" class="h-3.5 w-3.5" aria-hidden="true" />
            <Ban v-else-if="mode === 'cancel'" class="h-3.5 w-3.5" aria-hidden="true" />
            <RotateCcw v-else class="h-3.5 w-3.5" aria-hidden="true" />
            {{
              loading
                ? 'Wird gesendet…'
                : mode === 'immediate'
                  ? 'Jetzt sofort neu starten'
                  : mode === 'cancel'
                    ? 'Neustart abbrechen'
                    : `Countdown (${graceMinutes} Min.) starten`
            }}
          </button>
        </div>
      </form>
    </div>
  </div>
</template>
