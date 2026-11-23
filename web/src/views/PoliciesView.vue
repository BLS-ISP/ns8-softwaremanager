<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { RouterLink } from 'vue-router'
import {
  BellOff,
  FolderKanban,
  Lock,
  MonitorCog,
  RotateCcw,
  Save,
  Sparkles,
  UploadCloud,
} from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type { AgentArtifactItem, DeviceGroup, OsFamily, RebootPolicy } from '@/api/types'
import { useToast } from '@/composables/useToast'
import EmptyState from '@/components/EmptyState.vue'
import { formatBytes } from '@/utils/format'

const toast = useToast()

const groups = ref<DeviceGroup[]>([])
const artifacts = ref<AgentArtifactItem[]>([])
const loading = ref(false)
const savingGroupId = ref<string | null>(null)

function normalizeGroupPolicies(grp: DeviceGroup): DeviceGroup {
  return {
    ...grp,
    enforce_tray: grp.enforce_tray ?? true,
    allow_self_service: grp.allow_self_service ?? true,
    allow_user_os_updates: grp.allow_user_os_updates ?? true,
    silent_notifications: grp.silent_notifications ?? false,
  }
}

// Agent Self-Update Form
const agentVersion = ref('1.0.5')
const agentOsFamily = ref<OsFamily>('windows')
const agentDownloadUrl = ref('/dl/agents/windows/swm-agent.exe')
const agentSha256 = ref('')
const targetGroupIds = ref<string[]>([])
const respectMw = ref(true)
const rollingOut = ref(false)

const matchingBundledAgent = computed(() =>
  artifacts.value.find(
    (a) => a.role === 'agent' && a.os_family === agentOsFamily.value && a.available && a.sha256,
  ),
)

function applyBundledAgentArtifact() {
  const found = matchingBundledAgent.value
  if (!found) return
  agentDownloadUrl.value = found.download_path
  agentSha256.value = found.sha256
  toast.info('Prüfsumme übernommen', `${found.filename} (SHA-256 automatisch eingetragen)`)
}

async function loadGroups() {
  loading.value = true
  try {
    const [grpList, artMeta] = await Promise.all([
      adminApi.listGroups(),
      adminApi.getAgentArtifacts().catch(() => ({
        server_url: '',
        version: '1.0.5',
        artifacts: [],
      })),
    ])
    groups.value = grpList.map(normalizeGroupPolicies)
    artifacts.value = artMeta.artifacts || []
    if (matchingBundledAgent.value && !agentSha256.value) {
      agentSha256.value = matchingBundledAgent.value.sha256
      agentDownloadUrl.value = matchingBundledAgent.value.download_path
    }
  } catch (err) {
    toast.error('Richtlinien konnten nicht geladen werden', err instanceof Error ? err.message : undefined)
  } finally {
    loading.value = false
  }
}

async function savePolicy(group: DeviceGroup) {
  savingGroupId.value = group.id
  try {
    const updated = await adminApi.updateGroup(group.id, {
      reboot_policy: group.reboot_policy as RebootPolicy,
      grace_period_minutes: Number(group.grace_period_minutes) || 240,
      enforce_tray: group.enforce_tray ?? true,
      allow_self_service: group.allow_self_service ?? true,
      allow_user_os_updates: group.allow_user_os_updates ?? true,
      silent_notifications: group.silent_notifications ?? false,
    })
    const idx = groups.value.findIndex((g) => g.id === group.id)
    if (idx !== -1 && updated && typeof updated === 'object') {
      groups.value[idx] = normalizeGroupPolicies({ ...group, ...updated })
    }
    toast.success('Gruppen-Richtlinien gespeichert', group.name)
  } catch (err) {
    toast.error('Speichern fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    savingGroupId.value = null
  }
}

function onOsChange() {
  agentDownloadUrl.value =
    agentOsFamily.value === 'windows'
      ? '/dl/agents/windows/swm-agent.exe'
      : '/dl/agents/linux-amd64/swm-agent'
  if (matchingBundledAgent.value) {
    agentDownloadUrl.value = matchingBundledAgent.value.download_path
    agentSha256.value = matchingBundledAgent.value.sha256
  } else {
    agentSha256.value = ''
  }
}

async function handleSelfUpdate() {
  if (!agentVersion.value.trim()) {
    toast.error('Bitte Ziel-Version angeben.')
    return
  }
  rollingOut.value = true
  try {
    const res = await adminApi.triggerSelfUpdate({
      version: agentVersion.value.trim(),
      os_family: agentOsFamily.value,
      sha256: agentSha256.value.trim(),
      download_url: agentDownloadUrl.value.trim() || null,
      target_group_ids: targetGroupIds.value,
      target_device_ids: [],
      respect_maintenance_window: respectMw.value,
    })
    toast.success(
      'Agent-Self-Update eingereiht',
      `${res.jobs_created ?? 0} Ed25519-signierte Aktualisierungsaufträge erstellt.`,
    )
  } catch (err) {
    toast.error('Self-Update-Rollout fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    rollingOut.value = false
  }
}

onMounted(() => {
  loadGroups()
})
</script>

<template>
  <div class="space-y-8">
    <div>
      <h1 class="text-2xl font-bold tracking-tight text-slate-900 dark:text-slate-50">
        Richtlinien, Desktop-Client &amp; Agent-Self-Update
      </h1>
      <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
        Steuern Sie Neustart-Verhalten, „NS8 Software Center“-Härtung pro Gerätegruppe sowie zentrale Agent-Updates
      </p>
    </div>

    <!-- Reboot & Desktop Client Policies per Group -->
    <div
      class="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900"
    >
      <div class="flex flex-wrap items-center justify-between gap-2 border-b border-slate-200 px-6 py-4 dark:border-slate-800">
        <div class="flex items-center gap-2">
          <RotateCcw class="h-4 w-4 text-sky-600 dark:text-sky-400" aria-hidden="true" />
          <h2 class="text-sm font-bold text-slate-900 dark:text-slate-100">
            Neustart- &amp; Desktop-Client-Steuerung pro Gerätegruppe
          </h2>
        </div>
        <span class="text-xs text-slate-500 dark:text-slate-400">
          Wirkt auf zugeordnete Windows- &amp; Linux-Clients bei Status-Synchronisation
        </span>
      </div>

      <div v-if="groups.length === 0 && !loading" class="p-6">
        <EmptyState
          :icon="FolderKanban"
          title="Noch keine Gerätegruppen vorhanden"
          description="Legen Sie mindestens eine Gerätegruppe an, um individuelle Neustart-Richtlinien und Desktop-Client-Regeln zu vergeben."
        >
          <template #actions>
            <RouterLink
              to="/groups"
              class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white hover:bg-sky-700"
            >
              <FolderKanban class="h-4 w-4" aria-hidden="true" />
              Gerätegruppen verwalten
            </RouterLink>
          </template>
        </EmptyState>
      </div>

      <div v-else class="divide-y divide-slate-200 dark:divide-slate-800">
        <div
          v-for="grp in groups"
          :key="grp.id"
          :data-testid="`policy-group-${grp.id}`"
          class="space-y-4 p-5 sm:p-6"
        >
          <div class="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
            <div>
              <div class="flex flex-wrap items-center gap-2">
                <span class="text-sm font-bold text-slate-900 dark:text-slate-100">{{ grp.name }}</span>
                <span
                  v-if="grp.enforce_tray"
                  class="inline-flex items-center gap-1 rounded-full bg-emerald-50 px-2 py-0.5 text-[11px] font-medium text-emerald-700 ring-1 ring-inset ring-emerald-600/20 dark:bg-emerald-500/15 dark:text-emerald-300 dark:ring-emerald-400/30"
                >
                  <Lock class="h-3 w-3" aria-hidden="true" />
                  Tray geschützt
                </span>
                <span
                  v-if="!grp.allow_self_service"
                  class="inline-flex items-center gap-1 rounded-full bg-amber-50 px-2 py-0.5 text-[11px] font-semibold text-amber-800 ring-1 ring-inset ring-amber-600/25 dark:bg-amber-500/15 dark:text-amber-300 dark:ring-amber-400/30"
                >
                  Managed-Kiosk-Modus
                </span>
                <span
                  v-if="grp.silent_notifications"
                  class="inline-flex items-center gap-1 rounded-full bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-slate-700 ring-1 ring-inset ring-slate-500/20 dark:bg-slate-800 dark:text-slate-300 dark:ring-slate-600/40"
                >
                  <BellOff class="h-3 w-3" aria-hidden="true" />
                  Stiller Modus
                </span>
              </div>
              <div class="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
                {{ grp.description || 'Gerätegruppe' }}
                <span v-if="grp.device_count !== undefined"> · {{ grp.device_count }} Geräte</span>
              </div>
            </div>

            <div class="flex flex-wrap items-center gap-3">
              <select
                v-model="grp.reboot_policy"
                :aria-label="`Neustart-Richtlinie für ${grp.name}`"
                class="rounded-lg border border-slate-300 bg-white px-3 py-2 text-xs font-medium text-slate-800 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-200"
              >
                <option value="never">Kein automatischer Neustart (Server-Schutz)</option>
                <option value="maintenance_window">Automatisch im Wartungsfenster</option>
                <option value="grace_period">Benutzer-Karenzzeit &amp; Countdown</option>
              </select>

              <div v-if="grp.reboot_policy === 'grace_period'" class="flex items-center gap-1.5 text-xs">
                <span class="text-slate-500 dark:text-slate-400">Deadline:</span>
                <input
                  v-model.number="grp.grace_period_minutes"
                  type="number"
                  min="15"
                  class="w-24 rounded-lg border border-slate-300 px-2.5 py-1.5 text-xs dark:border-slate-700 dark:bg-slate-950"
                />
                <span class="text-slate-500 dark:text-slate-400">Min.</span>
              </div>

              <button
                type="button"
                :data-testid="`save-policy-${grp.id}`"
                :disabled="savingGroupId === grp.id"
                class="inline-flex items-center gap-1.5 rounded-lg bg-slate-900 px-3.5 py-2 text-xs font-semibold text-white hover:bg-slate-800 disabled:opacity-60 dark:bg-sky-600 dark:hover:bg-sky-500"
                @click="savePolicy(grp)"
              >
                <Save class="h-3.5 w-3.5" aria-hidden="true" />
                {{ savingGroupId === grp.id ? 'Speichere…' : 'Speichern' }}
              </button>
            </div>
          </div>

          <!-- Desktop-Client-Steuerung („NS8 Software Center“) -->
          <div
            class="rounded-xl border border-slate-200/90 bg-slate-50/70 p-4 dark:border-slate-800 dark:bg-slate-950/60"
          >
            <div class="mb-3 flex items-center gap-2 text-xs font-semibold text-slate-700 dark:text-slate-200">
              <MonitorCog class="h-4 w-4 text-cyan-600 dark:text-cyan-400" aria-hidden="true" />
              <span>Desktop-Client-Steuerung („NS8 Software Center“)</span>
            </div>

            <div class="grid grid-cols-1 gap-2.5 md:grid-cols-2">
              <label
                class="flex cursor-pointer items-start gap-3 rounded-lg border border-slate-200 bg-white p-3 transition hover:border-sky-300 dark:border-slate-800 dark:bg-slate-900 dark:hover:border-sky-500/40"
              >
                <input
                  v-model="grp.enforce_tray"
                  :data-testid="`policy-enforce-tray-${grp.id}`"
                  type="checkbox"
                  class="mt-0.5 h-4 w-4 rounded border-slate-300 text-sky-600 focus:ring-sky-500 dark:border-slate-600 dark:bg-slate-950"
                />
                <span class="min-w-0">
                  <span class="block text-xs font-semibold text-slate-900 dark:text-slate-100">
                    Tray-Icon erzwingen &amp; Beenden sperren
                  </span>
                  <span class="mt-0.5 block text-[11px] text-slate-500 dark:text-slate-400">
                    Deaktiviert „Beenden“ im System-Tray und hält den Client per Session-Watchdog aktiv
                  </span>
                </span>
              </label>

              <label
                class="flex cursor-pointer items-start gap-3 rounded-lg border border-slate-200 bg-white p-3 transition hover:border-sky-300 dark:border-slate-800 dark:bg-slate-900 dark:hover:border-sky-500/40"
              >
                <input
                  v-model="grp.allow_self_service"
                  :data-testid="`policy-allow-self-service-${grp.id}`"
                  type="checkbox"
                  class="mt-0.5 h-4 w-4 rounded border-slate-300 text-sky-600 focus:ring-sky-500 dark:border-slate-600 dark:bg-slate-950"
                />
                <span class="min-w-0">
                  <span class="flex flex-wrap items-center gap-1.5 text-xs font-semibold text-slate-900 dark:text-slate-100">
                    <span>Self-Service-Installationen erlauben</span>
                    <span
                      v-if="!grp.allow_self_service"
                      class="rounded bg-amber-100 px-1.5 py-0.5 text-[10px] font-bold text-amber-800 dark:bg-amber-500/20 dark:text-amber-300"
                    >
                      Managed-Kiosk-Modus
                    </span>
                  </span>
                  <span class="mt-0.5 block text-[11px] text-slate-500 dark:text-slate-400">
                    {{
                      grp.allow_self_service
                        ? 'Benutzer dürfen freigegebene Katalog-Apps selbstständig installieren'
                        : 'Managed-Kiosk-Modus aktiv — nur zentrale Pflicht-Rollouts durch IT erlaubt'
                    }}
                  </span>
                </span>
              </label>

              <label
                class="flex cursor-pointer items-start gap-3 rounded-lg border border-slate-200 bg-white p-3 transition hover:border-sky-300 dark:border-slate-800 dark:bg-slate-900 dark:hover:border-sky-500/40"
              >
                <input
                  v-model="grp.allow_user_os_updates"
                  :data-testid="`policy-allow-os-updates-${grp.id}`"
                  type="checkbox"
                  class="mt-0.5 h-4 w-4 rounded border-slate-300 text-sky-600 focus:ring-sky-500 dark:border-slate-600 dark:bg-slate-950"
                />
                <span class="min-w-0">
                  <span class="block text-xs font-semibold text-slate-900 dark:text-slate-100">
                    Manuelle OS-Updates durch Nutzer erlauben
                  </span>
                  <span class="mt-0.5 block text-[11px] text-slate-500 dark:text-slate-400">
                    Erlaubt Endbenutzern, ausstehende System-Patches vor dem Wartungsfenster anzustoßen
                  </span>
                </span>
              </label>

              <label
                class="flex cursor-pointer items-start gap-3 rounded-lg border border-slate-200 bg-white p-3 transition hover:border-sky-300 dark:border-slate-800 dark:bg-slate-900 dark:hover:border-sky-500/40"
              >
                <input
                  v-model="grp.silent_notifications"
                  :data-testid="`policy-silent-notifications-${grp.id}`"
                  type="checkbox"
                  class="mt-0.5 h-4 w-4 rounded border-slate-300 text-sky-600 focus:ring-sky-500 dark:border-slate-600 dark:bg-slate-950"
                />
                <span class="min-w-0">
                  <span class="block text-xs font-semibold text-slate-900 dark:text-slate-100">
                    Stiller Modus — nur kritische Reboot-Warnungen
                  </span>
                  <span class="mt-0.5 block text-[11px] text-slate-500 dark:text-slate-400">
                    Unterdrückt Standard-Popups und zeigt nur dringende Admin-Hinweise &amp; Reboot-Deadlines
                  </span>
                </span>
              </label>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Agent Self-Update Section -->
    <div
      class="rounded-xl border border-slate-200 bg-white p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900"
    >
      <div class="flex items-center gap-2.5">
        <div class="rounded-lg bg-sky-50 p-2 text-sky-600 dark:bg-sky-500/15 dark:text-sky-400">
          <UploadCloud class="h-5 w-5" aria-hidden="true" />
        </div>
        <div>
          <h2 class="text-base font-bold text-slate-900 dark:text-slate-100">
            Zentrales Agent-Self-Update ausrollen
          </h2>
          <p class="text-xs text-slate-500 dark:text-slate-400">
            Erzeugt Ed25519-signierte Aktualisierungsaufträge mit SHA-256-Prüfung und atomarem Dienst-Austausch
          </p>
        </div>
      </div>

      <div
        v-if="matchingBundledAgent"
        class="mt-4 flex flex-col justify-between gap-2 rounded-lg border border-sky-200 bg-sky-50/70 px-4 py-3 text-xs sm:flex-row sm:items-center dark:border-sky-500/25 dark:bg-sky-500/10"
      >
        <div class="text-slate-700 dark:text-slate-200">
          Integriertes Server-Paket erkannt:
          <code class="font-mono font-semibold">{{ matchingBundledAgent.platform }}/{{ matchingBundledAgent.filename }}</code>
          ({{ formatBytes(matchingBundledAgent.size_bytes) }})
        </div>
        <button
          type="button"
          class="inline-flex shrink-0 items-center gap-1.5 rounded-md bg-sky-600 px-3 py-1.5 font-semibold text-white hover:bg-sky-700"
          @click="applyBundledAgentArtifact"
        >
          <Sparkles class="h-3.5 w-3.5" aria-hidden="true" />
          Pfad &amp; SHA-256 übernehmen
        </button>
      </div>

      <form class="mt-5 grid grid-cols-1 gap-4 sm:grid-cols-2" @submit.prevent="handleSelfUpdate">
        <div>
          <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Ziel-Plattform</label>
          <select
            v-model="agentOsFamily"
            class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
            @change="onOsChange"
          >
            <option value="windows">Windows (swm-agent.exe)</option>
            <option value="linux">Linux (swm-agent)</option>
          </select>
        </div>

        <div>
          <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Neue Agent-Version *</label>
          <input
            v-model="agentVersion"
            type="text"
            placeholder="1.0.1"
            class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-sm dark:border-slate-700 dark:bg-slate-950"
          />
        </div>

        <div>
          <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Download-Pfad / URL</label>
          <input
            v-model="agentDownloadUrl"
            type="text"
            class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-xs dark:border-slate-700 dark:bg-slate-950"
          />
        </div>

        <div>
          <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Erwarteter SHA-256-Hash (Hex)</label>
          <input
            v-model="agentSha256"
            type="text"
            placeholder="64-stellige SHA-256 Prüfsumme des neuen Agent-Binaries"
            class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-xs dark:border-slate-700 dark:bg-slate-950"
          />
        </div>

        <div class="sm:col-span-2">
          <label class="mb-1.5 block text-xs font-semibold text-slate-700 dark:text-slate-300">
            Auf Gruppen beschränken (leer = alle freigegebenen Geräte dieser Plattform)
          </label>
          <div class="flex flex-wrap gap-3 rounded-lg border border-slate-200 bg-slate-50 p-3 dark:border-slate-800 dark:bg-slate-950/60">
            <label
              v-for="grp in groups"
              :key="grp.id"
              class="flex items-center gap-1.5 text-xs text-slate-800 dark:text-slate-200"
            >
              <input
                v-model="targetGroupIds"
                :value="grp.id"
                type="checkbox"
                class="h-3.5 w-3.5 rounded border-slate-300 text-sky-600"
              />
              <span>{{ grp.name }}</span>
            </label>
            <span v-if="groups.length === 0" class="text-xs text-slate-400">Alle Geräte</span>
          </div>
        </div>

        <div class="flex flex-wrap items-center justify-between gap-3 sm:col-span-2">
          <label class="flex items-center gap-2 text-xs font-medium text-slate-700 dark:text-slate-300">
            <input
              v-model="respectMw"
              type="checkbox"
              class="h-4 w-4 rounded border-slate-300 text-sky-600"
            />
            Wartungsfenster beachten
          </label>

          <button
            type="submit"
            :disabled="rollingOut"
            class="rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white hover:bg-sky-700 disabled:opacity-60"
          >
            {{ rollingOut ? 'Wird geplant…' : 'Self-Update jetzt ausrollen' }}
          </button>
        </div>
      </form>
    </div>
  </div>
</template>
