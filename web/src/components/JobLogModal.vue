<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import { AlertTriangle, Check, Copy, RefreshCw, Terminal, X } from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type { JobItem } from '@/api/types'
import StatusBadge from './StatusBadge.vue'
import { formatDateTime } from '@/utils/format'

const props = defineProps<{
  open: boolean
  jobId: string | null
  initialJob?: JobItem | null
}>()

const emit = defineEmits<{
  (e: 'close'): void
}>()

const job = ref<JobItem | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)
const copiedStream = ref<'stdout' | 'stderr' | null>(null)

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.open) {
    emit('close')
  }
}

async function loadJobDetail() {
  if (!props.jobId) return
  loading.value = true
  error.value = null
  try {
    job.value = await adminApi.getJob(props.jobId)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Fehler beim Laden des Auftragsprotokolls'
    if (props.initialJob) {
      job.value = props.initialJob
    }
  } finally {
    loading.value = false
  }
}

watch(
  () => [props.open, props.jobId],
  ([isOpen]) => {
    if (typeof window !== 'undefined') {
      if (isOpen) {
        window.addEventListener('keydown', onKeyDown)
      } else {
        window.removeEventListener('keydown', onKeyDown)
      }
    }
    if (isOpen && props.jobId) {
      job.value = props.initialJob || null
      loadJobDetail()
    }
  },
  { immediate: true },
)

onBeforeUnmount(() => {
  if (typeof window !== 'undefined') {
    window.removeEventListener('keydown', onKeyDown)
  }
})

async function copyOutput(stream: 'stdout' | 'stderr') {
  if (!job.value) return
  const text = (stream === 'stdout' ? job.value.stdout : job.value.stderr) || ''
  if (!text) return
  try {
    await navigator.clipboard.writeText(text)
    copiedStream.value = stream
    setTimeout(() => {
      copiedStream.value = null
    }, 2000)
  } catch {
    // ignore clipboard restrictions in headless tests
  }
}
</script>

<template>
  <div
    v-if="open"
    role="dialog"
    aria-modal="true"
    aria-label="Auftrags- und Ausführungsprotokoll"
    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/65 p-4 backdrop-blur-sm"
    @click.self="emit('close')"
  >
    <div
      class="flex max-h-[90vh] w-full max-w-4xl flex-col overflow-hidden rounded-xl border border-slate-200 bg-white shadow-2xl dark:border-slate-800 dark:bg-slate-900"
    >
      <!-- Header -->
      <div class="flex items-center justify-between border-b border-slate-800 bg-slate-900 px-6 py-4 text-white dark:bg-slate-950">
        <div class="flex items-center gap-3">
          <div class="rounded-lg bg-slate-800 p-2 text-cyan-400">
            <Terminal class="h-5 w-5" aria-hidden="true" />
          </div>
          <div>
            <h3 class="text-base font-semibold">Auftrags- &amp; Ausführungsprotokoll</h3>
            <p class="font-mono text-xs text-slate-400">Job-ID: {{ job?.id || jobId }}</p>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <button
            type="button"
            aria-label="Auftragsprotokoll aktualisieren"
            class="inline-flex items-center gap-1.5 rounded-lg bg-slate-800 px-3 py-1.5 text-xs font-medium text-slate-200 hover:bg-slate-700"
            :disabled="loading"
            @click="loadJobDetail"
          >
            <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': loading }" />
            Aktualisieren
          </button>
          <button
            type="button"
            aria-label="Dialog schließen"
            class="rounded-lg p-1.5 text-slate-400 hover:bg-slate-800 hover:text-white"
            @click="emit('close')"
          >
            <X class="h-5 w-5" />
          </button>
        </div>
      </div>

      <!-- Body -->
      <div class="flex-1 space-y-5 overflow-y-auto p-6">
        <div
          v-if="error"
          class="rounded-lg border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-300"
        >
          {{ error }}
        </div>

        <template v-if="job">
          <!-- Metadata Grid -->
          <div class="grid grid-cols-2 gap-4 rounded-xl border border-slate-200 bg-slate-50 p-4 sm:grid-cols-4 dark:border-slate-800 dark:bg-slate-950/60">
            <div>
              <div class="text-xs font-medium text-slate-500 dark:text-slate-400">Status</div>
              <div class="mt-1">
                <StatusBadge variant="job" :value="job.status" />
              </div>
            </div>
            <div>
              <div class="text-xs font-medium text-slate-500 dark:text-slate-400">Gerät</div>
              <div class="mt-1 text-sm font-semibold text-slate-900 dark:text-slate-100">
                {{ job.device_hostname || job.hostname || job.device_id }}
              </div>
            </div>
            <div>
              <div class="text-xs font-medium text-slate-500 dark:text-slate-400">Typ &amp; Aktion</div>
              <div class="mt-1 text-sm font-medium text-slate-800 dark:text-slate-200">
                {{ job.job_type }} / <span class="font-mono text-xs">{{ job.action }}</span>
              </div>
            </div>
            <div>
              <div class="text-xs font-medium text-slate-500 dark:text-slate-400">Exit-Code</div>
              <div class="mt-1 flex flex-wrap items-center gap-2">
                <span
                  v-if="job.exit_code !== undefined && job.exit_code !== null"
                  class="inline-flex items-center rounded px-2 py-0.5 font-mono text-xs font-semibold"
                  :class="
                    job.exit_code === 0
                      ? 'bg-emerald-100 text-emerald-800 dark:bg-emerald-500/20 dark:text-emerald-300'
                      : job.exit_code === 3010 || job.exit_code === 1641
                        ? 'bg-amber-100 text-amber-800 dark:bg-amber-500/20 dark:text-amber-300'
                        : 'bg-rose-100 text-rose-800 dark:bg-rose-500/20 dark:text-rose-300'
                  "
                >
                  {{ job.exit_code }}
                </span>
                <span v-else class="text-sm text-slate-400">—</span>
                <StatusBadge v-if="job.reboot_required" variant="reboot" />
              </div>
            </div>
            <div>
              <div class="text-xs font-medium text-slate-500 dark:text-slate-400">Ziel / Paket</div>
              <div class="mt-1 truncate font-mono text-xs text-slate-700 dark:text-slate-300" :title="job.target_id">
                {{ job.target_name || job.target_id || '—' }}
                <span v-if="job.version" class="text-slate-500 dark:text-slate-400">({{ job.version }})</span>
              </div>
            </div>
            <div>
              <div class="text-xs font-medium text-slate-500 dark:text-slate-400">Erstellt</div>
              <div class="mt-1 text-xs text-slate-700 dark:text-slate-300">{{ formatDateTime(job.created_at) }}</div>
            </div>
            <div>
              <div class="text-xs font-medium text-slate-500 dark:text-slate-400">Gestartet</div>
              <div class="mt-1 text-xs text-slate-700 dark:text-slate-300">{{ formatDateTime(job.started_at) }}</div>
            </div>
            <div>
              <div class="text-xs font-medium text-slate-500 dark:text-slate-400">Abgeschlossen</div>
              <div class="mt-1 text-xs text-slate-700 dark:text-slate-300">
                {{ formatDateTime(job.completed_at || job.finished_at) }}
              </div>
            </div>
          </div>

          <!-- STDOUT Terminal -->
          <div>
            <div class="mb-1.5 flex items-center justify-between">
              <span class="text-xs font-semibold uppercase tracking-wider text-slate-600 dark:text-slate-400">
                Standardausgabe (stdout)
              </span>
              <button
                v-if="job.stdout"
                type="button"
                class="inline-flex items-center gap-1 text-xs font-medium text-sky-600 hover:text-sky-700 dark:text-sky-400"
                @click="copyOutput('stdout')"
              >
                <Check v-if="copiedStream === 'stdout'" class="h-3.5 w-3.5 text-emerald-600 dark:text-emerald-400" />
                <Copy v-else class="h-3.5 w-3.5" />
                {{ copiedStream === 'stdout' ? 'Kopiert' : 'Kopieren' }}
              </button>
            </div>
            <pre
              data-testid="job-stdout"
              class="max-h-60 overflow-auto rounded-lg border border-slate-800 bg-slate-950 p-4 font-mono text-xs leading-relaxed text-emerald-300 shadow-inner"
            >{{ job.stdout || '(Keine Standardausgabe vorhanden)' }}</pre>
          </div>

          <!-- STDERR Terminal -->
          <div>
            <div class="mb-1.5 flex items-center justify-between">
              <span class="inline-flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-rose-700 dark:text-rose-400">
                <AlertTriangle v-if="job.stderr" class="h-3.5 w-3.5" aria-hidden="true" />
                Fehlerausgabe (stderr)
              </span>
              <button
                v-if="job.stderr"
                type="button"
                class="inline-flex items-center gap-1 text-xs font-medium text-sky-600 hover:text-sky-700 dark:text-sky-400"
                @click="copyOutput('stderr')"
              >
                <Check v-if="copiedStream === 'stderr'" class="h-3.5 w-3.5 text-emerald-600 dark:text-emerald-400" />
                <Copy v-else class="h-3.5 w-3.5" />
                {{ copiedStream === 'stderr' ? 'Kopiert' : 'Kopieren' }}
              </button>
            </div>
            <pre
              data-testid="job-stderr"
              class="max-h-52 overflow-auto rounded-lg border border-slate-800 bg-slate-950 p-4 font-mono text-xs leading-relaxed shadow-inner"
              :class="job.stderr ? 'text-rose-300' : 'text-slate-500'"
            >{{ job.stderr || '(Keine Fehlerausgabe vorhanden)' }}</pre>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>
