<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { CalendarClock, Plus, RefreshCw, Terminal, Trash2, X } from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type { DeviceGroup, JobItem, MaintenanceWindow } from '@/api/types'
import { useToast } from '@/composables/useToast'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import EmptyState from '@/components/EmptyState.vue'
import JobLogModal from '@/components/JobLogModal.vue'
import StatusBadge from '@/components/StatusBadge.vue'
import { formatDateTime, formatWeekdays, WEEKDAY_LABELS } from '@/utils/format'

const toast = useToast()

const activeSection = ref<'jobs' | 'windows'>('jobs')
const jobs = ref<JobItem[]>([])
const windows = ref<MaintenanceWindow[]>([])
const groups = ref<DeviceGroup[]>([])
const loading = ref(false)

const statusFilter = ref('')
const typeFilter = ref('')
const selectedJobId = ref<string | null>(null)
const selectedJobInitial = ref<JobItem | null>(null)

// Maintenance window modal
const mwModalOpen = ref(false)
const mwGroupId = ref('')
const mwName = ref('')
const mwDays = ref<number[]>([6])
const mwStartTime = ref('02:00')
const mwEndTime = ref('04:00')
const mwTimezone = ref('Europe/Berlin')
const mwCatchUp = ref(true)
const savingMw = ref(false)

const pendingDeleteWindow = ref<MaintenanceWindow | null>(null)
const deletingMw = ref(false)

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && mwModalOpen.value && !savingMw.value) {
    mwModalOpen.value = false
  }
}

watch(mwModalOpen, (isOpen) => {
  if (typeof window !== 'undefined') {
    if (isOpen) {
      window.addEventListener('keydown', onKeyDown)
    } else {
      window.removeEventListener('keydown', onKeyDown)
    }
  }
})

onBeforeUnmount(() => {
  if (typeof window !== 'undefined') {
    window.removeEventListener('keydown', onKeyDown)
  }
})

async function loadData() {
  loading.value = true
  try {
    const [jobList, mwList, grpList] = await Promise.all([
      adminApi.listJobs({
        status: statusFilter.value || undefined,
        job_type: typeFilter.value || undefined,
      }),
      adminApi.listMaintenanceWindows(),
      adminApi.listGroups(),
    ])
    jobs.value = jobList
    windows.value = mwList
    groups.value = grpList
    if (!mwGroupId.value && grpList.length > 0) {
      mwGroupId.value = grpList[0].id
    }
  } catch (err) {
    toast.error('Daten konnten nicht geladen werden', err instanceof Error ? err.message : undefined)
  } finally {
    loading.value = false
  }
}

watch([statusFilter, typeFilter], () => {
  loadData()
})

function toggleDay(day: number) {
  if (mwDays.value.includes(day)) {
    mwDays.value = mwDays.value.filter((d) => d !== day)
  } else {
    mwDays.value = [...mwDays.value, day]
  }
}

async function handleCreateWindow() {
  if (!mwGroupId.value || !mwName.value.trim() || mwDays.value.length === 0) {
    toast.error('Bitte Gruppe, Name und mindestens einen Wochentag wählen.')
    return
  }
  savingMw.value = true
  try {
    await adminApi.createMaintenanceWindow({
      group_id: mwGroupId.value,
      name: mwName.value.trim(),
      days_of_week: mwDays.value,
      start_time: mwStartTime.value,
      end_time: mwEndTime.value,
      timezone: mwTimezone.value.trim() || 'Europe/Berlin',
      catch_up: mwCatchUp.value,
    })
    toast.success('Wartungsfenster angelegt')
    mwModalOpen.value = false
    mwName.value = ''
    await loadData()
  } catch (err) {
    toast.error('Speichern fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    savingMw.value = false
  }
}

async function confirmDeleteWindow() {
  if (!pendingDeleteWindow.value) return
  deletingMw.value = true
  const target = pendingDeleteWindow.value
  try {
    await adminApi.deleteMaintenanceWindow(target.id)
    windows.value = windows.value.filter((w) => w.id !== target.id)
    toast.success('Wartungsfenster gelöscht', `${target.name} wurde entfernt.`)
    pendingDeleteWindow.value = null
  } catch (err) {
    toast.error('Löschen fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    deletingMw.value = false
  }
}

function openJob(job: JobItem) {
  selectedJobInitial.value = job
  selectedJobId.value = job.id
}

function resolveGroupName(groupId: string, fallback?: string): string {
  if (fallback) return fallback
  return groups.value.find((g) => g.id === groupId)?.name || groupId
}

onMounted(() => {
  loadData()
})
</script>

<template>
  <div class="space-y-6">
    <div class="flex flex-col justify-between gap-4 sm:flex-row sm:items-center">
      <div>
        <h1 class="text-2xl font-bold tracking-tight text-slate-900 dark:text-slate-50">
          Auftrags-Zentrale &amp; Wartungsfenster
        </h1>
        <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
          Ed25519-signierte Jobs mit vollständigem stdout/stderr-Protokoll sowie gruppenbasierte Wartungsfenster
        </p>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <button
          v-if="activeSection === 'windows'"
          type="button"
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white hover:bg-sky-700"
          @click="mwModalOpen = true"
        >
          <Plus class="h-4 w-4" aria-hidden="true" />
          Wartungsfenster anlegen
        </button>
        <button
          type="button"
          aria-label="Aufträge und Wartungsfenster aktualisieren"
          class="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-3.5 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
          @click="loadData"
        >
          <RefreshCw class="h-4 w-4" :class="{ 'animate-spin': loading }" />
          Aktualisieren
        </button>
      </div>
    </div>

    <!-- Section Switch -->
    <div class="flex gap-1 border-b border-slate-200 dark:border-slate-800">
      <button
        type="button"
        class="border-b-2 px-4 py-2.5 text-xs font-semibold"
        :class="
          activeSection === 'jobs'
            ? 'border-sky-600 text-sky-700 dark:border-sky-400 dark:text-sky-400'
            : 'border-transparent text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
        "
        @click="activeSection = 'jobs'"
      >
        Aufträge &amp; Ausführungs-Logs ({{ jobs.length }})
      </button>
      <button
        type="button"
        class="border-b-2 px-4 py-2.5 text-xs font-semibold"
        :class="
          activeSection === 'windows'
            ? 'border-sky-600 text-sky-700 dark:border-sky-400 dark:text-sky-400'
            : 'border-transparent text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
        "
        @click="activeSection = 'windows'"
      >
        Wartungsfenster ({{ windows.length }})
      </button>
    </div>

    <!-- SECTION: JOBS -->
    <template v-if="activeSection === 'jobs'">
      <div
        class="grid grid-cols-1 gap-3 rounded-xl border border-slate-200 bg-white p-4 shadow-xs sm:grid-cols-2 dark:border-slate-800 dark:bg-slate-900"
      >
        <select
          v-model="statusFilter"
          aria-label="Nach Auftragsstatus filtern"
          class="rounded-lg border border-slate-300 bg-white px-3 py-2 text-xs font-medium text-slate-700 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-200"
        >
          <option value="">Alle Auftragsstatus</option>
          <option value="queued">Warteschlange (Queued)</option>
          <option value="running">Wird ausgeführt (Running)</option>
          <option value="succeeded">Erfolgreich (Succeeded)</option>
          <option value="failed">Fehlgeschlagen (Failed)</option>
          <option value="waiting_for_process">Wartet auf Prozessende</option>
        </select>

        <select
          v-model="typeFilter"
          aria-label="Nach Auftragstyp filtern"
          class="rounded-lg border border-slate-300 bg-white px-3 py-2 text-xs font-medium text-slate-700 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-200"
        >
          <option value="">Alle Auftragstypen</option>
          <option value="software">Software-Rollout</option>
          <option value="os_patch">OS-Patching</option>
          <option value="scan_updates">Update-Scan</option>
          <option value="self_update">Agent-Self-Update</option>
        </select>
      </div>

      <div v-if="jobs.length === 0 && !loading">
        <EmptyState
          :icon="Terminal"
          title="Keine Aufträge vorhanden"
          description="Sobald Sie OS-Patches freigeben, ein Software-Deployment starten oder einen Update-Scan anfordern, sehen Sie hier Status und stdout/stderr-Protokolle."
        />
      </div>

      <div
        v-else
        class="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900"
      >
        <div class="overflow-x-auto">
          <table class="min-w-full divide-y divide-slate-200 text-sm dark:divide-slate-800">
            <thead
              class="bg-slate-50 text-left text-xs font-semibold uppercase tracking-wider text-slate-500 dark:bg-slate-800/70 dark:text-slate-400"
            >
              <tr>
                <th class="px-4 py-3">Gerät</th>
                <th class="px-4 py-3">Typ &amp; Aktion</th>
                <th class="px-4 py-3">Ziel / Version</th>
                <th class="px-4 py-3">Status</th>
                <th class="px-4 py-3">Exit-Code</th>
                <th class="px-4 py-3">Erstellt / Beendet</th>
                <th class="px-4 py-3 text-right">Terminal-Log</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-slate-100 dark:divide-slate-800/70">
              <tr
                v-for="job in jobs"
                :key="job.id"
                class="hover:bg-slate-50/80 dark:hover:bg-slate-800/50"
              >
                <td class="px-4 py-3 font-semibold text-slate-900 dark:text-slate-100">
                  {{ job.device_hostname || job.hostname || job.device_id }}
                </td>
                <td class="px-4 py-3 text-xs text-slate-700 dark:text-slate-300">
                  <span class="font-semibold">{{ job.job_type }}</span> ·
                  <span class="font-mono">{{ job.action }}</span>
                </td>
                <td class="px-4 py-3 font-mono text-xs text-slate-600 dark:text-slate-400">
                  {{ job.target_name || job.target_id || '—' }}
                  <span v-if="job.version">({{ job.version }})</span>
                </td>
                <td class="px-4 py-3">
                  <div class="flex flex-wrap items-center gap-1.5">
                    <StatusBadge variant="job" :value="job.status" />
                    <StatusBadge v-if="job.reboot_required" variant="reboot" />
                  </div>
                </td>
                <td class="px-4 py-3 font-mono text-xs">
                  <span
                    v-if="job.exit_code !== null && job.exit_code !== undefined"
                    class="rounded px-2 py-0.5 font-semibold"
                    :class="
                      job.exit_code === 0
                        ? 'bg-emerald-100 text-emerald-800 dark:bg-emerald-500/20 dark:text-emerald-300'
                        : job.exit_code === 3010
                          ? 'bg-amber-100 text-amber-800 dark:bg-amber-500/20 dark:text-amber-300'
                          : 'bg-rose-100 text-rose-800 dark:bg-rose-500/20 dark:text-rose-300'
                    "
                  >
                    {{ job.exit_code }}
                  </span>
                  <span v-else class="text-slate-400">—</span>
                </td>
                <td class="px-4 py-3 text-xs text-slate-500 dark:text-slate-400">
                  {{ formatDateTime(job.completed_at || job.created_at) }}
                </td>
                <td class="px-4 py-3 text-right">
                  <button
                    type="button"
                    aria-label="Auftragsprotokoll anzeigen"
                    class="inline-flex items-center gap-1 rounded-md border border-slate-200 px-2.5 py-1 text-xs font-medium text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
                    @click="openJob(job)"
                  >
                    <Terminal class="h-3.5 w-3.5" aria-hidden="true" />
                    stdout/stderr
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </template>

    <!-- SECTION: MAINTENANCE WINDOWS -->
    <template v-if="activeSection === 'windows'">
      <div v-if="windows.length === 0 && !loading">
        <EmptyState
          :icon="CalendarClock"
          title="Noch keine Wartungsfenster definiert"
          description="Definieren Sie feste Zeitfenster pro Gerätegruppe (inkl. Catch-up für mobile Arbeitsplätze), damit Updates nur außerhalb der Geschäftszeiten starten."
        >
          <template #actions>
            <button
              type="button"
              class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white hover:bg-sky-700"
              @click="mwModalOpen = true"
            >
              <Plus class="h-4 w-4" aria-hidden="true" />
              Erstes Wartungsfenster anlegen
            </button>
          </template>
        </EmptyState>
      </div>

      <div v-else class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
        <div
          v-for="mw in windows"
          :key="mw.id"
          class="flex flex-col justify-between rounded-xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900"
        >
          <div>
            <div class="flex items-start justify-between">
              <div class="flex items-center gap-2.5">
                <div class="rounded-lg bg-sky-50 p-2 text-sky-600 dark:bg-sky-500/15 dark:text-sky-400">
                  <CalendarClock class="h-5 w-5" aria-hidden="true" />
                </div>
                <div>
                  <h3 class="text-sm font-bold text-slate-900 dark:text-slate-100">{{ mw.name }}</h3>
                  <span class="text-xs text-slate-500 dark:text-slate-400">
                    Gruppe: <strong class="text-slate-700 dark:text-slate-200">{{ resolveGroupName(mw.group_id, mw.group_name) }}</strong>
                  </span>
                </div>
              </div>
              <button
                type="button"
                :aria-label="`Wartungsfenster ${mw.name} löschen`"
                title="Wartungsfenster löschen"
                class="rounded p-1.5 text-rose-500 hover:bg-rose-50 dark:text-rose-400 dark:hover:bg-rose-500/15"
                @click="pendingDeleteWindow = mw"
              >
                <Trash2 class="h-4 w-4" />
              </button>
            </div>

            <div class="mt-4 space-y-1.5 text-xs text-slate-700 dark:text-slate-300">
              <div>
                <span class="text-slate-500 dark:text-slate-400">Wochentage:</span>
                <strong class="ml-1">{{ formatWeekdays(mw.days_of_week) }}</strong>
              </div>
              <div>
                <span class="text-slate-500 dark:text-slate-400">Zeitfenster:</span>
                <strong class="ml-1 font-mono">{{ mw.start_time }} – {{ mw.end_time }}</strong>
                <span class="ml-1 text-slate-400">({{ mw.timezone }})</span>
              </div>
            </div>
          </div>

          <div class="mt-4 border-t border-slate-100 pt-3 text-xs dark:border-slate-800">
            <span
              v-if="mw.catch_up"
              class="rounded-full bg-emerald-50 px-2.5 py-0.5 font-semibold text-emerald-700 dark:bg-emerald-500/15 dark:text-emerald-300"
            >
              Catch-up aktiv (Nachholen bei ausgeschalteten Laptops)
            </span>
            <span v-else class="text-slate-500 dark:text-slate-400">Nur strikt innerhalb des Zeitfensters</span>
          </div>
        </div>
      </div>
    </template>

    <!-- Create Maintenance Window Modal -->
    <div
      v-if="mwModalOpen"
      role="dialog"
      aria-modal="true"
      aria-label="Neues Wartungsfenster definieren"
      class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/65 p-4 backdrop-blur-sm"
      @click.self="mwModalOpen = false"
    >
      <div
        class="w-full max-w-lg overflow-hidden rounded-xl border border-slate-200 bg-white shadow-2xl dark:border-slate-800 dark:bg-slate-900"
      >
        <div class="flex items-center justify-between border-b border-slate-800 bg-slate-900 px-6 py-4 text-white dark:bg-slate-950">
          <h3 class="text-base font-semibold">Neues Wartungsfenster definieren</h3>
          <button
            type="button"
            aria-label="Dialog schließen"
            class="rounded-lg p-1 text-slate-400 hover:bg-slate-800 hover:text-white"
            @click="mwModalOpen = false"
          >
            <X class="h-5 w-5" />
          </button>
        </div>

        <form class="space-y-4 p-6" @submit.prevent="handleCreateWindow">
          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Zielgruppe *</label>
            <select
              v-model="mwGroupId"
              class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
            >
              <option value="" disabled>— Bitte Gruppe wählen —</option>
              <option v-for="g in groups" :key="g.id" :value="g.id">{{ g.name }}</option>
            </select>
          </div>

          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Bezeichnung *</label>
            <input
              v-model="mwName"
              type="text"
              placeholder="z. B. Server-Wartung Sonntag Nacht"
              class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
            />
          </div>

          <div>
            <label class="mb-1.5 block text-xs font-semibold text-slate-700 dark:text-slate-300">Wochentage *</label>
            <div class="flex flex-wrap gap-1.5">
              <button
                v-for="day in WEEKDAY_LABELS"
                :key="day.value"
                type="button"
                class="rounded-lg border px-3 py-1.5 text-xs font-semibold transition"
                :class="
                  mwDays.includes(day.value)
                    ? 'border-sky-600 bg-sky-600 text-white'
                    : 'border-slate-300 bg-white text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700'
                "
                @click="toggleDay(day.value)"
              >
                {{ day.short }}
              </button>
            </div>
          </div>

          <div class="grid grid-cols-3 gap-3">
            <div>
              <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Startzeit</label>
              <input
                v-model="mwStartTime"
                type="time"
                class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
              />
            </div>
            <div>
              <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Endzeit</label>
              <input
                v-model="mwEndTime"
                type="time"
                class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
              />
            </div>
            <div>
              <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Zeitzone</label>
              <input
                v-model="mwTimezone"
                type="text"
                class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
              />
            </div>
          </div>

          <label class="flex items-center gap-3 rounded-lg border border-slate-200 bg-slate-50 p-3.5 dark:border-slate-800 dark:bg-slate-950/60">
            <input
              v-model="mwCatchUp"
              type="checkbox"
              class="h-4 w-4 rounded border-slate-300 text-sky-600"
            />
            <div>
              <div class="text-sm font-medium text-slate-900 dark:text-slate-100">Catch-up aktivieren</div>
              <div class="text-xs text-slate-500 dark:text-slate-400">
                War ein Client während des Fensters ausgeschaltet, wird der Auftrag beim nächsten Online-Kontakt nachgeholt.
              </div>
            </div>
          </label>

          <div class="flex justify-end gap-2 border-t border-slate-200 pt-4 dark:border-slate-800">
            <button
              type="button"
              class="rounded-lg border border-slate-300 bg-white px-4 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
              @click="mwModalOpen = false"
            >
              Abbrechen
            </button>
            <button
              type="submit"
              :disabled="savingMw"
              class="rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white hover:bg-sky-700 disabled:opacity-60"
            >
              Speichern
            </button>
          </div>
        </form>
      </div>
    </div>

    <JobLogModal
      :open="Boolean(selectedJobId)"
      :job-id="selectedJobId"
      :initial-job="selectedJobInitial"
      @close="selectedJobId = null"
    />

    <ConfirmDialog
      :open="Boolean(pendingDeleteWindow)"
      variant="danger"
      :title="`Wartungsfenster „${pendingDeleteWindow?.name}“ löschen?`"
      message="Das Wartungsfenster wird dauerhaft entfernt. Hat die Gruppe kein weiteres Wartungsfenster, dürfen Pflicht-Aufträge sofort starten."
      confirm-label="Wartungsfenster löschen"
      :loading="deletingMw"
      @cancel="pendingDeleteWindow = null"
      @confirm="confirmDeleteWindow"
    />
  </div>
</template>
