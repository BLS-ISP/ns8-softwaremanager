<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import {
  Ban,
  CheckCircle,
  Eye,
  FolderKanban,
  Laptop,
  MessageSquareShare,
  MonitorCheck,
  RefreshCw,
  Search,
  ShieldCheck,
  Trash2,
  X,
  Zap,
} from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type {
  BulkDeviceActionType,
  Device,
  DeviceGroup,
  DeviceNotifyPayload,
} from '@/api/types'
import { useToast } from '@/composables/useToast'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import DeviceNotifyModal from '@/components/DeviceNotifyModal.vue'
import EmptyState from '@/components/EmptyState.vue'
import OsBadge from '@/components/OsBadge.vue'
import StatusBadge from '@/components/StatusBadge.vue'
import { formatDateTime } from '@/utils/format'

const route = useRoute()
const toast = useToast()

const devices = ref<Device[]>([])
const groups = ref<DeviceGroup[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

const search = ref('')
const statusFilter = ref<string>((route.query.status as string) || '')
const osFilter = ref<string>('')
const groupFilter = ref<string>('')

const selectedIds = ref<string[]>([])
const bulkGroupId = ref<string>('')
const bulkBusy = ref(false)
const bulkNotifyOpen = ref(false)

const confirmTarget = ref<{ action: 'revoke' | 'delete'; device: Device } | null>(null)
const confirmBusy = ref(false)

const hasActiveFilters = computed(() =>
  Boolean(search.value.trim() || statusFilter.value || osFilter.value || groupFilter.value),
)

const allSelected = computed(
  () => devices.value.length > 0 && devices.value.every((d) => selectedIds.value.includes(d.id)),
)

const someSelected = computed(
  () => selectedIds.value.length > 0 && !allSelected.value,
)

function toggleSelectAll() {
  if (allSelected.value) {
    selectedIds.value = []
  } else {
    selectedIds.value = devices.value.map((d) => d.id)
  }
}

function toggleSelectDevice(id: string) {
  if (selectedIds.value.includes(id)) {
    selectedIds.value = selectedIds.value.filter((item) => item !== id)
  } else {
    selectedIds.value = [...selectedIds.value, id]
  }
}

function clearSelection() {
  selectedIds.value = []
}

function resetFilters() {
  search.value = ''
  statusFilter.value = ''
  osFilter.value = ''
  groupFilter.value = ''
  loadData()
}

async function loadData() {
  loading.value = true
  error.value = null
  try {
    const [devList, grpList] = await Promise.all([
      adminApi.listDevices({
        status: statusFilter.value || undefined,
        os_family: osFilter.value || undefined,
        group_id: groupFilter.value || undefined,
        search: search.value.trim() || undefined,
      }),
      adminApi.listGroups(),
    ])
    devices.value = devList
    groups.value = grpList
    const validIds = new Set(devList.map((d) => d.id))
    selectedIds.value = selectedIds.value.filter((id) => validIds.has(id))
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Geräteliste konnte nicht geladen werden'
  } finally {
    loading.value = false
  }
}

watch([statusFilter, osFilter, groupFilter], () => {
  loadData()
})

async function handleApprove(device: Device) {
  try {
    const updated = await adminApi.approveDevice(device.id)
    const idx = devices.value.findIndex((d) => d.id === device.id)
    if (idx !== -1) {
      devices.value[idx] = { ...devices.value[idx], ...updated, status: 'approved' }
    }
    toast.success('Gerät freigegeben', `${device.hostname} darf nun signierte Aufträge empfangen.`)
  } catch (err) {
    toast.error('Freigabe fehlgeschlagen', err instanceof Error ? err.message : undefined)
  }
}

async function handleSyncDevice(device: Device) {
  try {
    await adminApi.syncDevice(device.id)
    toast.success(
      'Sofort-Synchronisation gesendet',
      `${device.hostname} gleicht Inventar, Katalog und Richtlinien ab.`,
    )
  } catch (err) {
    toast.error('Synchronisation fehlgeschlagen', err instanceof Error ? err.message : undefined)
  }
}

function requestRevoke(device: Device) {
  confirmTarget.value = { action: 'revoke', device }
}

function requestDelete(device: Device) {
  confirmTarget.value = { action: 'delete', device }
}

async function executeConfirmedAction() {
  if (!confirmTarget.value) return
  const { action, device } = confirmTarget.value
  confirmBusy.value = true
  try {
    if (action === 'revoke') {
      const updated = await adminApi.revokeDevice(device.id)
      const idx = devices.value.findIndex((d) => d.id === device.id)
      if (idx !== -1) {
        devices.value[idx] = {
          ...devices.value[idx],
          ...updated,
          status: 'revoked',
          is_online: false,
        }
      }
      toast.warning(
        'Gerät gesperrt',
        `${device.hostname} wurde der API- und WebSocket-Zugriff entzogen.`,
      )
    } else {
      await adminApi.deleteDevice(device.id)
      devices.value = devices.value.filter((d) => d.id !== device.id)
      selectedIds.value = selectedIds.value.filter((id) => id !== device.id)
      toast.success('Gerät entfernt', `${device.hostname} wurde aus der Flotte gelöscht.`)
    }
    confirmTarget.value = null
  } catch (err) {
    toast.error(
      action === 'revoke' ? 'Sperren fehlgeschlagen' : 'Löschen fehlgeschlagen',
      err instanceof Error ? err.message : undefined,
    )
  } finally {
    confirmBusy.value = false
  }
}

async function handleScanUpdates(device: Device) {
  try {
    await adminApi.scanDeviceUpdates(device.id)
    toast.success(
      'Update-Scan beauftragt',
      `Signierter Scan-Auftrag für ${device.hostname} eingereiht.`,
    )
  } catch (err) {
    toast.error('Update-Scan fehlgeschlagen', err instanceof Error ? err.message : undefined)
  }
}

async function executeBulkAction(action: Exclude<BulkDeviceActionType, 'notify'>) {
  if (selectedIds.value.length === 0 || bulkBusy.value) return
  if (action === 'assign_group' && !bulkGroupId.value) {
    toast.warning('Bitte Zielgruppe wählen', 'Wählen Sie eine Gerätegruppe für die Zuweisung aus.')
    return
  }

  bulkBusy.value = true
  try {
    const res = await adminApi.bulkDeviceAction({
      device_ids: [...selectedIds.value],
      action,
      group_id: action === 'assign_group' ? bulkGroupId.value : undefined,
    })

    const actionLabels: Record<Exclude<BulkDeviceActionType, 'notify'>, string> = {
      approve: 'Freigabe ausgeführt',
      sync: 'Flotten-Synchronisation gestartet',
      scan_updates: 'OS-Update-Scan eingereiht',
      assign_group: 'Gruppenzuweisung aktualisiert',
    }

    toast.success(
      actionLabels[action],
      `${res.affected ?? selectedIds.value.length} System(e) verarbeitet.`,
    )
    await loadData()
  } catch (err) {
    toast.error('Sammelaktion fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    bulkBusy.value = false
  }
}

async function handleBulkNotifySubmit(payload: DeviceNotifyPayload) {
  if (selectedIds.value.length === 0 || bulkBusy.value) return
  bulkBusy.value = true
  try {
    const res = await adminApi.bulkDeviceAction({
      device_ids: [...selectedIds.value],
      action: 'notify',
      title: payload.title,
      message: payload.message,
      urgent: payload.urgent,
    })
    bulkNotifyOpen.value = false
    toast.success(
      'Desktop-Rundnachricht gesendet',
      `An ${res.affected ?? selectedIds.value.length} Gerät(e) übermittelt.`,
    )
  } catch (err) {
    toast.error('Nachricht konnte nicht gesendet werden', err instanceof Error ? err.message : undefined)
  } finally {
    bulkBusy.value = false
  }
}

onMounted(() => {
  loadData()
})
</script>

<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex flex-col justify-between gap-4 sm:flex-row sm:items-center">
      <div>
        <h1 class="text-2xl font-bold tracking-tight text-slate-900 dark:text-slate-50">
          Geräteverwaltung &amp; Flotte
        </h1>
        <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
          Echtzeit-Status, Onboarding-Freigaben, Flottensteuerung und Inventar aller Endpunkte
        </p>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <RouterLink
          to="/enrollment"
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-3.5 py-2 text-xs font-semibold text-white shadow-sm hover:bg-sky-700"
        >
          <ShieldCheck class="h-4 w-4" aria-hidden="true" />
          Geräte hinzufügen (Enrollment)
        </RouterLink>
        <button
          type="button"
          aria-label="Geräteliste aktualisieren"
          class="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-3.5 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
          @click="loadData"
        >
          <RefreshCw class="h-4 w-4" :class="{ 'animate-spin': loading }" />
          Aktualisieren
        </button>
      </div>
    </div>

    <!-- Filter Bar -->
    <div
      class="grid grid-cols-1 gap-3 rounded-xl border border-slate-200 bg-white p-4 shadow-xs sm:grid-cols-4 dark:border-slate-800 dark:bg-slate-900"
    >
      <form class="relative sm:col-span-1" @submit.prevent="loadData">
        <Search class="pointer-events-none absolute left-3 top-2.5 h-4 w-4 text-slate-400" aria-hidden="true" />
        <input
          v-model="search"
          data-testid="devices-search-input"
          aria-label="Geräte durchsuchen"
          type="text"
          placeholder="Hostname, IP, Software suchen…"
          class="w-full rounded-lg border border-slate-300 py-2 pl-9 pr-3 text-xs dark:border-slate-700 dark:bg-slate-950"
          @change="loadData"
        />
      </form>

      <select
        v-model="statusFilter"
        data-testid="devices-status-filter"
        aria-label="Nach Zulassungsstatus filtern"
        class="rounded-lg border border-slate-300 bg-white px-3 py-2 text-xs font-medium text-slate-700 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-200"
      >
        <option value="">Alle Zulassungs-Status</option>
        <option value="pending">Freigabe ausstehend (Pending)</option>
        <option value="approved">Freigegeben (Approved)</option>
        <option value="revoked">Gesperrt (Revoked)</option>
      </select>

      <select
        v-model="osFilter"
        data-testid="devices-os-filter"
        aria-label="Nach Betriebssystem filtern"
        class="rounded-lg border border-slate-300 bg-white px-3 py-2 text-xs font-medium text-slate-700 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-200"
      >
        <option value="">Alle Betriebssysteme</option>
        <option value="windows">Windows</option>
        <option value="linux">Linux</option>
      </select>

      <select
        v-model="groupFilter"
        data-testid="devices-group-filter"
        aria-label="Nach Gerätegruppe filtern"
        class="rounded-lg border border-slate-300 bg-white px-3 py-2 text-xs font-medium text-slate-700 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-200"
      >
        <option value="">Alle Gerätegruppen</option>
        <option v-for="g in groups" :key="g.id" :value="g.id">{{ g.name }}</option>
      </select>
    </div>

    <!-- Bulk Fleet Actions Toolbar -->
    <div
      v-if="selectedIds.length > 0"
      data-testid="bulk-actions-bar"
      class="flex flex-col gap-3 rounded-xl border border-sky-200 bg-gradient-to-r from-sky-50 via-cyan-50/70 to-white p-3.5 shadow-sm lg:flex-row lg:items-center lg:justify-between dark:border-sky-500/30 dark:from-sky-500/15 dark:via-slate-900 dark:to-slate-900"
    >
      <div class="flex items-center gap-2.5">
        <span
          class="inline-flex items-center justify-center rounded-lg bg-sky-600 px-2.5 py-1 text-xs font-bold text-white"
        >
          {{ selectedIds.length }} ausgewählt
        </span>
        <span class="text-xs font-medium text-slate-700 dark:text-slate-200">
          Flotten-Sammelsteuerung (Ed25519-signiert)
        </span>
        <button
          type="button"
          data-testid="bulk-clear-selection-btn"
          class="inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium text-slate-500 hover:bg-slate-200/70 hover:text-slate-800 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-200"
          @click="clearSelection"
        >
          <X class="h-3.5 w-3.5" aria-hidden="true" />
          Auswahl aufheben
        </button>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <button
          type="button"
          data-testid="bulk-approve-btn"
          :disabled="bulkBusy"
          class="inline-flex items-center gap-1.5 rounded-lg bg-emerald-600 px-3 py-1.5 text-xs font-semibold text-white shadow-xs hover:bg-emerald-700 disabled:opacity-50"
          @click="executeBulkAction('approve')"
        >
          <CheckCircle class="h-3.5 w-3.5" aria-hidden="true" />
          Freigeben
        </button>

        <button
          type="button"
          data-testid="bulk-sync-btn"
          :disabled="bulkBusy"
          class="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-3 py-1.5 text-xs font-semibold text-slate-800 hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-100 dark:hover:bg-slate-700"
          @click="executeBulkAction('sync')"
        >
          <Zap class="h-3.5 w-3.5 text-sky-600 dark:text-sky-400" aria-hidden="true" />
          Jetzt synchronisieren
        </button>

        <button
          type="button"
          data-testid="bulk-scan-btn"
          :disabled="bulkBusy"
          class="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-3 py-1.5 text-xs font-semibold text-slate-800 hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-100 dark:hover:bg-slate-700"
          @click="executeBulkAction('scan_updates')"
        >
          <RefreshCw class="h-3.5 w-3.5 text-sky-600 dark:text-sky-400" aria-hidden="true" />
          Update-Scan
        </button>

        <div class="flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white p-0.5 pl-2 dark:border-slate-700 dark:bg-slate-800">
          <FolderKanban class="h-3.5 w-3.5 shrink-0 text-slate-400" aria-hidden="true" />
          <select
            v-model="bulkGroupId"
            data-testid="bulk-group-select"
            aria-label="Zielgruppe für Sammelzuweisung"
            class="bg-transparent py-1 pr-2 text-xs font-medium text-slate-700 focus:outline-none dark:text-slate-200"
          >
            <option value="">Gruppe zuweisen…</option>
            <option v-for="g in groups" :key="g.id" :value="g.id">{{ g.name }}</option>
          </select>
          <button
            type="button"
            data-testid="bulk-assign-group-btn"
            :disabled="bulkBusy || !bulkGroupId"
            class="rounded-md bg-slate-900 px-2.5 py-1 text-xs font-semibold text-white hover:bg-slate-800 disabled:opacity-40 dark:bg-sky-600 dark:hover:bg-sky-500"
            @click="executeBulkAction('assign_group')"
          >
            Anwenden
          </button>
        </div>

        <button
          type="button"
          data-testid="bulk-notify-btn"
          :disabled="bulkBusy"
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-3 py-1.5 text-xs font-semibold text-white shadow-xs hover:bg-sky-700 disabled:opacity-50"
          @click="bulkNotifyOpen = true"
        >
          <MessageSquareShare class="h-3.5 w-3.5" aria-hidden="true" />
          Nachricht senden…
        </button>
      </div>
    </div>

    <div
      v-if="error"
      role="alert"
      class="rounded-xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-300"
    >
      {{ error }}
    </div>

    <!-- Devices Table or Empty State -->
    <div v-if="devices.length === 0 && !loading">
      <EmptyState
        :icon="Laptop"
        :title="hasActiveFilters ? 'Keine passenden Geräte gefunden' : 'Noch keine Endpunkte registriert'"
        :description="
          hasActiveFilters
            ? 'Für die gewählte Kombination aus Suchbegriff und Filtern wurden keine Systeme gefunden.'
            : 'Erstellen Sie ein Enrollment-Token und führen Sie den PowerShell- oder Bash-Einzeiler auf Ihren Windows- oder Linux-Systemen aus.'
        "
      >
        <template #actions>
          <button
            v-if="hasActiveFilters"
            type="button"
            class="rounded-lg border border-slate-300 bg-white px-3.5 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
            @click="resetFilters"
          >
            Filter zurücksetzen
          </button>
          <RouterLink
            to="/enrollment"
            class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-3.5 py-2 text-xs font-semibold text-white hover:bg-sky-700"
          >
            <ShieldCheck class="h-4 w-4" aria-hidden="true" />
            Erstes Gerät anbinden
          </RouterLink>
        </template>
      </EmptyState>
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
              <th class="w-10 px-4 py-3.5">
                <input
                  data-testid="select-all-devices"
                  aria-label="Alle angezeigten Geräte auswählen"
                  type="checkbox"
                  :checked="allSelected"
                  :indeterminate.prop="someSelected"
                  class="h-4 w-4 cursor-pointer rounded border-slate-300 text-sky-600 focus:ring-sky-500 dark:border-slate-600 dark:bg-slate-950"
                  @change="toggleSelectAll"
                />
              </th>
              <th class="px-4 py-3.5">System / Hostname</th>
              <th class="px-4 py-3.5">Betriebssystem</th>
              <th class="px-4 py-3.5">Verbindung &amp; Freigabe</th>
              <th class="px-4 py-3.5">Zustand</th>
              <th class="px-4 py-3.5">Gruppen &amp; Tags</th>
              <th class="px-4 py-3.5">Letzter Kontakt</th>
              <th class="px-4 py-3.5 text-right">Aktionen</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-100 dark:divide-slate-800/70">
            <tr
              v-for="dev in devices"
              :key="dev.id"
              :data-testid="`device-row-${dev.id}`"
              class="transition-colors hover:bg-slate-50/80 dark:hover:bg-slate-800/50"
              :class="{ 'bg-sky-50/50 dark:bg-sky-500/10': selectedIds.includes(dev.id) }"
            >
              <td class="px-4 py-3.5">
                <input
                  :data-testid="`select-device-${dev.id}`"
                  :aria-label="`Gerät ${dev.hostname} auswählen`"
                  type="checkbox"
                  :checked="selectedIds.includes(dev.id)"
                  class="h-4 w-4 cursor-pointer rounded border-slate-300 text-sky-600 focus:ring-sky-500 dark:border-slate-600 dark:bg-slate-950"
                  @change="toggleSelectDevice(dev.id)"
                />
              </td>

              <td class="px-4 py-3.5">
                <RouterLink
                  :to="`/devices/${dev.id}`"
                  class="font-semibold text-slate-900 hover:text-sky-600 dark:text-slate-100 dark:hover:text-sky-400"
                >
                  {{ dev.hostname }}
                </RouterLink>
                <div class="mt-0.5 font-mono text-[11px] text-slate-500 dark:text-slate-400">
                  {{ (dev.ip_addresses && dev.ip_addresses[0]) || dev.domain || dev.arch }} · Agent v{{ dev.agent_version }}
                </div>
              </td>

              <td class="px-4 py-3.5">
                <OsBadge
                  :os-family="dev.os_family"
                  :os-name="dev.os_name"
                  :os-version="dev.os_version"
                />
              </td>

              <td class="px-4 py-3.5">
                <div class="flex flex-wrap items-center gap-1.5">
                  <StatusBadge :variant="dev.is_online ? 'online' : 'offline'" />
                  <StatusBadge :variant="dev.status" />
                  <span
                    v-if="dev.center_running === true"
                    :data-testid="`center-active-badge-${dev.id}`"
                    title="NS8 Software Center läuft aktiv in einer Benutzersitzung"
                    class="inline-flex items-center gap-1 rounded-full bg-cyan-50 px-2 py-0.5 text-[11px] font-medium text-cyan-800 ring-1 ring-inset ring-cyan-600/20 dark:bg-cyan-500/15 dark:text-cyan-300 dark:ring-cyan-400/30"
                  >
                    <MonitorCheck class="h-3 w-3" aria-hidden="true" />
                    Software Center aktiv
                  </span>
                </div>
              </td>

              <td class="px-4 py-3.5">
                <div class="flex flex-wrap items-center gap-1.5">
                  <StatusBadge v-if="dev.reboot_required" variant="reboot" />
                  <span
                    v-if="dev.pending_updates_count && dev.pending_updates_count > 0"
                    class="rounded-full bg-sky-50 px-2 py-0.5 text-xs font-medium text-sky-700 ring-1 ring-inset ring-sky-600/20 dark:bg-sky-500/15 dark:text-sky-300 dark:ring-sky-400/30"
                  >
                    {{ dev.pending_updates_count }} Updates offen
                  </span>
                  <span
                    v-if="!dev.reboot_required && !dev.pending_updates_count"
                    class="text-xs text-slate-400 dark:text-slate-500"
                  >
                    Unauffällig
                  </span>
                </div>
              </td>

              <td class="px-4 py-3.5">
                <div class="flex flex-wrap gap-1">
                  <span
                    v-for="grp in dev.groups || []"
                    :key="grp.id"
                    class="rounded bg-slate-100 px-2 py-0.5 text-xs font-medium text-slate-700 dark:bg-slate-800 dark:text-slate-300"
                  >
                    {{ grp.name }}
                  </span>
                  <span
                    v-for="tag in dev.tags || []"
                    :key="tag"
                    class="rounded bg-cyan-50 px-2 py-0.5 text-[11px] font-medium text-cyan-800 dark:bg-cyan-500/15 dark:text-cyan-300"
                  >
                    #{{ tag }}
                  </span>
                  <span
                    v-if="(!dev.groups || dev.groups.length === 0) && (!dev.tags || dev.tags.length === 0)"
                    class="text-xs text-slate-400"
                  >
                    —
                  </span>
                </div>
              </td>

              <td class="px-4 py-3.5 text-xs text-slate-500 dark:text-slate-400">
                {{ formatDateTime(dev.last_seen_at) }}
              </td>

              <td class="px-4 py-3.5 text-right">
                <div class="inline-flex items-center gap-1.5">
                  <button
                    v-if="dev.status === 'pending' || dev.status === 'revoked'"
                    type="button"
                    :data-testid="`approve-device-${dev.id}`"
                    class="inline-flex items-center gap-1 rounded-md bg-emerald-600 px-2.5 py-1 text-xs font-semibold text-white hover:bg-emerald-700"
                    @click="handleApprove(dev)"
                  >
                    <CheckCircle class="h-3.5 w-3.5" aria-hidden="true" />
                    Freigeben
                  </button>

                  <button
                    v-if="dev.status === 'approved'"
                    type="button"
                    :data-testid="`sync-device-${dev.id}`"
                    aria-label="Inventar & Richtlinien sofort synchronisieren"
                    title="Inventar & Richtlinien sofort synchronisieren"
                    class="inline-flex items-center gap-1 rounded-md border border-slate-200 px-2 py-1 text-xs font-medium text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
                    @click="handleSyncDevice(dev)"
                  >
                    <Zap class="h-3.5 w-3.5 text-sky-600 dark:text-sky-400" aria-hidden="true" />
                    Sync
                  </button>

                  <button
                    v-if="dev.status === 'approved'"
                    type="button"
                    aria-label="OS-Update-Scan anfordern"
                    title="OS-Update-Scan anfordern"
                    class="inline-flex items-center gap-1 rounded-md border border-slate-200 px-2 py-1 text-xs font-medium text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
                    @click="handleScanUpdates(dev)"
                  >
                    <RefreshCw class="h-3.5 w-3.5" aria-hidden="true" />
                    Scan
                  </button>

                  <button
                    v-if="dev.status === 'approved'"
                    type="button"
                    :data-testid="`revoke-device-${dev.id}`"
                    :aria-label="`Gerät ${dev.hostname} sperren`"
                    title="Gerät sperren"
                    class="inline-flex items-center rounded-md border border-amber-200 bg-amber-50 p-1.5 text-amber-700 hover:bg-amber-100 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-400 dark:hover:bg-amber-500/20"
                    @click="requestRevoke(dev)"
                  >
                    <Ban class="h-3.5 w-3.5" />
                  </button>

                  <RouterLink
                    :to="`/devices/${dev.id}`"
                    :aria-label="`Details für ${dev.hostname}`"
                    title="Gerätedetails & Inventar"
                    class="inline-flex items-center rounded-md border border-slate-200 p-1.5 text-slate-600 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
                  >
                    <Eye class="h-3.5 w-3.5" />
                  </RouterLink>

                  <button
                    type="button"
                    :data-testid="`delete-device-${dev.id}`"
                    :aria-label="`Gerät ${dev.hostname} löschen`"
                    title="Gerät löschen"
                    class="inline-flex items-center rounded-md border border-rose-200 bg-rose-50 p-1.5 text-rose-600 hover:bg-rose-100 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-400 dark:hover:bg-rose-500/20"
                    @click="requestDelete(dev)"
                  >
                    <Trash2 class="h-3.5 w-3.5" />
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <DeviceNotifyModal
      :open="bulkNotifyOpen"
      :target-label="`${selectedIds.length} ausgewählte System(e)`"
      :loading="bulkBusy"
      @cancel="bulkNotifyOpen = false"
      @submit="handleBulkNotifySubmit"
    />

    <ConfirmDialog
      :open="Boolean(confirmTarget)"
      :variant="confirmTarget?.action === 'revoke' ? 'warning' : 'danger'"
      :title="
        confirmTarget?.action === 'revoke'
          ? `Gerät „${confirmTarget?.device.hostname}“ sperren?`
          : `Gerät „${confirmTarget?.device.hostname}“ endgültig löschen?`
      "
      :message="
        confirmTarget?.action === 'revoke'
          ? 'Dem Endpunkt wird sofort der API- und WebSocket-Zugriff entzogen. Sie können das Gerät später jederzeit wieder freigeben.'
          : 'Das Gerät inklusive Inventar- und Auftragszuordnung wird dauerhaft aus der Verwaltungskonsole entfernt.'
      "
      :confirm-label="confirmTarget?.action === 'revoke' ? 'Gerät sperren' : 'Endgültig löschen'"
      :loading="confirmBusy"
      @cancel="confirmTarget = null"
      @confirm="executeConfirmedAction"
    />
  </div>
</template>
