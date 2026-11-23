<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import {
  ArrowLeft,
  Ban,
  CheckCircle,
  Cpu,
  HardDrive,
  MessageSquareShare,
  MonitorCheck,
  MonitorOff,
  RefreshCw,
  RotateCcw,
  Save,
  Search,
  Terminal,
  Trash2,
  Zap,
} from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type {
  Device,
  DeviceGroup,
  DeviceNotifyPayload,
  DeviceRebootPayload,
  JobItem,
} from '@/api/types'
import { useToast } from '@/composables/useToast'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import DeviceNotifyModal from '@/components/DeviceNotifyModal.vue'
import DeviceRebootModal from '@/components/DeviceRebootModal.vue'
import JobLogModal from '@/components/JobLogModal.vue'
import OsBadge from '@/components/OsBadge.vue'
import StatusBadge from '@/components/StatusBadge.vue'
import { formatBytes, formatDateTime } from '@/utils/format'

const route = useRoute()
const router = useRouter()
const toast = useToast()

const deviceId = computed(() => String(route.params.id))
const device = ref<Device | null>(null)
const allGroups = ref<DeviceGroup[]>([])
const activeTab = ref<'overview' | 'software' | 'updates' | 'jobs'>('overview')
const softwareSearch = ref('')
const selectedGroupIds = ref<string[]>([])
const tagsInput = ref('')
const loading = ref(false)
const savingAssignment = ref(false)
const selectedJobId = ref<string | null>(null)
const selectedJobInitial = ref<JobItem | null>(null)

const confirmAction = ref<'revoke' | 'delete' | null>(null)
const confirmBusy = ref(false)

const notifyModalOpen = ref(false)
const notifyBusy = ref(false)
const rebootModalOpen = ref(false)
const rebootBusy = ref(false)
const syncBusy = ref(false)

const filteredSoftware = computed(() => {
  const list = device.value?.installed_software || []
  const q = softwareSearch.value.trim().toLowerCase()
  if (!q) return list
  return list.filter(
    (s) =>
      s.name.toLowerCase().includes(q) ||
      (s.version || '').toLowerCase().includes(q) ||
      (s.publisher || '').toLowerCase().includes(q) ||
      (s.source || '').toLowerCase().includes(q),
  )
})

async function loadDevice() {
  loading.value = true
  try {
    const [dev, grps] = await Promise.all([
      adminApi.getDevice(deviceId.value),
      adminApi.listGroups(),
    ])
    device.value = dev
    allGroups.value = grps
    selectedGroupIds.value = [...(dev.group_ids || [])]
    tagsInput.value = (dev.tags || []).join(', ')
  } catch (err) {
    toast.error('Gerät konnte nicht geladen werden', err instanceof Error ? err.message : undefined)
  } finally {
    loading.value = false
  }
}

async function saveGroupsAndTags() {
  if (!device.value) return
  savingAssignment.value = true
  try {
    const tags = tagsInput.value
      .split(',')
      .map((t) => t.trim())
      .filter(Boolean)
    const updated = await adminApi.updateDevice(device.value.id, {
      group_ids: selectedGroupIds.value,
      tags,
    })
    device.value = { ...device.value, ...updated }
    toast.success('Gruppen & Tags gespeichert')
  } catch (err) {
    toast.error('Speichern fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    savingAssignment.value = false
  }
}

async function handleApprove() {
  if (!device.value) return
  try {
    const updated = await adminApi.approveDevice(device.value.id)
    device.value = { ...device.value, ...updated }
    toast.success('Gerät freigegeben')
  } catch (err) {
    toast.error('Freigabe fehlgeschlagen', err instanceof Error ? err.message : undefined)
  }
}

async function handleScan() {
  if (!device.value) return
  try {
    await adminApi.scanDeviceUpdates(device.value.id)
    toast.success('Update-Scan gestartet')
    await loadDevice()
  } catch (err) {
    toast.error('Update-Scan fehlgeschlagen', err instanceof Error ? err.message : undefined)
  }
}

async function handleSync() {
  if (!device.value || syncBusy.value) return
  syncBusy.value = true
  try {
    await adminApi.syncDevice(device.value.id)
    toast.success(
      'Sofort-Synchronisation ausgelöst',
      `${device.value.hostname} gleicht Inventar, Katalog und Richtlinien ab.`,
    )
    await loadDevice()
  } catch (err) {
    toast.error('Synchronisation fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    syncBusy.value = false
  }
}

async function handleNotifySubmit(payload: DeviceNotifyPayload) {
  if (!device.value || notifyBusy.value) return
  notifyBusy.value = true
  try {
    await adminApi.notifyDevice(device.value.id, payload)
    notifyModalOpen.value = false
    toast.success(
      'Desktop-Nachricht gesendet',
      `Mitteilung „${payload.title}“ an ${device.value.hostname} übermittelt.`,
    )
    await loadDevice()
  } catch (err) {
    toast.error('Nachricht fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    notifyBusy.value = false
  }
}

async function handleRebootSubmit(payload: DeviceRebootPayload) {
  if (!device.value || rebootBusy.value) return
  rebootBusy.value = true
  try {
    await adminApi.rebootDevice(device.value.id, payload)
    rebootModalOpen.value = false
    if (payload.mode === 'cancel') {
      toast.success('Neustart abgebrochen', `Ausstehende Deadline auf ${device.value.hostname} aufgehoben.`)
    } else if (payload.mode === 'immediate') {
      toast.warning('Sofortiger Neustart gesendet', `${device.value.hostname} startet umgehend neu.`)
    } else {
      toast.success(
        'Neustart-Countdown gestartet',
        `Karenzzeit (${payload.grace_minutes ?? 60} Min.) auf ${device.value.hostname} aktiviert.`,
      )
    }
    await loadDevice()
  } catch (err) {
    toast.error('Neustart-Steuerung fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    rebootBusy.value = false
  }
}

async function executeConfirmedAction() {
  if (!device.value || !confirmAction.value) return
  confirmBusy.value = true
  try {
    if (confirmAction.value === 'revoke') {
      const updated = await adminApi.revokeDevice(device.value.id)
      device.value = { ...device.value, ...updated }
      toast.warning('Gerät gesperrt')
      confirmAction.value = null
    } else {
      await adminApi.deleteDevice(device.value.id)
      toast.success('Gerät gelöscht')
      confirmAction.value = null
      await router.push('/devices')
    }
  } catch (err) {
    toast.error('Aktion fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    confirmBusy.value = false
  }
}

function openJob(job: JobItem) {
  selectedJobInitial.value = job
  selectedJobId.value = job.id
}

onMounted(() => {
  loadDevice()
})
</script>

<template>
  <div class="space-y-6">
    <div>
      <RouterLink
        to="/devices"
        class="inline-flex items-center gap-1.5 text-xs font-semibold text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-100"
      >
        <ArrowLeft class="h-4 w-4" aria-hidden="true" />
        Zurück zur Flotten-Übersicht
      </RouterLink>
    </div>

    <template v-if="device">
      <!-- Top Summary Header -->
      <div
        class="flex flex-col justify-between gap-4 rounded-xl border border-slate-200 bg-white p-6 shadow-xs xl:flex-row xl:items-center dark:border-slate-800 dark:bg-slate-900"
      >
        <div class="space-y-2.5">
          <div class="flex flex-wrap items-center gap-2.5">
            <h1 class="text-2xl font-bold text-slate-900 dark:text-slate-50">{{ device.hostname }}</h1>
            <StatusBadge :variant="device.is_online ? 'online' : 'offline'" />
            <StatusBadge :variant="device.status" />
            <StatusBadge v-if="device.reboot_required" variant="reboot" />
            <span
              data-testid="device-center-status"
              class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium ring-1 ring-inset"
              :class="
                device.center_running
                  ? 'bg-cyan-50 text-cyan-800 ring-cyan-600/20 dark:bg-cyan-500/15 dark:text-cyan-300 dark:ring-cyan-400/30'
                  : 'bg-slate-100 text-slate-600 ring-slate-500/20 dark:bg-slate-800 dark:text-slate-400 dark:ring-slate-600/40'
              "
            >
              <MonitorCheck v-if="device.center_running" class="h-3.5 w-3.5" aria-hidden="true" />
              <MonitorOff v-else class="h-3.5 w-3.5" aria-hidden="true" />
              {{
                device.center_running
                  ? 'Software Center: Aktiv in Benutzersitzung'
                  : 'Kein aktives Tray / Headless'
              }}
            </span>
          </div>
          <div class="flex flex-wrap items-center gap-3 text-xs text-slate-500 dark:text-slate-400">
            <OsBadge
              :os-family="device.os_family"
              :os-name="device.os_name"
              :os-version="device.os_version"
            />
            <span>Build/Kernel: <strong class="font-mono text-slate-700 dark:text-slate-200">{{ device.kernel_or_build || '—' }}</strong></span>
            <span>Architektur: <strong class="font-mono text-slate-700 dark:text-slate-200">{{ device.arch }}</strong></span>
            <span>Agent: <strong class="font-mono text-slate-700 dark:text-slate-200">v{{ device.agent_version }}</strong></span>
            <span>Letzter Kontakt: {{ formatDateTime(device.last_seen_at) }}</span>
          </div>
        </div>

        <div class="flex flex-wrap items-center gap-2">
          <button
            v-if="device.status !== 'approved'"
            type="button"
            class="inline-flex items-center gap-1.5 rounded-lg bg-emerald-600 px-3.5 py-2 text-xs font-semibold text-white hover:bg-emerald-700"
            @click="handleApprove"
          >
            <CheckCircle class="h-4 w-4" aria-hidden="true" />
            Freigeben
          </button>

          <button
            v-if="device.status === 'approved'"
            type="button"
            data-testid="device-sync-btn"
            :disabled="syncBusy"
            class="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-3.5 py-2 text-xs font-semibold text-slate-800 hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-100 dark:hover:bg-slate-700"
            @click="handleSync"
          >
            <Zap class="h-4 w-4 text-sky-600 dark:text-sky-400" aria-hidden="true" />
            Jetzt synchronisieren
          </button>

          <button
            v-if="device.status === 'approved'"
            type="button"
            class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-3.5 py-2 text-xs font-semibold text-white hover:bg-sky-700"
            @click="handleScan"
          >
            <RefreshCw class="h-4 w-4" aria-hidden="true" />
            Update-Scan auslösen
          </button>

          <button
            v-if="device.status === 'approved'"
            type="button"
            data-testid="device-notify-btn"
            class="inline-flex items-center gap-1.5 rounded-lg border border-sky-200 bg-sky-50 px-3.5 py-2 text-xs font-semibold text-sky-800 hover:bg-sky-100 dark:border-sky-500/30 dark:bg-sky-500/15 dark:text-sky-300 dark:hover:bg-sky-500/25"
            @click="notifyModalOpen = true"
          >
            <MessageSquareShare class="h-4 w-4" aria-hidden="true" />
            Nachricht senden
          </button>

          <button
            v-if="device.status === 'approved'"
            type="button"
            data-testid="device-reboot-btn"
            class="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-3.5 py-2 text-xs font-semibold text-slate-800 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-100 dark:hover:bg-slate-700"
            @click="rebootModalOpen = true"
          >
            <RotateCcw class="h-4 w-4 text-amber-600 dark:text-amber-400" aria-hidden="true" />
            Neustart steuern
          </button>

          <button
            v-if="device.status === 'approved'"
            type="button"
            class="inline-flex items-center gap-1.5 rounded-lg border border-amber-300 bg-amber-50 px-3.5 py-2 text-xs font-semibold text-amber-800 hover:bg-amber-100 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300 dark:hover:bg-amber-500/20"
            @click="confirmAction = 'revoke'"
          >
            <Ban class="h-4 w-4" aria-hidden="true" />
            Sperren
          </button>
          <button
            type="button"
            aria-label="Gerät löschen"
            title="Gerät löschen"
            class="inline-flex items-center gap-1.5 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-xs font-semibold text-rose-700 hover:bg-rose-100 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-400 dark:hover:bg-rose-500/20"
            @click="confirmAction = 'delete'"
          >
            <Trash2 class="h-4 w-4" />
          </button>
        </div>
      </div>

      <!-- Detail Tabs -->
      <div class="flex overflow-x-auto gap-1 border-b border-slate-200 dark:border-slate-800">
        <button
          type="button"
          class="whitespace-nowrap border-b-2 px-4 py-2.5 text-xs font-semibold"
          :class="
            activeTab === 'overview'
              ? 'border-sky-600 text-sky-700 dark:border-sky-400 dark:text-sky-400'
              : 'border-transparent text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
          "
          @click="activeTab = 'overview'"
        >
          Hardware &amp; Gruppen
        </button>
        <button
          type="button"
          class="whitespace-nowrap border-b-2 px-4 py-2.5 text-xs font-semibold"
          :class="
            activeTab === 'software'
              ? 'border-sky-600 text-sky-700 dark:border-sky-400 dark:text-sky-400'
              : 'border-transparent text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
          "
          @click="activeTab = 'software'"
        >
          Installierte Software ({{ device.installed_software?.length || 0 }})
        </button>
        <button
          type="button"
          class="whitespace-nowrap border-b-2 px-4 py-2.5 text-xs font-semibold"
          :class="
            activeTab === 'updates'
              ? 'border-sky-600 text-sky-700 dark:border-sky-400 dark:text-sky-400'
              : 'border-transparent text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
          "
          @click="activeTab = 'updates'"
        >
          Ausstehende OS-Updates ({{ device.pending_os_updates?.length || 0 }})
        </button>
        <button
          type="button"
          class="whitespace-nowrap border-b-2 px-4 py-2.5 text-xs font-semibold"
          :class="
            activeTab === 'jobs'
              ? 'border-sky-600 text-sky-700 dark:border-sky-400 dark:text-sky-400'
              : 'border-transparent text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
          "
          @click="activeTab = 'jobs'"
        >
          Aufträge ({{ device.recent_jobs?.length || 0 }})
        </button>
      </div>

      <!-- TAB: OVERVIEW -->
      <div v-if="activeTab === 'overview'" class="grid grid-cols-1 gap-6 lg:grid-cols-2">
        <!-- System & Hardware -->
        <div
          class="space-y-4 rounded-xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900"
        >
          <div class="flex items-center gap-2 text-sm font-bold text-slate-900 dark:text-slate-100">
            <Cpu class="h-4 w-4 text-sky-600 dark:text-sky-400" aria-hidden="true" />
            Hardware- &amp; Netzwerk-Inventar
          </div>
          <dl class="grid grid-cols-2 gap-4 text-xs">
            <div>
              <dt class="text-slate-500 dark:text-slate-400">Prozessor (CPU)</dt>
              <dd class="mt-0.5 font-semibold text-slate-900 dark:text-slate-100">
                {{ device.hardware?.cpu_model || '—' }}
                <span v-if="device.hardware?.cpu_cores">({{ device.hardware.cpu_cores }} Kerne)</span>
              </dd>
            </div>
            <div>
              <dt class="text-slate-500 dark:text-slate-400">Arbeitsspeicher (RAM)</dt>
              <dd class="mt-0.5 font-semibold text-slate-900 dark:text-slate-100">
                <template v-if="device.hardware?.ram_total_mb">
                  {{ Math.round((device.hardware.ram_total_mb / 1024) * 10) / 10 }} GB gesamt
                  ({{ Math.round(((device.hardware.ram_free_mb || 0) / 1024) * 10) / 10 }} GB frei)
                </template>
                <template v-else>—</template>
              </dd>
            </div>
            <div>
              <dt class="text-slate-500 dark:text-slate-400">Lokale IP-Adressen</dt>
              <dd class="mt-0.5 font-mono text-slate-800 dark:text-slate-200">
                {{ (device.ip_addresses || []).join(', ') || '—' }}
              </dd>
            </div>
            <div>
              <dt class="text-slate-500 dark:text-slate-400">Angemeldete Benutzer</dt>
              <dd class="mt-0.5 font-semibold text-slate-800 dark:text-slate-200">
                {{ (device.logged_in_users || []).join(', ') || 'Keine aktive Sitzung' }}
              </dd>
            </div>
            <div>
              <dt class="text-slate-500 dark:text-slate-400">Domain / Workgroup</dt>
              <dd class="mt-0.5 font-semibold text-slate-800 dark:text-slate-200">{{ device.domain || '—' }}</dd>
            </div>
            <div>
              <dt class="text-slate-500 dark:text-slate-400">Machine-UID</dt>
              <dd class="mt-0.5 truncate font-mono text-slate-600 dark:text-slate-400">{{ device.machine_uid || '—' }}</dd>
            </div>
          </dl>

          <!-- Disks -->
          <div v-if="device.hardware?.disks?.length" class="border-t border-slate-100 pt-4 dark:border-slate-800">
            <div class="mb-2 flex items-center gap-1.5 text-xs font-semibold text-slate-700 dark:text-slate-300">
              <HardDrive class="h-3.5 w-3.5 text-slate-500 dark:text-slate-400" aria-hidden="true" />
              Laufwerke &amp; Partitionen
            </div>
            <div class="space-y-2">
              <div
                v-for="(disk, idx) in device.hardware.disks"
                :key="idx"
                class="rounded-lg bg-slate-50 p-2.5 text-xs dark:bg-slate-800/60"
              >
                <div class="flex justify-between font-medium text-slate-800 dark:text-slate-200">
                  <span class="font-mono">{{ disk.mount || disk.mount_point || disk.name }}</span>
                  <span>{{ disk.free_gb ?? '—' }} GB frei von {{ disk.total_gb ?? '—' }} GB</span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Groups & Tags Assignment -->
        <div
          class="space-y-4 rounded-xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900"
        >
          <h3 class="text-sm font-bold text-slate-900 dark:text-slate-100">Gerätegruppen &amp; Tags zuordnen</h3>
          <div>
            <label class="mb-1.5 block text-xs font-semibold text-slate-700 dark:text-slate-300">Zugewiesene Gruppen</label>
            <div class="max-h-44 space-y-1.5 overflow-y-auto rounded-lg border border-slate-200 bg-slate-50 p-3 dark:border-slate-800 dark:bg-slate-950/60">
              <div v-if="allGroups.length === 0" class="text-xs text-slate-400">
                Noch keine Gruppen vorhanden.
              </div>
              <label
                v-for="grp in allGroups"
                :key="grp.id"
                class="flex cursor-pointer items-center gap-2 text-xs text-slate-800 dark:text-slate-200"
              >
                <input
                  v-model="selectedGroupIds"
                  :value="grp.id"
                  type="checkbox"
                  class="h-3.5 w-3.5 rounded border-slate-300 text-sky-600"
                />
                <span class="font-medium">{{ grp.name }}</span>
              </label>
            </div>
          </div>

          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Tags (kommagetrennt)</label>
            <input
              v-model="tagsInput"
              type="text"
              placeholder="z. B. pilot, buchhaltung, laptop"
              class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-xs dark:border-slate-700 dark:bg-slate-950"
            />
          </div>

          <button
            type="button"
            :disabled="savingAssignment"
            class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white hover:bg-sky-700 disabled:opacity-60"
            @click="saveGroupsAndTags"
          >
            <Save class="h-3.5 w-3.5" aria-hidden="true" />
            {{ savingAssignment ? 'Speichere…' : 'Zuordnung speichern' }}
          </button>
        </div>
      </div>

      <!-- TAB: INSTALLED SOFTWARE -->
      <div
        v-if="activeTab === 'software'"
        class="space-y-4 rounded-xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900"
      >
        <div class="relative max-w-md">
          <Search class="pointer-events-none absolute left-3 top-2.5 h-4 w-4 text-slate-400" aria-hidden="true" />
          <input
            v-model="softwareSearch"
            aria-label="Installierte Software durchsuchen"
            type="text"
            placeholder="Installierte Anwendung, Herausgeber oder Version filtern…"
            class="w-full rounded-lg border border-slate-300 py-2 pl-9 pr-3 text-xs dark:border-slate-700 dark:bg-slate-950"
          />
        </div>

        <div class="overflow-x-auto">
          <table class="min-w-full divide-y divide-slate-200 text-xs dark:divide-slate-800">
            <thead class="bg-slate-50 text-left font-semibold uppercase text-slate-500 dark:bg-slate-800/70 dark:text-slate-400">
              <tr>
                <th class="px-4 py-2.5">Anwendung</th>
                <th class="px-4 py-2.5">Version</th>
                <th class="px-4 py-2.5">Herausgeber</th>
                <th class="px-4 py-2.5">Quelle</th>
                <th class="px-4 py-2.5">Scope</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-slate-100 dark:divide-slate-800/70">
              <tr v-if="filteredSoftware.length === 0">
                <td colspan="5" class="px-4 py-8 text-center text-slate-400">
                  Keine Software-Einträge gefunden.
                </td>
              </tr>
              <tr v-for="(sw, idx) in filteredSoftware" :key="idx" class="hover:bg-slate-50 dark:hover:bg-slate-800/50">
                <td class="px-4 py-2.5 font-medium text-slate-900 dark:text-slate-100">{{ sw.name }}</td>
                <td class="px-4 py-2.5 font-mono text-slate-700 dark:text-slate-300">{{ sw.version }}</td>
                <td class="px-4 py-2.5 text-slate-600 dark:text-slate-400">{{ sw.publisher || sw.vendor || '—' }}</td>
                <td class="px-4 py-2.5">
                  <span class="rounded bg-slate-100 px-2 py-0.5 font-mono dark:bg-slate-800 dark:text-slate-300">{{ sw.source || 'system' }}</span>
                </td>
                <td class="px-4 py-2.5 text-slate-600 dark:text-slate-400">{{ sw.scope || 'machine' }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <!-- TAB: PENDING OS UPDATES -->
      <div
        v-if="activeTab === 'updates'"
        class="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900"
      >
        <div class="overflow-x-auto">
          <table class="min-w-full divide-y divide-slate-200 text-xs dark:divide-slate-800">
            <thead class="bg-slate-50 text-left font-semibold uppercase text-slate-500 dark:bg-slate-800/70 dark:text-slate-400">
              <tr>
                <th class="px-4 py-3">Update-ID / KB</th>
                <th class="px-4 py-3">Titel</th>
                <th class="px-4 py-3">Schweregrad</th>
                <th class="px-4 py-3">Kategorie</th>
                <th class="px-4 py-3">Größe</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-slate-100 dark:divide-slate-800/70">
              <tr v-if="!device.pending_os_updates?.length">
                <td colspan="5" class="px-4 py-8 text-center text-slate-400">
                  Keine ausstehenden OS-Updates gemeldet — System ist aktuell.
                </td>
              </tr>
              <tr v-for="upd in device.pending_os_updates" :key="upd.update_id" class="hover:bg-slate-50 dark:hover:bg-slate-800/50">
                <td class="px-4 py-3 font-mono font-semibold text-slate-900 dark:text-slate-100">{{ upd.update_id }}</td>
                <td class="px-4 py-3 text-slate-800 dark:text-slate-200">{{ upd.title }}</td>
                <td class="px-4 py-3">
                  <StatusBadge variant="severity" :value="upd.severity || 'moderate'" />
                </td>
                <td class="px-4 py-3 text-slate-600 dark:text-slate-400">{{ upd.category || '—' }}</td>
                <td class="px-4 py-3 text-slate-500 dark:text-slate-400">{{ formatBytes(upd.size_bytes) }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <!-- TAB: JOBS -->
      <div
        v-if="activeTab === 'jobs'"
        class="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900"
      >
        <div class="overflow-x-auto">
          <table class="min-w-full divide-y divide-slate-200 text-xs dark:divide-slate-800">
            <thead class="bg-slate-50 text-left font-semibold uppercase text-slate-500 dark:bg-slate-800/70 dark:text-slate-400">
              <tr>
                <th class="px-4 py-3">Typ / Aktion</th>
                <th class="px-4 py-3">Ziel</th>
                <th class="px-4 py-3">Status</th>
                <th class="px-4 py-3">Exit-Code</th>
                <th class="px-4 py-3">Zeitpunkt</th>
                <th class="px-4 py-3 text-right">Protokoll</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-slate-100 dark:divide-slate-800/70">
              <tr v-if="!device.recent_jobs?.length">
                <td colspan="6" class="px-4 py-8 text-center text-slate-400">
                  Noch keine Aufträge für dieses Gerät vorhanden.
                </td>
              </tr>
              <tr v-for="job in device.recent_jobs" :key="job.id" class="hover:bg-slate-50 dark:hover:bg-slate-800/50">
                <td class="px-4 py-3 font-medium text-slate-900 dark:text-slate-100">{{ job.job_type }} / {{ job.action }}</td>
                <td class="px-4 py-3 font-mono text-slate-600 dark:text-slate-300">{{ job.target_id }}</td>
                <td class="px-4 py-3"><StatusBadge variant="job" :value="job.status" /></td>
                <td class="px-4 py-3 font-mono dark:text-slate-300">{{ job.exit_code ?? '—' }}</td>
                <td class="px-4 py-3 text-slate-500 dark:text-slate-400">{{ formatDateTime(job.created_at) }}</td>
                <td class="px-4 py-3 text-right">
                  <button
                    type="button"
                    aria-label="Auftragsprotokoll öffnen"
                    class="inline-flex items-center gap-1 rounded border border-slate-200 px-2.5 py-1 font-medium text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
                    @click="openJob(job)"
                  >
                    <Terminal class="h-3.5 w-3.5" aria-hidden="true" />
                    Log öffnen
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </template>

    <DeviceNotifyModal
      :open="notifyModalOpen"
      :target-label="device?.hostname || ''"
      :loading="notifyBusy"
      @cancel="notifyModalOpen = false"
      @submit="handleNotifySubmit"
    />

    <DeviceRebootModal
      :open="rebootModalOpen"
      :hostname="device?.hostname || ''"
      :reboot-required="Boolean(device?.reboot_required)"
      :loading="rebootBusy"
      @cancel="rebootModalOpen = false"
      @submit="handleRebootSubmit"
    />

    <JobLogModal
      :open="Boolean(selectedJobId)"
      :job-id="selectedJobId"
      :initial-job="selectedJobInitial"
      @close="selectedJobId = null"
    />

    <ConfirmDialog
      :open="Boolean(confirmAction)"
      :variant="confirmAction === 'revoke' ? 'warning' : 'danger'"
      :title="
        confirmAction === 'revoke'
          ? `Gerät „${device?.hostname}“ sperren?`
          : `Gerät „${device?.hostname}“ endgültig löschen?`
      "
      :message="
        confirmAction === 'revoke'
          ? 'Der Agent verliert umgehend die Berechtigung zum Abruf von Aufträgen und Katalog-Manifesten.'
          : 'Das Gerät und alle zugehörigen Inventardaten werden unwiderruflich aus NS8-SoftwareManager entfernt.'
      "
      :confirm-label="confirmAction === 'revoke' ? 'Gerät sperren' : 'Endgültig löschen'"
      :loading="confirmBusy"
      @cancel="confirmAction = null"
      @confirm="executeConfirmedAction"
    />
  </div>
</template>
