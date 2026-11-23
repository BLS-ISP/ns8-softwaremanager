<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Ban, CheckCircle2, RefreshCw, Search, ShieldCheck, X } from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type { DeviceGroup, OsPatch } from '@/api/types'
import { useToast } from '@/composables/useToast'
import EmptyState from '@/components/EmptyState.vue'
import OsBadge from '@/components/OsBadge.vue'
import StatusBadge from '@/components/StatusBadge.vue'
import { formatBytes } from '@/utils/format'

const toast = useToast()

const patches = ref<OsPatch[]>([])
const groups = ref<DeviceGroup[]>([])
const loading = ref(false)

const osFilter = ref('')
const severityFilter = ref('')
const search = ref('')
const showExcludedOnly = ref(false)

const selectedUpdateIds = ref<string[]>([])
const approveModalOpen = ref(false)
const targetGroupIds = ref<string[]>([])
const respectMaintenanceWindow = ref(true)
const approving = ref(false)

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && approveModalOpen.value && !approving.value) {
    approveModalOpen.value = false
  }
}

watch(approveModalOpen, (isOpen) => {
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

const visiblePatches = computed(() => {
  return patches.value.filter((p) => {
    if (showExcludedOnly.value && !p.excluded) return false
    if (osFilter.value && p.os_family !== osFilter.value) return false
    if (severityFilter.value && (p.severity || '').toLowerCase() !== severityFilter.value) return false
    if (search.value.trim()) {
      const q = search.value.trim().toLowerCase()
      return (
        p.update_id.toLowerCase().includes(q) ||
        p.title.toLowerCase().includes(q)
      )
    }
    return true
  })
})

async function loadData() {
  loading.value = true
  try {
    const [patchList, grpList] = await Promise.all([
      adminApi.listPatches(),
      adminApi.listGroups(),
    ])
    patches.value = patchList
    groups.value = grpList
  } catch (err) {
    toast.error('OS-Patches konnten nicht geladen werden', err instanceof Error ? err.message : undefined)
  } finally {
    loading.value = false
  }
}

watch([osFilter, severityFilter], () => {
  selectedUpdateIds.value = []
})

function toggleSelectAll(event: Event) {
  const checked = (event.target as HTMLInputElement).checked
  if (checked) {
    selectedUpdateIds.value = visiblePatches.value.filter((p) => !p.excluded).map((p) => p.update_id)
  } else {
    selectedUpdateIds.value = []
  }
}

async function handleApproveSelected() {
  if (selectedUpdateIds.value.length === 0) return
  approving.value = true
  try {
    const res = await adminApi.approvePatches({
      update_ids: selectedUpdateIds.value,
      group_ids: targetGroupIds.value,
      respect_maintenance_window: respectMaintenanceWindow.value,
    })
    toast.success(
      'OS-Updates freigegeben',
      `${res.jobs_created ?? selectedUpdateIds.value.length} signierte Patch-Aufträge erstellt.`,
    )
    approveModalOpen.value = false
    selectedUpdateIds.value = []
    await loadData()
  } catch (err) {
    toast.error('Freigabe fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    approving.value = false
  }
}

async function toggleExclude(patch: OsPatch) {
  try {
    const nextExcluded = !patch.excluded
    await adminApi.excludePatches({
      update_ids: [patch.update_id],
      excluded: nextExcluded,
    })
    patch.excluded = nextExcluded
    toast.info(
      nextExcluded ? 'Update ausgeschlossen (KB gesperrt)' : 'Update-Sperre aufgehoben',
      patch.update_id,
    )
  } catch (err) {
    toast.error('Aktion fehlgeschlagen', err instanceof Error ? err.message : undefined)
  }
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
          Betriebssystem-Update-Management (OS Patching)
        </h1>
        <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
          Windows Update Agent (WUA / KB-Steuerung ohne WSUS), Linux APT &amp; DNF sowie Apple macOS <code class="font-mono text-xs">softwareupdate</code>
        </p>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <button
          v-if="selectedUpdateIds.length > 0"
          type="button"
          class="inline-flex items-center gap-1.5 rounded-lg bg-emerald-600 px-4 py-2 text-xs font-semibold text-white shadow-sm hover:bg-emerald-700"
          @click="approveModalOpen = true"
        >
          <ShieldCheck class="h-4 w-4" aria-hidden="true" />
          {{ selectedUpdateIds.length }} Update(s) freigeben
        </button>
        <button
          type="button"
          aria-label="Patch-Liste aktualisieren"
          class="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-3.5 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
          @click="loadData"
        >
          <RefreshCw class="h-4 w-4" :class="{ 'animate-spin': loading }" />
          Aktualisieren
        </button>
      </div>
    </div>

    <!-- Filters -->
    <div
      class="grid grid-cols-1 gap-3 rounded-xl border border-slate-200 bg-white p-4 shadow-xs sm:grid-cols-4 dark:border-slate-800 dark:bg-slate-900"
    >
      <div class="relative">
        <Search class="pointer-events-none absolute left-3 top-2.5 h-4 w-4 text-slate-400" aria-hidden="true" />
        <input
          v-model="search"
          aria-label="OS-Updates durchsuchen"
          type="text"
          placeholder="KB-Nummer oder Paketname suchen…"
          class="w-full rounded-lg border border-slate-300 py-2 pl-9 pr-3 text-xs dark:border-slate-700 dark:bg-slate-950"
        />
      </div>

      <select
        v-model="osFilter"
        aria-label="Nach Betriebssystem filtern"
        class="rounded-lg border border-slate-300 bg-white px-3 py-2 text-xs font-medium text-slate-700 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-200"
      >
        <option value="">Windows, Linux &amp; macOS</option>
        <option value="windows">Nur Windows (WUA / KB)</option>
        <option value="linux">Nur Linux (APT / DNF)</option>
        <option value="macos">Nur macOS (softwareupdate)</option>
      </select>

      <select
        v-model="severityFilter"
        aria-label="Nach Kritikalität filtern"
        class="rounded-lg border border-slate-300 bg-white px-3 py-2 text-xs font-medium text-slate-700 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-200"
      >
        <option value="">Alle Kritikalitäten</option>
        <option value="critical">Kritisch (Critical)</option>
        <option value="security">Sicherheitsupdate (Security)</option>
        <option value="important">Wichtig (Important)</option>
        <option value="moderate">Mittel / Optional</option>
      </select>

      <label class="flex items-center gap-2 rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-xs font-medium text-slate-700 dark:border-slate-800 dark:bg-slate-950/60 dark:text-slate-300">
        <input v-model="showExcludedOnly" type="checkbox" class="h-3.5 w-3.5 rounded border-slate-300 text-sky-600" />
        Nur gesperrte Updates (KB-Ausschluss)
      </label>
    </div>

    <!-- Empty State -->
    <div v-if="visiblePatches.length === 0 && !loading">
      <EmptyState
        :icon="ShieldCheck"
        title="Keine ausstehenden OS-Updates gefunden"
        description="Alle angebundenen Systeme sind auf dem aktuellen Patch-Stand oder entsprechen den gewählten Filterkriterien."
      >
        <template #actions>
          <button
            type="button"
            class="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-3.5 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
            @click="loadData"
          >
            <RefreshCw class="h-3.5 w-3.5" aria-hidden="true" />
            Patch-Bestand erneut prüfen
          </button>
        </template>
      </EmptyState>
    </div>

    <!-- Patches Table -->
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
              <th class="w-10 px-4 py-3">
                <input
                  type="checkbox"
                  aria-label="Alle sichtbaren Updates auswählen"
                  class="h-3.5 w-3.5 rounded border-slate-300 text-sky-600"
                  @change="toggleSelectAll"
                />
              </th>
              <th class="px-4 py-3">Update-ID / KB</th>
              <th class="px-4 py-3">Titel</th>
              <th class="px-4 py-3">Plattform</th>
              <th class="px-4 py-3">Schweregrad</th>
              <th class="px-4 py-3">Betroffene Systeme</th>
              <th class="px-4 py-3">Größe</th>
              <th class="px-4 py-3 text-right">Steuerung</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-100 dark:divide-slate-800/70">
            <tr
              v-for="patch in visiblePatches"
              :key="patch.update_id"
              class="hover:bg-slate-50/80 dark:hover:bg-slate-800/50"
              :class="{ 'bg-slate-50 opacity-60 dark:bg-slate-900/40': patch.excluded }"
            >
              <td class="px-4 py-3">
                <input
                  v-model="selectedUpdateIds"
                  :value="patch.update_id"
                  :disabled="patch.excluded"
                  :aria-label="`Update ${patch.update_id} auswählen`"
                  type="checkbox"
                  class="h-3.5 w-3.5 rounded border-slate-300 text-sky-600"
                />
              </td>
              <td class="px-4 py-3 font-mono text-xs font-bold text-slate-900 dark:text-slate-100">
                {{ patch.update_id }}
              </td>
              <td class="px-4 py-3">
                <div class="text-xs font-medium text-slate-900 dark:text-slate-100">{{ patch.title }}</div>
                <div class="mt-0.5 flex flex-wrap items-center gap-2 text-[11px] text-slate-500 dark:text-slate-400">
                  <span v-if="patch.category">{{ patch.category }}</span>
                  <span v-if="patch.approved" class="font-semibold text-emerald-600 dark:text-emerald-400">✓ Freigegeben</span>
                  <span v-if="patch.excluded" class="font-semibold text-rose-600 dark:text-rose-400">Gesperrt (Ausgeschlossen)</span>
                </div>
              </td>
              <td class="px-4 py-3">
                <OsBadge :os-family="patch.os_family" />
              </td>
              <td class="px-4 py-3">
                <StatusBadge variant="severity" :value="patch.severity" />
              </td>
              <td class="px-4 py-3 text-xs font-semibold text-slate-800 dark:text-slate-200">
                {{ patch.affected_device_count ?? 0 }} Gerät(e)
              </td>
              <td class="px-4 py-3 text-xs text-slate-500 dark:text-slate-400">
                {{ formatBytes(patch.size_bytes) }}
              </td>
              <td class="px-4 py-3 text-right">
                <div class="inline-flex items-center gap-1.5">
                  <button
                    v-if="!patch.excluded"
                    type="button"
                    class="inline-flex items-center gap-1 rounded-md bg-emerald-600 px-2.5 py-1 text-xs font-semibold text-white hover:bg-emerald-700"
                    @click="selectedUpdateIds = [patch.update_id]; approveModalOpen = true"
                  >
                    <CheckCircle2 class="h-3.5 w-3.5" aria-hidden="true" />
                    Freigeben
                  </button>
                  <button
                    type="button"
                    class="inline-flex items-center gap-1 rounded-md border px-2.5 py-1 text-xs font-medium"
                    :class="
                      patch.excluded
                        ? 'border-slate-300 bg-white text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700'
                        : 'border-rose-200 bg-rose-50 text-rose-700 hover:bg-rose-100 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-400 dark:hover:bg-rose-500/20'
                    "
                    @click="toggleExclude(patch)"
                  >
                    <Ban class="h-3.5 w-3.5" aria-hidden="true" />
                    {{ patch.excluded ? 'Entsperren' : 'Ausschließen' }}
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Approve Modal -->
    <div
      v-if="approveModalOpen"
      role="dialog"
      aria-modal="true"
      aria-label="OS-Updates zur Installation freigeben"
      class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/65 p-4 backdrop-blur-sm"
      @click.self="approveModalOpen = false"
    >
      <div
        class="w-full max-w-lg overflow-hidden rounded-xl border border-slate-200 bg-white shadow-2xl dark:border-slate-800 dark:bg-slate-900"
      >
        <div class="flex items-center justify-between border-b border-slate-800 bg-slate-900 px-6 py-4 text-white dark:bg-slate-950">
          <h3 class="text-base font-semibold">
            {{ selectedUpdateIds.length }} OS-Update(s) zur Installation freigeben
          </h3>
          <button
            type="button"
            aria-label="Dialog schließen"
            class="rounded-lg p-1 text-slate-400 hover:bg-slate-800 hover:text-white"
            @click="approveModalOpen = false"
          >
            <X class="h-5 w-5" />
          </button>
        </div>

        <form class="space-y-4 p-6" @submit.prevent="handleApproveSelected">
          <div>
            <label class="mb-1.5 block text-xs font-semibold text-slate-700 dark:text-slate-300">
              Auf Gerätegruppen beschränken (leer = alle betroffenen Systeme)
            </label>
            <div class="max-h-40 space-y-1.5 overflow-y-auto rounded-lg border border-slate-200 bg-slate-50 p-3 dark:border-slate-800 dark:bg-slate-950/60">
              <div v-if="groups.length === 0" class="text-xs text-slate-400">
                Keine Gruppen definiert — gilt für alle betroffenen Geräte.
              </div>
              <label
                v-for="grp in groups"
                :key="grp.id"
                class="flex cursor-pointer items-center gap-2 text-xs text-slate-800 dark:text-slate-200"
              >
                <input
                  v-model="targetGroupIds"
                  :value="grp.id"
                  type="checkbox"
                  class="h-3.5 w-3.5 rounded border-slate-300 text-sky-600"
                />
                <span>{{ grp.name }}</span>
              </label>
            </div>
          </div>

          <label class="flex items-center gap-3 rounded-lg border border-slate-200 bg-slate-50 p-3.5 dark:border-slate-800 dark:bg-slate-950/60">
            <input
              v-model="respectMaintenanceWindow"
              type="checkbox"
              class="h-4 w-4 rounded border-slate-300 text-sky-600"
            />
            <div>
              <div class="text-sm font-medium text-slate-900 dark:text-slate-100">Wartungsfenster einhalten</div>
              <div class="text-xs text-slate-500 dark:text-slate-400">
                Installation startet erst im nächsten definierten Wartungsfenster der Gruppe.
              </div>
            </div>
          </label>

          <div class="flex justify-end gap-2 border-t border-slate-200 pt-4 dark:border-slate-800">
            <button
              type="button"
              class="rounded-lg border border-slate-300 bg-white px-4 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
              @click="approveModalOpen = false"
            >
              Abbrechen
            </button>
            <button
              type="submit"
              :disabled="approving"
              class="rounded-lg bg-emerald-600 px-4 py-2 text-xs font-semibold text-white hover:bg-emerald-700 disabled:opacity-60"
            >
              {{ approving ? 'Wird freigegeben…' : 'Jetzt freigeben' }}
            </button>
          </div>
        </form>
      </div>
    </div>
  </div>
</template>
