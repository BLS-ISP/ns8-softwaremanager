<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { RouterLink } from 'vue-router'
import { FolderKanban, RotateCcw, Save, UploadCloud } from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type { DeviceGroup, OsFamily, RebootPolicy } from '@/api/types'
import { useToast } from '@/composables/useToast'
import EmptyState from '@/components/EmptyState.vue'

const toast = useToast()

const groups = ref<DeviceGroup[]>([])
const loading = ref(false)

// Agent Self-Update Form
const agentVersion = ref('1.0.1')
const agentOsFamily = ref<OsFamily>('windows')
const agentDownloadUrl = ref('/dl/agents/windows/swm-agent.exe')
const agentSha256 = ref('')
const targetGroupIds = ref<string[]>([])
const respectMw = ref(true)
const rollingOut = ref(false)

async function loadGroups() {
  loading.value = true
  try {
    groups.value = await adminApi.listGroups()
  } catch (err) {
    toast.error('Richtlinien konnten nicht geladen werden', err instanceof Error ? err.message : undefined)
  } finally {
    loading.value = false
  }
}

async function savePolicy(group: DeviceGroup) {
  try {
    await adminApi.updateGroup(group.id, {
      reboot_policy: group.reboot_policy as RebootPolicy,
      grace_period_minutes: Number(group.grace_period_minutes) || 240,
    })
    toast.success('Neustart-Richtlinie gespeichert', group.name)
  } catch (err) {
    toast.error('Speichern fehlgeschlagen', err instanceof Error ? err.message : undefined)
  }
}

function onOsChange() {
  agentDownloadUrl.value =
    agentOsFamily.value === 'windows'
      ? '/dl/agents/windows/swm-agent.exe'
      : '/dl/agents/linux-amd64/swm-agent'
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
        Neustart-Richtlinien &amp; Agent-Self-Update
      </h1>
      <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
        Verhindern Sie ungeplante Server-Reboots und aktualisieren Sie den Rust-Hintergrundagenten zentral
      </p>
    </div>

    <!-- Reboot Policies per Group -->
    <div
      class="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900"
    >
      <div class="flex items-center gap-2 border-b border-slate-200 px-6 py-4 dark:border-slate-800">
        <RotateCcw class="h-4 w-4 text-sky-600 dark:text-sky-400" aria-hidden="true" />
        <h2 class="text-sm font-bold text-slate-900 dark:text-slate-100">
          Neustart-Steuerung pro Gerätegruppe
        </h2>
      </div>

      <div v-if="groups.length === 0 && !loading" class="p-6">
        <EmptyState
          :icon="FolderKanban"
          title="Noch keine Gerätegruppen vorhanden"
          description="Legen Sie mindestens eine Gerätegruppe an, um individuelle Neustart-Richtlinien und Benutzer-Karenzzeiten zu vergeben."
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

      <div v-else class="divide-y divide-slate-100 dark:divide-slate-800/70">
        <div
          v-for="grp in groups"
          :key="grp.id"
          class="flex flex-col gap-4 p-5 lg:flex-row lg:items-center lg:justify-between"
        >
          <div>
            <div class="text-sm font-bold text-slate-900 dark:text-slate-100">{{ grp.name }}</div>
            <div class="text-xs text-slate-500 dark:text-slate-400">{{ grp.description || 'Gerätegruppe' }}</div>
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
              class="inline-flex items-center gap-1.5 rounded-lg bg-slate-900 px-3.5 py-2 text-xs font-semibold text-white hover:bg-slate-800 dark:bg-sky-600 dark:hover:bg-sky-500"
              @click="savePolicy(grp)"
            >
              <Save class="h-3.5 w-3.5" aria-hidden="true" />
              Speichern
            </button>
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
