<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Edit2, FolderKanban, Lock, MonitorCog, Plus, Trash2, X } from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type { DeviceGroup, RebootPolicy } from '@/api/types'
import { useToast } from '@/composables/useToast'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import EmptyState from '@/components/EmptyState.vue'
import { formatRebootPolicy } from '@/utils/format'

const toast = useToast()
const groups = ref<DeviceGroup[]>([])
const loading = ref(false)

const modalOpen = ref(false)
const editingGroup = ref<DeviceGroup | null>(null)
const name = ref('')
const description = ref('')
const rebootPolicy = ref<RebootPolicy>('never')
const gracePeriodMinutes = ref(240)
const enforceTray = ref(true)
const allowSelfService = ref(true)
const allowUserOsUpdates = ref(true)
const silentNotifications = ref(false)
const saving = ref(false)

const pendingDeleteGroup = ref<DeviceGroup | null>(null)
const deleting = ref(false)

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && modalOpen.value && !saving.value) {
    modalOpen.value = false
  }
}

watch(modalOpen, (isOpen) => {
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

async function loadGroups() {
  loading.value = true
  try {
    groups.value = await adminApi.listGroups()
  } catch (err) {
    toast.error('Gruppen konnten nicht geladen werden', err instanceof Error ? err.message : undefined)
  } finally {
    loading.value = false
  }
}

function openCreate() {
  editingGroup.value = null
  name.value = ''
  description.value = ''
  rebootPolicy.value = 'never'
  gracePeriodMinutes.value = 240
  enforceTray.value = true
  allowSelfService.value = true
  allowUserOsUpdates.value = true
  silentNotifications.value = false
  modalOpen.value = true
}

function openEdit(grp: DeviceGroup) {
  editingGroup.value = grp
  name.value = grp.name
  description.value = grp.description || ''
  rebootPolicy.value = grp.reboot_policy
  gracePeriodMinutes.value = grp.grace_period_minutes || 240
  enforceTray.value = grp.enforce_tray ?? true
  allowSelfService.value = grp.allow_self_service ?? true
  allowUserOsUpdates.value = grp.allow_user_os_updates ?? true
  silentNotifications.value = grp.silent_notifications ?? false
  modalOpen.value = true
}

async function handleSave() {
  if (!name.value.trim()) {
    toast.error('Gruppenname fehlt')
    return
  }
  saving.value = true
  try {
    const payload = {
      name: name.value.trim(),
      description: description.value.trim(),
      reboot_policy: rebootPolicy.value,
      grace_period_minutes: Number(gracePeriodMinutes.value) || 240,
      enforce_tray: enforceTray.value,
      allow_self_service: allowSelfService.value,
      allow_user_os_updates: allowUserOsUpdates.value,
      silent_notifications: silentNotifications.value,
    }
    if (editingGroup.value) {
      await adminApi.updateGroup(editingGroup.value.id, payload)
      toast.success('Gruppe aktualisiert')
    } else {
      await adminApi.createGroup(payload)
      toast.success('Gerätegruppe angelegt')
    }
    modalOpen.value = false
    await loadGroups()
  } catch (err) {
    toast.error('Speichern fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    saving.value = false
  }
}

async function confirmDelete() {
  if (!pendingDeleteGroup.value) return
  deleting.value = true
  const grp = pendingDeleteGroup.value
  try {
    await adminApi.deleteGroup(grp.id)
    groups.value = groups.value.filter((g) => g.id !== grp.id)
    toast.success('Gruppe gelöscht', `${grp.name} wurde entfernt.`)
    pendingDeleteGroup.value = null
  } catch (err) {
    toast.error('Löschen fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    deleting.value = false
  }
}

onMounted(() => {
  loadGroups()
})
</script>

<template>
  <div class="space-y-6">
    <div class="flex flex-col justify-between gap-4 sm:flex-row sm:items-center">
      <div>
        <h1 class="text-2xl font-bold tracking-tight text-slate-900 dark:text-slate-50">
          Gerätegruppen &amp; Ringe
        </h1>
        <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
          Strukturieren Sie Server, Workstations und Pilot-Ringe für gestufte Rollouts und Neustart-Richtlinien
        </p>
      </div>
      <button
        type="button"
        class="inline-flex items-center gap-1.5 self-start rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white shadow-sm hover:bg-sky-700"
        @click="openCreate"
      >
        <Plus class="h-4 w-4" aria-hidden="true" />
        Neue Gruppe anlegen
      </button>
    </div>

    <div v-if="groups.length === 0 && !loading">
      <EmptyState
        :icon="FolderKanban"
        title="Noch keine Gerätegruppen vorhanden"
        description="Legen Sie logische Gruppen wie „Pilot-Ring“, „Büro-Workstations“ oder „Produktiv-Server“ an, um Rollouts und Wartungsfenster gezielt zu steuern."
      >
        <template #actions>
          <button
            type="button"
            class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white hover:bg-sky-700"
            @click="openCreate"
          >
            <Plus class="h-4 w-4" aria-hidden="true" />
            Erste Gerätegruppe anlegen
          </button>
        </template>
      </EmptyState>
    </div>

    <div v-else class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
      <div
        v-for="grp in groups"
        :key="grp.id"
        class="flex flex-col justify-between rounded-xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900"
      >
        <div>
          <div class="flex items-start justify-between">
            <div class="flex items-center gap-2.5">
              <div class="rounded-lg bg-sky-50 p-2 text-sky-600 dark:bg-sky-500/15 dark:text-sky-400">
                <FolderKanban class="h-5 w-5" aria-hidden="true" />
              </div>
              <div>
                <h3 class="text-base font-bold text-slate-900 dark:text-slate-100">{{ grp.name }}</h3>
                <span class="text-xs text-slate-500 dark:text-slate-400">{{ grp.device_count ?? 0 }} zugeordnete Geräte</span>
              </div>
            </div>
            <div class="flex items-center gap-1">
              <button
                type="button"
                :aria-label="`Gruppe ${grp.name} bearbeiten`"
                title="Gruppe bearbeiten"
                class="rounded p-1.5 text-slate-500 hover:bg-slate-100 hover:text-slate-800 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100"
                @click="openEdit(grp)"
              >
                <Edit2 class="h-4 w-4" />
              </button>
              <button
                type="button"
                :aria-label="`Gruppe ${grp.name} löschen`"
                title="Gruppe löschen"
                class="rounded p-1.5 text-rose-500 hover:bg-rose-50 dark:text-rose-400 dark:hover:bg-rose-500/15"
                @click="pendingDeleteGroup = grp"
              >
                <Trash2 class="h-4 w-4" />
              </button>
            </div>
          </div>
          <p class="mt-3 text-xs text-slate-600 dark:text-slate-300">
            {{ grp.description || 'Keine Beschreibung hinterlegt.' }}
          </p>
        </div>

        <div class="mt-4 space-y-2.5 border-t border-slate-100 pt-3 dark:border-slate-800">
          <div>
            <div class="text-[11px] font-semibold uppercase tracking-wider text-slate-400 dark:text-slate-500">
              Neustart-Richtlinie
            </div>
            <div class="mt-1 text-xs font-semibold text-slate-800 dark:text-slate-200">
              {{ formatRebootPolicy(grp.reboot_policy, grp.grace_period_minutes) }}
            </div>
          </div>

          <div class="flex flex-wrap gap-1.5 pt-0.5">
            <span
              v-if="grp.enforce_tray ?? true"
              class="inline-flex items-center gap-1 rounded bg-emerald-50 px-2 py-0.5 text-[11px] font-medium text-emerald-700 dark:bg-emerald-500/15 dark:text-emerald-300"
            >
              <Lock class="h-3 w-3" aria-hidden="true" />
              Tray geschützt
            </span>
            <span
              v-if="!(grp.allow_self_service ?? true)"
              class="rounded bg-amber-50 px-2 py-0.5 text-[11px] font-semibold text-amber-800 dark:bg-amber-500/15 dark:text-amber-300"
            >
              Managed-Kiosk-Modus
            </span>
            <span
              v-else
              class="rounded bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-slate-700 dark:bg-slate-800 dark:text-slate-300"
            >
              Self-Service aktiv
            </span>
            <span
              v-if="grp.silent_notifications"
              class="rounded bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-slate-600 dark:bg-slate-800 dark:text-slate-400"
            >
              Stiller Modus
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- Modal -->
    <div
      v-if="modalOpen"
      role="dialog"
      aria-modal="true"
      :aria-label="editingGroup ? 'Gerätegruppe bearbeiten' : 'Neue Gerätegruppe anlegen'"
      class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/65 p-4 backdrop-blur-sm"
      @click.self="modalOpen = false"
    >
      <div
        class="w-full max-w-lg overflow-hidden rounded-xl border border-slate-200 bg-white shadow-2xl dark:border-slate-800 dark:bg-slate-900"
      >
        <div class="flex items-center justify-between border-b border-slate-800 bg-slate-900 px-6 py-4 text-white dark:bg-slate-950">
          <h3 class="text-base font-semibold">
            {{ editingGroup ? 'Gerätegruppe bearbeiten' : 'Neue Gerätegruppe anlegen' }}
          </h3>
          <button
            type="button"
            aria-label="Dialog schließen"
            class="rounded-lg p-1 text-slate-400 hover:bg-slate-800 hover:text-white"
            @click="modalOpen = false"
          >
            <X class="h-5 w-5" />
          </button>
        </div>

        <form class="space-y-4 p-6" @submit.prevent="handleSave">
          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Gruppenname *</label>
            <input
              v-model="name"
              type="text"
              placeholder="z. B. Produktiv-Server oder Pilot-Ring"
              class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
            />
          </div>

          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Beschreibung</label>
            <textarea
              v-model="description"
              rows="2"
              placeholder="Zweck oder enthaltene Systeme..."
              class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
            />
          </div>

          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Neustart-Richtlinie nach Updates</label>
            <select
              v-model="rebootPolicy"
              class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
            >
              <option value="never">Kein automatischer Neustart (Empfohlen für Server)</option>
              <option value="maintenance_window">Automatischer Neustart im Wartungsfenster</option>
              <option value="grace_period">Benutzergesteuert mit Karenzzeit (Workstations)</option>
            </select>
          </div>

          <div v-if="rebootPolicy === 'grace_period'">
            <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Karenzzeit bis Deadline (Minuten)</label>
            <input
              v-model.number="gracePeriodMinutes"
              type="number"
              min="15"
              max="10080"
              class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
            />
          </div>

          <div
            class="space-y-2.5 rounded-lg border border-slate-200 bg-slate-50 p-3.5 dark:border-slate-800 dark:bg-slate-950/60"
          >
            <div class="flex items-center gap-1.5 text-xs font-semibold text-slate-700 dark:text-slate-200">
              <MonitorCog class="h-4 w-4 text-cyan-600 dark:text-cyan-400" aria-hidden="true" />
              <span>Desktop-Client-Steuerung („NS8 Software Center“)</span>
            </div>

            <label class="flex cursor-pointer items-center gap-2.5 text-xs text-slate-800 dark:text-slate-200">
              <input
                v-model="enforceTray"
                data-testid="group-enforce-tray"
                type="checkbox"
                class="h-4 w-4 rounded border-slate-300 text-sky-600"
              />
              <span>Tray-Icon erzwingen &amp; Beenden sperren</span>
            </label>

            <label class="flex cursor-pointer items-center gap-2.5 text-xs text-slate-800 dark:text-slate-200">
              <input
                v-model="allowSelfService"
                data-testid="group-allow-self-service"
                type="checkbox"
                class="h-4 w-4 rounded border-slate-300 text-sky-600"
              />
              <span>
                Self-Service-Installationen erlauben
                <span v-if="!allowSelfService" class="ml-1 font-semibold text-amber-700 dark:text-amber-300">
                  (Managed-Kiosk-Modus)
                </span>
              </span>
            </label>

            <label class="flex cursor-pointer items-center gap-2.5 text-xs text-slate-800 dark:text-slate-200">
              <input
                v-model="allowUserOsUpdates"
                data-testid="group-allow-os-updates"
                type="checkbox"
                class="h-4 w-4 rounded border-slate-300 text-sky-600"
              />
              <span>Manuelle OS-Updates durch Nutzer erlauben</span>
            </label>

            <label class="flex cursor-pointer items-center gap-2.5 text-xs text-slate-800 dark:text-slate-200">
              <input
                v-model="silentNotifications"
                data-testid="group-silent-notifications"
                type="checkbox"
                class="h-4 w-4 rounded border-slate-300 text-sky-600"
              />
              <span>Stiller Modus — nur kritische Reboot-Warnungen</span>
            </label>
          </div>

          <div class="flex justify-end gap-2 border-t border-slate-200 pt-4 dark:border-slate-800">
            <button
              type="button"
              class="rounded-lg border border-slate-300 bg-white px-4 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
              @click="modalOpen = false"
            >
              Abbrechen
            </button>
            <button
              type="submit"
              :disabled="saving"
              class="rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white hover:bg-sky-700 disabled:opacity-60"
            >
              {{ saving ? 'Speichere…' : 'Speichern' }}
            </button>
          </div>
        </form>
      </div>
    </div>

    <ConfirmDialog
      :open="Boolean(pendingDeleteGroup)"
      variant="danger"
      :title="`Gerätegruppe „${pendingDeleteGroup?.name}“ löschen?`"
      message="Zugeordnete Geräte bleiben in der Flotte erhalten, verlieren jedoch die Gruppenzuordnung und zugehörige Gruppen-Wartungsfenster."
      confirm-label="Gruppe löschen"
      :loading="deleting"
      @cancel="pendingDeleteGroup = null"
      @confirm="confirmDelete"
    />
  </div>
</template>
