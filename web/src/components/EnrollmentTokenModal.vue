<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { Check, Copy, KeyRound, Monitor, Server, X } from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type { DeviceGroup, EnrollmentToken } from '@/api/types'
import { useToast } from '@/composables/useToast'
import { applyHeadlessToggle } from '@/utils/format'

const props = defineProps<{
  open: boolean
  groups: DeviceGroup[]
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'created', token: EnrollmentToken): void
}>()

const toast = useToast()

const name = ref('')
const expiresInHours = ref(72)
const maxUsesInput = ref<string>('50')
const autoApprove = ref(false)
const groupId = ref<string>('')
const saving = ref(false)
const error = ref<string | null>(null)
const createdToken = ref<EnrollmentToken | null>(null)
const headlessPreview = ref(false)
const copiedTarget = ref<'ps1' | 'sh' | 'token' | null>(null)

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.open && !saving.value) {
    emit('close')
  }
}

watch(
  () => props.open,
  (isOpen) => {
    if (typeof window !== 'undefined') {
      if (isOpen) {
        window.addEventListener('keydown', onKeyDown)
      } else {
        window.removeEventListener('keydown', onKeyDown)
      }
    }
    if (!isOpen) return
    name.value = ''
    expiresInHours.value = 72
    maxUsesInput.value = '50'
    autoApprove.value = false
    groupId.value = ''
    error.value = null
    createdToken.value = null
    headlessPreview.value = false
  },
  { immediate: true },
)

onBeforeUnmount(() => {
  if (typeof window !== 'undefined') {
    window.removeEventListener('keydown', onKeyDown)
  }
})

const ps1Preview = computed(() =>
  createdToken.value ? applyHeadlessToggle(createdToken.value.install_ps1_cmd, headlessPreview.value) : '',
)
const shPreview = computed(() =>
  createdToken.value ? applyHeadlessToggle(createdToken.value.install_sh_cmd, headlessPreview.value) : '',
)

async function handleCreate() {
  error.value = null
  if (!name.value.trim()) {
    error.value = 'Bitte vergeben Sie eine Bezeichnung für das Enrollment-Token.'
    return
  }
  saving.value = true
  try {
    const parsedMax = maxUsesInput.value.trim() ? Number(maxUsesInput.value) : null
    const token = await adminApi.createEnrollmentToken({
      name: name.value.trim(),
      expires_in_hours: Number(expiresInHours.value) || 72,
      max_uses: parsedMax && parsedMax > 0 ? parsedMax : null,
      auto_approve: autoApprove.value,
      group_id: groupId.value || null,
    })
    createdToken.value = token
    toast.success('Enrollment-Token erstellt', `${token.name} ist bereit für das Client-Onboarding.`)
    emit('created', token)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Fehler beim Erstellen des Enrollment-Tokens'
  } finally {
    saving.value = false
  }
}

async function copyText(text: string, target: 'ps1' | 'sh' | 'token') {
  try {
    await navigator.clipboard.writeText(text)
    copiedTarget.value = target
    toast.info('In Zwischenablage kopiert')
    setTimeout(() => {
      copiedTarget.value = null
    }, 2000)
  } catch {
    // ignore in headless env
  }
}
</script>

<template>
  <div
    v-if="open"
    role="dialog"
    aria-modal="true"
    aria-label="Enrollment-Token verwalten"
    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/65 p-4 backdrop-blur-sm"
    @click.self="emit('close')"
  >
    <div
      class="flex max-h-[92vh] w-full max-w-2xl flex-col overflow-hidden rounded-xl border border-slate-200 bg-white shadow-2xl dark:border-slate-800 dark:bg-slate-900"
    >
      <div class="flex items-center justify-between border-b border-slate-800 bg-slate-900 px-6 py-4 text-white dark:bg-slate-950">
        <div class="flex items-center gap-3">
          <div class="rounded-lg bg-sky-500/20 p-2 text-sky-400">
            <KeyRound class="h-5 w-5" aria-hidden="true" />
          </div>
          <div>
            <h3 class="text-base font-semibold">
              {{ createdToken ? 'Enrollment-Token bereit zum Rollout' : 'Neues Enrollment-Token generieren' }}
            </h3>
            <p class="text-xs text-slate-400">
              Sicheres Client-Onboarding für Windows-GPO/PowerShell und Linux-Bash
            </p>
          </div>
        </div>
        <button
          type="button"
          aria-label="Dialog schließen"
          class="rounded-lg p-1.5 text-slate-400 hover:bg-slate-800 hover:text-white"
          @click="emit('close')"
        >
          <X class="h-5 w-5" />
        </button>
      </div>

      <!-- Creation Form -->
      <form v-if="!createdToken" class="space-y-4 overflow-y-auto p-6" @submit.prevent="handleCreate">
        <div
          v-if="error"
          class="rounded-lg border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-300"
        >
          {{ error }}
        </div>

        <div>
          <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">
            Bezeichnung / Rollout-Zweck *
          </label>
          <input
            v-model="name"
            type="text"
            placeholder="z. B. Büro-Workstations GPO-Rollout"
            class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
          />
        </div>

        <div class="grid grid-cols-1 gap-4 sm:grid-cols-3">
          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">
              Gültigkeit (Stunden)
            </label>
            <input
              v-model.number="expiresInHours"
              type="number"
              min="1"
              max="8760"
              class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
            />
          </div>
          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">
              Max. Registrierungen
            </label>
            <input
              v-model="maxUsesInput"
              type="number"
              min="1"
              placeholder="Unbegrenzt"
              class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
            />
          </div>
          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">
              Zielgruppe vorbelegen
            </label>
            <select
              v-model="groupId"
              class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
            >
              <option value="">Keine Vorbelegung</option>
              <option v-for="g in groups" :key="g.id" :value="g.id">{{ g.name }}</option>
            </select>
          </div>
        </div>

        <label class="flex items-center gap-3 rounded-lg border border-slate-200 bg-slate-50 p-3.5 dark:border-slate-800 dark:bg-slate-950/60">
          <input
            v-model="autoApprove"
            type="checkbox"
            class="h-4 w-4 rounded border-slate-300 text-sky-600"
          />
          <div>
            <div class="text-sm font-medium text-slate-900 dark:text-slate-100">
              Neue Geräte automatisch freigeben (Auto-Approve)
            </div>
            <div class="text-xs text-slate-500 dark:text-slate-400">
              Wenn deaktiviert, landen neue Rechner zunächst im sicheren Status „Freigabe ausstehend“.
            </div>
          </div>
        </label>

        <div class="flex justify-end gap-2.5 border-t border-slate-200 pt-4 dark:border-slate-800">
          <button
            type="button"
            class="rounded-lg border border-slate-300 bg-white px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
            @click="emit('close')"
          >
            Abbrechen
          </button>
          <button
            type="submit"
            :disabled="saving"
            class="rounded-lg bg-sky-600 px-4 py-2 text-sm font-semibold text-white hover:bg-sky-700 disabled:opacity-60"
          >
            {{ saving ? 'Generiere…' : 'Token erstellen' }}
          </button>
        </div>
      </form>

      <!-- Result View with 1-Click Copy & Headless Toggle -->
      <div v-else class="space-y-4 overflow-y-auto p-6">
        <div class="rounded-lg border border-emerald-200 bg-emerald-50 p-3.5 text-xs text-emerald-800 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-300">
          Token <strong>{{ createdToken.name }}</strong> wurde erfolgreich erstellt. Kopieren Sie den passenden Einzeiler für Ihre Zielsysteme:
        </div>

        <!-- Mode Toggle: Desktop vs Headless -->
        <div class="flex flex-wrap items-center justify-between gap-2 rounded-xl border border-slate-200 bg-slate-50 p-3 dark:border-slate-800 dark:bg-slate-950/60">
          <span class="text-xs font-semibold text-slate-700 dark:text-slate-300">Installations-Profil wählen:</span>
          <div class="inline-flex rounded-lg bg-slate-200 p-0.5 text-xs font-semibold dark:bg-slate-800">
            <button
              type="button"
              class="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 transition"
              :class="!headlessPreview ? 'bg-white text-slate-900 shadow-sm dark:bg-slate-700 dark:text-white' : 'text-slate-600 dark:text-slate-400'"
              @click="headlessPreview = false"
            >
              <Monitor class="h-3.5 w-3.5" aria-hidden="true" />
              Workstation (+ Software Center)
            </button>
            <button
              type="button"
              class="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 transition"
              :class="headlessPreview ? 'bg-white text-slate-900 shadow-sm dark:bg-slate-700 dark:text-white' : 'text-slate-600 dark:text-slate-400'"
              @click="headlessPreview = true"
            >
              <Server class="h-3.5 w-3.5" aria-hidden="true" />
              Headless-Server (nur Agent)
            </button>
          </div>
        </div>

        <!-- PowerShell One-Liner -->
        <div>
          <div class="mb-1 flex items-center justify-between">
            <span class="text-xs font-semibold text-slate-700 dark:text-slate-300">
              Windows (PowerShell als Administrator / GPO)
            </span>
            <button
              type="button"
              class="inline-flex items-center gap-1 text-xs font-semibold text-sky-600 hover:text-sky-700 dark:text-sky-400"
              @click="copyText(ps1Preview, 'ps1')"
            >
              <Check v-if="copiedTarget === 'ps1'" class="h-3.5 w-3.5 text-emerald-600 dark:text-emerald-400" />
              <Copy v-else class="h-3.5 w-3.5" />
              {{ copiedTarget === 'ps1' ? 'Kopiert!' : 'Befehl kopieren' }}
            </button>
          </div>
          <pre class="overflow-x-auto rounded-lg border border-slate-800 bg-slate-950 p-3 font-mono text-xs text-sky-300">{{ ps1Preview }}</pre>
        </div>

        <!-- Bash One-Liner -->
        <div>
          <div class="mb-1 flex items-center justify-between">
            <span class="text-xs font-semibold text-slate-700 dark:text-slate-300">
              Linux (Debian/Ubuntu &amp; RHEL/Rocky/Fedora)
            </span>
            <button
              type="button"
              class="inline-flex items-center gap-1 text-xs font-semibold text-sky-600 hover:text-sky-700 dark:text-sky-400"
              @click="copyText(shPreview, 'sh')"
            >
              <Check v-if="copiedTarget === 'sh'" class="h-3.5 w-3.5 text-emerald-600 dark:text-emerald-400" />
              <Copy v-else class="h-3.5 w-3.5" />
              {{ copiedTarget === 'sh' ? 'Kopiert!' : 'Befehl kopieren' }}
            </button>
          </div>
          <pre class="overflow-x-auto rounded-lg border border-slate-800 bg-slate-950 p-3 font-mono text-xs text-emerald-300">{{ shPreview }}</pre>
        </div>

        <div class="flex justify-end border-t border-slate-200 pt-4 dark:border-slate-800">
          <button
            type="button"
            class="rounded-lg bg-slate-900 px-5 py-2 text-sm font-semibold text-white hover:bg-slate-800 dark:bg-sky-600 dark:hover:bg-sky-500"
            @click="emit('close')"
          >
            Fertig
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
