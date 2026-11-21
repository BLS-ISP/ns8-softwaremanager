<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Check, Copy, KeyRound, Monitor, Plus, Server, Trash2 } from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type { DeviceGroup, EnrollmentToken } from '@/api/types'
import { useToast } from '@/composables/useToast'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import EmptyState from '@/components/EmptyState.vue'
import EnrollmentTokenModal from '@/components/EnrollmentTokenModal.vue'
import { applyHeadlessToggle, formatDateTime } from '@/utils/format'

const toast = useToast()

const tokens = ref<EnrollmentToken[]>([])
const groups = ref<DeviceGroup[]>([])
const loading = ref(false)
const modalOpen = ref(false)
const headlessMode = ref(false)
const copiedKey = ref<string | null>(null)
const pendingRevokeToken = ref<EnrollmentToken | null>(null)
const revoking = ref(false)

async function loadAll() {
  loading.value = true
  try {
    const [tokList, grpList] = await Promise.all([
      adminApi.listEnrollmentTokens(),
      adminApi.listGroups(),
    ])
    tokens.value = tokList
    groups.value = grpList
  } catch (err) {
    toast.error('Enrollment-Tokens konnten nicht geladen werden', err instanceof Error ? err.message : undefined)
  } finally {
    loading.value = false
  }
}

async function copyCmd(cmd: string, key: string) {
  const transformed = applyHeadlessToggle(cmd, headlessMode.value)
  try {
    await navigator.clipboard.writeText(transformed)
    copiedKey.value = key
    setTimeout(() => {
      copiedKey.value = null
    }, 2000)
    toast.success('Installationsbefehl in Zwischenablage kopiert')
  } catch {
    // ignore in non-browser environment
  }
}

function requestRevoke(tok: EnrollmentToken) {
  pendingRevokeToken.value = tok
}

async function confirmRevoke() {
  if (!pendingRevokeToken.value) return
  revoking.value = true
  const target = pendingRevokeToken.value
  try {
    await adminApi.deleteEnrollmentToken(target.id)
    tokens.value = tokens.value.filter((t) => t.id !== target.id)
    toast.success('Enrollment-Token widerrufen', `„${target.name}“ kann nicht mehr zur Registrierung genutzt werden.`)
    pendingRevokeToken.value = null
  } catch (err) {
    toast.error('Widerrufen fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    revoking.value = false
  }
}

onMounted(() => {
  loadAll()
})
</script>

<template>
  <div class="space-y-6">
    <div class="flex flex-col justify-between gap-4 sm:flex-row sm:items-center">
      <div>
        <h1 class="text-2xl font-bold tracking-tight text-slate-900 dark:text-slate-50">
          Client-Onboarding &amp; Enrollment-Tokens
        </h1>
        <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
          Generieren Sie zeit- und nutzungsbegrenzte Registrierungstokens inkl. fertiger PowerShell- &amp; Bash-Einzeiler
        </p>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <!-- Global Headless Toggle for Command Preview -->
        <div
          class="inline-flex rounded-lg border border-slate-300 bg-white p-0.5 text-xs font-semibold dark:border-slate-700 dark:bg-slate-900"
        >
          <button
            type="button"
            class="inline-flex items-center gap-1.5 rounded-md px-2.5 py-1.5 transition"
            :class="
              !headlessMode
                ? 'bg-slate-900 text-white dark:bg-sky-600'
                : 'text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
            "
            @click="headlessMode = false"
          >
            <Monitor class="h-3.5 w-3.5" aria-hidden="true" />
            Desktop (+ Software Center)
          </button>
          <button
            type="button"
            class="inline-flex items-center gap-1.5 rounded-md px-2.5 py-1.5 transition"
            :class="
              headlessMode
                ? 'bg-slate-900 text-white dark:bg-sky-600'
                : 'text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
            "
            @click="headlessMode = true"
          >
            <Server class="h-3.5 w-3.5" aria-hidden="true" />
            Headless-Server
          </button>
        </div>

        <button
          type="button"
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white shadow-sm hover:bg-sky-700"
          @click="modalOpen = true"
        >
          <Plus class="h-4 w-4" aria-hidden="true" />
          Neues Token erstellen
        </button>
      </div>
    </div>

    <!-- Empty State -->
    <div v-if="tokens.length === 0 && !loading">
      <EmptyState
        :icon="KeyRound"
        title="Noch keine Enrollment-Tokens erstellt"
        description="Erstellen Sie ein Token, um Windows-Workstations, Windows-Server oder Linux-Systeme per 1-Zeiler-Bootstrap sicher an NS8-SoftwareManager anzubinden."
      >
        <template #actions>
          <button
            type="button"
            class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white hover:bg-sky-700"
            @click="modalOpen = true"
          >
            <Plus class="h-4 w-4" aria-hidden="true" />
            Erstes Enrollment-Token generieren
          </button>
        </template>
      </EmptyState>
    </div>

    <!-- Token Cards -->
    <div v-else class="space-y-4">
      <div
        v-for="tok in tokens"
        :key="tok.id"
        :data-testid="`enrollment-card-${tok.id}`"
        class="rounded-xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900"
      >
        <div class="flex flex-col justify-between gap-3 sm:flex-row sm:items-center">
          <div class="flex items-center gap-3">
            <div class="rounded-lg bg-sky-50 p-2 text-sky-600 dark:bg-sky-500/15 dark:text-sky-400">
              <KeyRound class="h-5 w-5" aria-hidden="true" />
            </div>
            <div>
              <div class="flex flex-wrap items-center gap-2">
                <h3 class="text-sm font-bold text-slate-900 dark:text-slate-100">{{ tok.name }}</h3>
                <span
                  class="rounded-full px-2 py-0.5 text-[11px] font-semibold"
                  :class="
                    tok.auto_approve
                      ? 'bg-emerald-50 text-emerald-700 dark:bg-emerald-500/15 dark:text-emerald-300'
                      : 'bg-amber-50 text-amber-800 dark:bg-amber-500/15 dark:text-amber-300'
                  "
                >
                  {{ tok.auto_approve ? 'Auto-Approve' : 'Manuelle Admin-Freigabe' }}
                </span>
              </div>
              <p class="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
                Nutzungen: <strong class="text-slate-700 dark:text-slate-200">{{ tok.uses_count ?? 0 }}</strong> / {{ tok.max_uses ?? '∞' }} ·
                Gültig bis: {{ formatDateTime(tok.expires_at) }}
              </p>
            </div>
          </div>

          <button
            type="button"
            :data-testid="`revoke-token-${tok.id}`"
            :aria-label="`Enrollment-Token ${tok.name} widerrufen`"
            class="inline-flex items-center gap-1 self-start rounded-lg border border-rose-200 bg-rose-50 px-3 py-1.5 text-xs font-semibold text-rose-700 hover:bg-rose-100 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-400 dark:hover:bg-rose-500/20"
            @click="requestRevoke(tok)"
          >
            <Trash2 class="h-3.5 w-3.5" aria-hidden="true" />
            Widerrufen
          </button>
        </div>

        <div class="mt-4 grid grid-cols-1 gap-3 lg:grid-cols-2">
          <!-- PowerShell Command -->
          <div class="rounded-lg border border-slate-800 bg-slate-950 p-3">
            <div class="mb-1.5 flex items-center justify-between text-[11px]">
              <span class="font-semibold text-sky-400">Windows PowerShell (Admin)</span>
              <button
                type="button"
                aria-label="PowerShell-Befehl kopieren"
                class="inline-flex items-center gap-1 font-semibold text-slate-300 hover:text-white"
                @click="copyCmd(tok.install_ps1_cmd, `ps1-${tok.id}`)"
              >
                <Check v-if="copiedKey === `ps1-${tok.id}`" class="h-3.5 w-3.5 text-emerald-400" />
                <Copy v-else class="h-3.5 w-3.5" />
                {{ copiedKey === `ps1-${tok.id}` ? 'Kopiert' : 'Kopieren' }}
              </button>
            </div>
            <pre class="overflow-x-auto font-mono text-[11px] text-slate-200">{{ applyHeadlessToggle(tok.install_ps1_cmd, headlessMode) }}</pre>
          </div>

          <!-- Bash Command -->
          <div class="rounded-lg border border-slate-800 bg-slate-950 p-3">
            <div class="mb-1.5 flex items-center justify-between text-[11px]">
              <span class="font-semibold text-emerald-400">Linux Bash (root / sudo)</span>
              <button
                type="button"
                aria-label="Bash-Befehl kopieren"
                class="inline-flex items-center gap-1 font-semibold text-slate-300 hover:text-white"
                @click="copyCmd(tok.install_sh_cmd, `sh-${tok.id}`)"
              >
                <Check v-if="copiedKey === `sh-${tok.id}`" class="h-3.5 w-3.5 text-emerald-400" />
                <Copy v-else class="h-3.5 w-3.5" />
                {{ copiedKey === `sh-${tok.id}` ? 'Kopiert' : 'Kopieren' }}
              </button>
            </div>
            <pre class="overflow-x-auto font-mono text-[11px] text-slate-200">{{ applyHeadlessToggle(tok.install_sh_cmd, headlessMode) }}</pre>
          </div>
        </div>
      </div>
    </div>

    <EnrollmentTokenModal
      :open="modalOpen"
      :groups="groups"
      @close="modalOpen = false"
      @created="loadAll"
    />

    <ConfirmDialog
      :open="Boolean(pendingRevokeToken)"
      variant="danger"
      :title="`Enrollment-Token „${pendingRevokeToken?.name}“ widerrufen?`"
      message="Neue Clients können sich mit diesem Token ab sofort nicht mehr registrieren. Bereits registrierte und freigegebene Geräte bleiben unberührt."
      confirm-label="Token widerrufen"
      :loading="revoking"
      @cancel="pendingRevokeToken = null"
      @confirm="confirmRevoke"
    />
  </div>
</template>
