<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  Check,
  Copy,
  Download,
  FileCode2,
  KeyRound,
  Layers,
  Monitor,
  Plus,
  Server,
  ShieldCheck,
  Sparkles,
  Terminal,
  Trash2,
} from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type { AgentArtifactItem, DeviceGroup, EnrollmentToken } from '@/api/types'
import { useToast } from '@/composables/useToast'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import EmptyState from '@/components/EmptyState.vue'
import EnrollmentTokenModal from '@/components/EnrollmentTokenModal.vue'
import { applyHeadlessToggle, formatBytes, formatDateTime } from '@/utils/format'

type RolloutTab = 'quick' | 'gpo' | 'ansible' | 'manual'

const toast = useToast()

const tokens = ref<EnrollmentToken[]>([])
const groups = ref<DeviceGroup[]>([])
const artifacts = ref<AgentArtifactItem[]>([])
const serverBaseUrl = ref<string>('')
const loading = ref(false)
const quickCreating = ref(false)
const modalOpen = ref(false)
const headlessMode = ref(false)
const copiedKey = ref<string | null>(null)
const pendingRevokeToken = ref<EnrollmentToken | null>(null)
const revoking = ref(false)
const activeTabByToken = ref<Record<string, RolloutTab>>({})

const resolvedServerUrl = computed(() => {
  if (serverBaseUrl.value) return serverBaseUrl.value.replace(/\/+$/, '')
  const firstCmd = tokens.value[0]?.install_ps1_cmd || ''
  const urlMatch = /(https?:\/\/[^/'"\s]+)\/dl\//.exec(firstCmd)
  if (urlMatch && urlMatch[1]) return urlMatch[1]
  if (typeof window !== 'undefined' && window.location?.origin) {
    return window.location.origin.replace(/\/+$/, '')
  }
  return 'https://swm.beispiel.de'
})

function getActiveTab(tokenId: string): RolloutTab {
  return activeTabByToken.value[tokenId] || 'quick'
}

function setActiveTab(tokenId: string, tab: RolloutTab) {
  activeTabByToken.value = {
    ...activeTabByToken.value,
    [tokenId]: tab,
  }
}

function extractTokenOrigin(tok: EnrollmentToken): string {
  const match = /(https?:\/\/[^/'"\s]+)\/dl\//.exec(tok.install_ps1_cmd || '')
  return (match && match[1]) || resolvedServerUrl.value
}

function buildGpoScript(tok: EnrollmentToken): string {
  const origin = extractTokenOrigin(tok)
  const hl = headlessMode.value ? 1 : 0
  const bootstrapUrl = `${origin}/dl/install.ps1?token=${tok.token}&headless=${hl}&skip_if_enrolled=1`
  return [
    '# NS8-SoftwareManager - Idempotentes Computer-Startskript für Active Directory GPO / Microsoft Intune',
    '# Ausführungskontext: NT AUTHORITY\\SYSTEM (Computerkonfiguration -> Skripts -> Start)',
    '$ErrorActionPreference = "Stop"',
    '[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12',
    '$DataConfig = "C:\\ProgramData\\NS8-SoftwareManager\\config.json"',
    '$Svc = Get-Service -Name "NS8SoftwareManagerAgent" -ErrorAction SilentlyContinue',
    'if ((Test-Path $DataConfig) -and ($null -ne $Svc)) {',
    '    if ($Svc.Status -ne "Running") { Start-Service -Name "NS8SoftwareManagerAgent" -ErrorAction SilentlyContinue }',
    '    exit 0',
    '}',
    `Invoke-RestMethod -UseBasicParsing -Uri '${bootstrapUrl}' | Invoke-Expression`,
    '',
  ].join('\r\n')
}

function buildAnsiblePlaybook(tok: EnrollmentToken): string {
  const origin = extractTokenOrigin(tok)
  const hl = headlessMode.value ? 1 : 0
  const bootstrapUrl = `${origin}/dl/install.sh?token=${tok.token}&headless=${hl}&skip_if_enrolled=1`
  return [
    '---',
    '- name: NS8-SoftwareManager Agent & Software Center ausrollen',
    '  hosts: all',
    '  become: true',
    '  tasks:',
    '    - name: Endpunkt per Bootstrap-Installer registrieren (idempotent)',
    `      ansible.builtin.shell: "curl -fsSL '${bootstrapUrl}' | bash"`,
    '      args:',
    '        creates: /etc/ns8-swm/config.json',
    '',
    '    - name: Sicherstellen, dass swm-agent.service aktiv ist',
    '      ansible.builtin.systemd:',
    '        name: swm-agent.service',
    '        enabled: true',
    '        state: started',
    '',
  ].join('\n')
}

function buildManualWinSnippet(tok: EnrollmentToken): string {
  const origin = extractTokenOrigin(tok)
  const lines = [
    '# 1. Dateien nach "C:\\Program Files\\NS8-SoftwareManager" kopieren (swm-agent.exe + optional swm-center.exe)',
    'New-Item -ItemType Directory -Force -Path "C:\\Program Files\\NS8-SoftwareManager", "C:\\ProgramData\\NS8-SoftwareManager" | Out-Null',
    `& "C:\\Program Files\\NS8-SoftwareManager\\swm-agent.exe" enroll --server "${origin}" --token "${tok.token}" --data-dir "C:\\ProgramData\\NS8-SoftwareManager"`,
    '& "C:\\Program Files\\NS8-SoftwareManager\\swm-agent.exe" service-install --data-dir "C:\\ProgramData\\NS8-SoftwareManager"',
  ]
  if (!headlessMode.value) {
    lines.push(
      'Set-ItemProperty -Path "HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" -Name "NS8SoftwareCenter" -Value \'"C:\\Program Files\\NS8-SoftwareManager\\swm-center.exe" gui --tray-only\'',
    )
  }
  return lines.join('\r\n')
}

function buildManualLinuxSnippet(tok: EnrollmentToken): string {
  const origin = extractTokenOrigin(tok)
  return [
    '# Binary nach /usr/local/bin/swm-agent kopieren (chmod 0755) und registrieren:',
    'sudo mkdir -p /etc/ns8-swm /run/ns8-swm',
    `sudo /usr/local/bin/swm-agent enroll --server "${origin}" --token "${tok.token}" --data-dir /etc/ns8-swm`,
    'sudo systemctl enable --now swm-agent.service',
  ].join('\n')
}

function downloadTextFile(filename: string, content: string, mimeType = 'text/plain;charset=utf-8') {
  if (typeof document === 'undefined' || typeof URL === 'undefined') return
  const blob = new Blob([content], { type: mimeType })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = filename
  document.body.appendChild(anchor)
  anchor.click()
  document.body.removeChild(anchor)
  URL.revokeObjectURL(url)
  toast.success('Datei heruntergeladen', filename)
}

async function loadAll() {
  loading.value = true
  try {
    const [tokList, grpList, artMeta] = await Promise.all([
      adminApi.listEnrollmentTokens(),
      adminApi.listGroups(),
      adminApi.getAgentArtifacts().catch(() => ({
        server_url: '',
        version: '1.0.0',
        artifacts: [],
      })),
    ])
    tokens.value = tokList
    groups.value = grpList
    artifacts.value = artMeta.artifacts || []
    if (artMeta.server_url) {
      serverBaseUrl.value = artMeta.server_url
    }
  } catch (err) {
    toast.error(
      'Enrollment-Daten konnten nicht geladen werden',
      err instanceof Error ? err.message : undefined,
    )
  } finally {
    loading.value = false
  }
}

async function createQuickStartToken() {
  quickCreating.value = true
  try {
    const created = await adminApi.createEnrollmentToken({
      name: 'Standard Client-Rollout (Auto-Approve)',
      expires_in_hours: 168,
      max_uses: null,
      auto_approve: true,
      group_id: groups.value[0]?.id || null,
    })
    tokens.value = [created, ...tokens.value]
    toast.success(
      'Schnellstart-Token erstellt',
      'Kopieren Sie nun den passenden Einzeiler oder laden Sie das GPO-/Ansible-Skript herunter.',
    )
  } catch (err) {
    toast.error('Token-Erstellung fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    quickCreating.value = false
  }
}

async function copyRaw(text: string, key: string, label = 'In Zwischenablage kopiert') {
  try {
    await navigator.clipboard.writeText(text)
    copiedKey.value = key
    setTimeout(() => {
      copiedKey.value = null
    }, 2000)
    toast.success(label)
  } catch {
    // ignore in non-browser environment
  }
}

async function copyCmd(cmd: string, key: string) {
  const transformed = applyHeadlessToggle(cmd, headlessMode.value)
  await copyRaw(transformed, key, 'Installationsbefehl in Zwischenablage kopiert')
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
    toast.success(
      'Enrollment-Token widerrufen',
      `„${target.name}“ kann nicht mehr zur Registrierung genutzt werden.`,
    )
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
    <!-- Page Header & Workstation/Server Profile Switch -->
    <div class="flex flex-col justify-between gap-4 lg:flex-row lg:items-center">
      <div>
        <h1 class="text-2xl font-bold tracking-tight text-slate-900 dark:text-slate-50">
          Client-Rollout &amp; Agent-Installation
        </h1>
        <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
          So installieren Sie den Hintergrund-Agenten (<code class="font-mono text-xs">swm-agent</code>) und das Endbenutzer-Software-Center (<code class="font-mono text-xs">swm-center</code>) auf Windows- &amp; Linux-Systemen
        </p>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <!-- Global Desktop vs Headless Profile Toggle -->
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
          data-testid="open-token-modal-btn"
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white shadow-sm hover:bg-sky-700"
          @click="modalOpen = true"
        >
          <Plus class="h-4 w-4" aria-hidden="true" />
          Neues Token erstellen
        </button>
      </div>
    </div>

    <!-- Clear Architecture Explainer: What gets installed on the client? -->
    <div class="grid grid-cols-1 gap-4 lg:grid-cols-2">
      <!-- Component 1: swm-agent -->
      <div
        class="rounded-xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900"
      >
        <div class="flex items-start gap-3.5">
          <div class="rounded-lg bg-sky-50 p-2.5 text-sky-600 dark:bg-sky-500/15 dark:text-sky-400">
            <ShieldCheck class="h-5 w-5" aria-hidden="true" />
          </div>
          <div class="min-w-0 flex-1">
            <div class="flex flex-wrap items-center gap-2">
              <h2 class="text-sm font-bold text-slate-900 dark:text-slate-100">
                1. Management-Agent (<code class="font-mono text-xs">swm-agent</code>)
              </h2>
              <span class="rounded-full bg-emerald-50 px-2 py-0.5 text-[11px] font-semibold text-emerald-700 dark:bg-emerald-500/15 dark:text-emerald-300">
                Immer aktiv · Systemdienst
              </span>
            </div>
            <p class="mt-1.5 text-xs leading-relaxed text-slate-600 dark:text-slate-400">
              Läuft dauerhaft im Hintergrund als privilegierter Dienst (<code class="font-mono">NS8SoftwareManagerAgent</code> unter Windows bzw. <code class="font-mono">swm-agent.service</code> unter Linux). Verbindet sich <strong>ausschließlich ausgehend per HTTPS/WSS (Port 443)</strong> mit <code class="font-mono text-slate-800 dark:text-slate-200">{{ resolvedServerUrl }}</code>, überträgt Inventar &amp; Patch-Stand und führt Ed25519-signierte Aufträge aus.
            </p>
          </div>
        </div>
      </div>

      <!-- Component 2: swm-center -->
      <div
        class="rounded-xl border p-5 shadow-xs transition"
        :class="
          !headlessMode
            ? 'border-sky-200 bg-white dark:border-sky-500/30 dark:bg-slate-900'
            : 'border-slate-200 bg-slate-50/70 opacity-75 dark:border-slate-800 dark:bg-slate-900/50'
        "
      >
        <div class="flex items-start gap-3.5">
          <div class="rounded-lg bg-cyan-50 p-2.5 text-cyan-700 dark:bg-cyan-500/15 dark:text-cyan-300">
            <Monitor class="h-5 w-5" aria-hidden="true" />
          </div>
          <div class="min-w-0 flex-1">
            <div class="flex flex-wrap items-center gap-2">
              <h2 class="text-sm font-bold text-slate-900 dark:text-slate-100">
                2. Endbenutzer „NS8 Software Center“ (<code class="font-mono text-xs">swm-center</code>)
              </h2>
              <span
                class="rounded-full px-2 py-0.5 text-[11px] font-semibold"
                :class="
                  !headlessMode
                    ? 'bg-sky-50 text-sky-700 dark:bg-sky-500/15 dark:text-sky-300'
                    : 'bg-slate-200 text-slate-700 dark:bg-slate-800 dark:text-slate-400'
                "
              >
                {{ !headlessMode ? 'Wird mitinstalliert (Startmenü & Tray)' : 'Im Headless-Modus übersprungen' }}
              </span>
            </div>
            <p class="mt-1.5 text-xs leading-relaxed text-slate-600 dark:text-slate-400">
              Wird auf Arbeitsplatzrechnern automatisch ins <strong>Windows-Startmenü &amp; System-Tray</strong> bzw. ins <strong>Linux-Anwendungsmenü</strong> eingetragen. Mitarbeiter öffnen darüber den Self-Service-Katalog und installieren freigegebene Programme (<em>Verfügbar</em>) <strong>ohne lokale Administratorrechte</strong> über eine abgesicherte lokale IPC-Verbindung zum Dienst.
            </p>
          </div>
        </div>
      </div>
    </div>

    <!-- Empty State with 1-Click Quick-Start Token -->
    <div v-if="tokens.length === 0 && !loading">
      <EmptyState
        :icon="KeyRound"
        title="Schritt 1: Erstellen Sie ein Enrollment-Token für Ihre Clients"
        description="Mit einem Enrollment-Token generiert der Server sofort kopierfertige PowerShell- und Bash-Einzeiler sowie herunterladbare Active-Directory-GPO-Startskripte (.ps1) und Ansible-Playbooks (.yml)."
      >
        <template #actions>
          <button
            type="button"
            data-testid="quick-create-token-btn"
            :disabled="quickCreating"
            class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white shadow-xs hover:bg-sky-700 disabled:opacity-60"
            @click="createQuickStartToken"
          >
            <Sparkles class="h-4 w-4" aria-hidden="true" />
            {{ quickCreating ? 'Wird generiert…' : 'Schnellstart-Token (7 Tage, Auto-Approve) mit 1 Klick erzeugen' }}
          </button>
          <button
            type="button"
            class="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-4 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
            @click="modalOpen = true"
          >
            <Plus class="h-4 w-4" aria-hidden="true" />
            Benutzerdefiniertes Token…
          </button>
        </template>
      </EmptyState>
    </div>

    <!-- Token Cards with 4 Scenario Rollout Tabs -->
    <div v-else class="space-y-5">
      <div
        v-for="tok in tokens"
        :key="tok.id"
        :data-testid="`enrollment-card-${tok.id}`"
        class="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900"
      >
        <!-- Token Header -->
        <div class="flex flex-col justify-between gap-3 border-b border-slate-100 p-5 sm:flex-row sm:items-center dark:border-slate-800">
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
                  {{ tok.auto_approve ? 'Auto-Approve (Sofort freigegeben)' : 'Manuelle Admin-Freigabe' }}
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

        <!-- Rollout Method Tabs -->
        <div class="flex flex-wrap gap-1 border-b border-slate-200 bg-slate-50/80 px-5 pt-2.5 dark:border-slate-800 dark:bg-slate-950/50">
          <button
            type="button"
            class="inline-flex items-center gap-1.5 rounded-t-lg border-b-2 px-3 py-2 text-xs font-semibold transition"
            :class="
              getActiveTab(tok.id) === 'quick'
                ? 'border-sky-600 bg-white text-sky-700 dark:border-sky-400 dark:bg-slate-900 dark:text-sky-300'
                : 'border-transparent text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
            "
            @click="setActiveTab(tok.id, 'quick')"
          >
            <Terminal class="h-3.5 w-3.5" aria-hidden="true" />
            Einzelplatz / Schnelltest (1-Zeiler)
          </button>

          <button
            type="button"
            :data-testid="`tab-gpo-${tok.id}`"
            class="inline-flex items-center gap-1.5 rounded-t-lg border-b-2 px-3 py-2 text-xs font-semibold transition"
            :class="
              getActiveTab(tok.id) === 'gpo'
                ? 'border-sky-600 bg-white text-sky-700 dark:border-sky-400 dark:bg-slate-900 dark:text-sky-300'
                : 'border-transparent text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
            "
            @click="setActiveTab(tok.id, 'gpo')"
          >
            <Layers class="h-3.5 w-3.5" aria-hidden="true" />
            Windows Flotte (AD-GPO / Intune)
          </button>

          <button
            type="button"
            :data-testid="`tab-ansible-${tok.id}`"
            class="inline-flex items-center gap-1.5 rounded-t-lg border-b-2 px-3 py-2 text-xs font-semibold transition"
            :class="
              getActiveTab(tok.id) === 'ansible'
                ? 'border-sky-600 bg-white text-sky-700 dark:border-sky-400 dark:bg-slate-900 dark:text-sky-300'
                : 'border-transparent text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
            "
            @click="setActiveTab(tok.id, 'ansible')"
          >
            <FileCode2 class="h-3.5 w-3.5" aria-hidden="true" />
            Linux Flotte (Ansible / Cloud-Init)
          </button>

          <button
            type="button"
            :data-testid="`tab-manual-${tok.id}`"
            class="inline-flex items-center gap-1.5 rounded-t-lg border-b-2 px-3 py-2 text-xs font-semibold transition"
            :class="
              getActiveTab(tok.id) === 'manual'
                ? 'border-sky-600 bg-white text-sky-700 dark:border-sky-400 dark:bg-slate-900 dark:text-sky-300'
                : 'border-transparent text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
            "
            @click="setActiveTab(tok.id, 'manual')"
          >
            <Download class="h-3.5 w-3.5" aria-hidden="true" />
            Manuell / Gold-Image CLI
          </button>
        </div>

        <!-- Tab 1: Quick Interactive One-Liners -->
        <div v-if="getActiveTab(tok.id) === 'quick'" class="space-y-3 p-5">
          <p class="text-xs text-slate-600 dark:text-slate-400">
            Öffnen Sie auf dem Zielrechner eine <strong>Administrator-PowerShell</strong> (Windows) bzw. ein <strong>Root-/Sudo-Terminal</strong> (Linux) und führen Sie den Einzeiler aus. Der Rechner erscheint sofort unter <strong>Geräte &amp; Flotte</strong>.
          </p>

          <div class="grid grid-cols-1 gap-3 lg:grid-cols-2">
            <!-- PowerShell Command -->
            <div class="rounded-lg border border-slate-800 bg-slate-950 p-3.5">
              <div class="mb-2 flex items-center justify-between text-[11px]">
                <span class="font-semibold text-sky-400">Windows PowerShell (als Administrator)</span>
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
              <pre class="whitespace-pre-wrap break-all font-mono text-[11px] leading-relaxed text-slate-200">{{ applyHeadlessToggle(tok.install_ps1_cmd, headlessMode) }}</pre>
            </div>

            <!-- Bash Command -->
            <div class="rounded-lg border border-slate-800 bg-slate-950 p-3.5">
              <div class="mb-2 flex items-center justify-between text-[11px]">
                <span class="font-semibold text-emerald-400">Linux Bash (Debian/Ubuntu &amp; RHEL/Rocky/Fedora)</span>
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
              <pre class="whitespace-pre-wrap break-all font-mono text-[11px] leading-relaxed text-slate-200">{{ applyHeadlessToggle(tok.install_sh_cmd, headlessMode) }}</pre>
            </div>
          </div>
        </div>

        <!-- Tab 2: Windows Mass Rollout via Active Directory GPO / Intune -->
        <div v-else-if="getActiveTab(tok.id) === 'gpo'" class="space-y-4 p-5">
          <div class="flex flex-col justify-between gap-3 rounded-lg border border-sky-100 bg-sky-50/70 p-4 sm:flex-row sm:items-center dark:border-sky-500/20 dark:bg-sky-500/10">
            <div class="space-y-1 text-xs text-slate-700 dark:text-slate-200">
              <div class="font-bold text-slate-900 dark:text-white">
                Automatischer Rollout per Active Directory Gruppenrichtlinie (GPO) oder Microsoft Intune
              </div>
              <ol class="list-inside list-decimal space-y-0.5 text-slate-600 dark:text-slate-300">
                <li>Laden Sie rechts <code class="font-mono">install-swm-gpo.ps1</code> herunter und legen Sie es im NETLOGON-/GPO-Skriptordner ab.</li>
                <li>In <code class="font-mono">gpmc.msc</code>: <strong>Computerkonfiguration → Richtlinien → Windows-Einstellungen → Skripts (Start/Herunterfahren) → Start → PowerShell-Skripts</strong>.</li>
                <li>Das Skript prüft vorab <code class="font-mono">C:\ProgramData\NS8-SoftwareManager\config.json</code> und überspringt bereits angebundene PCs beim nächsten Neustart sofort.</li>
              </ol>
            </div>

            <button
              type="button"
              class="inline-flex shrink-0 items-center gap-1.5 rounded-lg bg-sky-600 px-3.5 py-2 text-xs font-semibold text-white shadow-xs hover:bg-sky-700"
              @click="downloadTextFile('install-swm-gpo.ps1', buildGpoScript(tok))"
            >
              <Download class="h-4 w-4" aria-hidden="true" />
              install-swm-gpo.ps1 herunterladen
            </button>
          </div>

          <div class="rounded-lg border border-slate-800 bg-slate-950 p-3.5">
            <div class="mb-2 flex items-center justify-between text-[11px]">
              <span class="font-semibold text-sky-400">install-swm-gpo.ps1 (GPO Computer-Startskript / Intune PowerShell)</span>
              <button
                type="button"
                class="inline-flex items-center gap-1 font-semibold text-slate-300 hover:text-white"
                @click="copyRaw(buildGpoScript(tok), `gpo-${tok.id}`, 'GPO-Startskript kopiert')"
              >
                <Check v-if="copiedKey === `gpo-${tok.id}`" class="h-3.5 w-3.5 text-emerald-400" />
                <Copy v-else class="h-3.5 w-3.5" />
                {{ copiedKey === `gpo-${tok.id}` ? 'Kopiert' : 'Skript kopieren' }}
              </button>
            </div>
            <pre class="overflow-x-auto font-mono text-[11px] text-slate-200">{{ buildGpoScript(tok) }}</pre>
          </div>
        </div>

        <!-- Tab 3: Linux Mass Rollout via Ansible / Cloud-Init -->
        <div v-else-if="getActiveTab(tok.id) === 'ansible'" class="space-y-4 p-5">
          <div class="flex flex-col justify-between gap-3 rounded-lg border border-emerald-100 bg-emerald-50/70 p-4 sm:flex-row sm:items-center dark:border-emerald-500/20 dark:bg-emerald-500/10">
            <div class="text-xs text-slate-700 dark:text-slate-200">
              <div class="font-bold text-slate-900 dark:text-white">
                Idempotentes Ansible-Playbook für Linux-Server &amp; Linux-Workstations
              </div>
              <p class="mt-1 text-slate-600 dark:text-slate-300">
                Ausführen mit <code class="font-mono">ansible-playbook -i inventory.ini deploy-swm-agent.yml</code>. Dank <code class="font-mono">creates: /etc/ns8-swm/config.json</code> werden bereits registrierte Systeme nicht erneut eingeschrieben.
              </p>
            </div>

            <button
              type="button"
              class="inline-flex shrink-0 items-center gap-1.5 rounded-lg bg-emerald-600 px-3.5 py-2 text-xs font-semibold text-white shadow-xs hover:bg-emerald-700"
              @click="downloadTextFile('deploy-swm-agent.yml', buildAnsiblePlaybook(tok))"
            >
              <Download class="h-4 w-4" aria-hidden="true" />
              deploy-swm-agent.yml herunterladen
            </button>
          </div>

          <div class="rounded-lg border border-slate-800 bg-slate-950 p-3.5">
            <div class="mb-2 flex items-center justify-between text-[11px]">
              <span class="font-semibold text-emerald-400">deploy-swm-agent.yml</span>
              <button
                type="button"
                class="inline-flex items-center gap-1 font-semibold text-slate-300 hover:text-white"
                @click="copyRaw(buildAnsiblePlaybook(tok), `ans-${tok.id}`, 'Ansible-Playbook kopiert')"
              >
                <Check v-if="copiedKey === `ans-${tok.id}`" class="h-3.5 w-3.5 text-emerald-400" />
                <Copy v-else class="h-3.5 w-3.5" />
                {{ copiedKey === `ans-${tok.id}` ? 'Kopiert' : 'Playbook kopieren' }}
              </button>
            </div>
            <pre class="overflow-x-auto font-mono text-[11px] text-slate-200">{{ buildAnsiblePlaybook(tok) }}</pre>
          </div>
        </div>

        <!-- Tab 4: Manual / Golden-Image CLI Commands -->
        <div v-else class="space-y-3 p-5">
          <p class="text-xs text-slate-600 dark:text-slate-400">
            Für Offline-Umgebungen, RMM-Werkzeuge oder vorinstallierte Golden-Images können Sie die unten aufgelisteten Binaries vorab ablegen und die Einschreibung direkt per CLI auslösen:
          </p>

          <div class="grid grid-cols-1 gap-3 lg:grid-cols-2">
            <div class="rounded-lg border border-slate-800 bg-slate-950 p-3.5">
              <div class="mb-2 flex items-center justify-between text-[11px]">
                <span class="font-semibold text-sky-400">Windows CLI (swm-agent.exe)</span>
                <button
                  type="button"
                  class="inline-flex items-center gap-1 font-semibold text-slate-300 hover:text-white"
                  @click="copyRaw(buildManualWinSnippet(tok), `man-win-${tok.id}`)"
                >
                  <Check v-if="copiedKey === `man-win-${tok.id}`" class="h-3.5 w-3.5 text-emerald-400" />
                  <Copy v-else class="h-3.5 w-3.5" />
                  Kopieren
                </button>
              </div>
              <pre class="overflow-x-auto font-mono text-[11px] text-slate-200">{{ buildManualWinSnippet(tok) }}</pre>
            </div>

            <div class="rounded-lg border border-slate-800 bg-slate-950 p-3.5">
              <div class="mb-2 flex items-center justify-between text-[11px]">
                <span class="font-semibold text-emerald-400">Linux CLI (swm-agent)</span>
                <button
                  type="button"
                  class="inline-flex items-center gap-1 font-semibold text-slate-300 hover:text-white"
                  @click="copyRaw(buildManualLinuxSnippet(tok), `man-lin-${tok.id}`)"
                >
                  <Check v-if="copiedKey === `man-lin-${tok.id}`" class="h-3.5 w-3.5 text-emerald-400" />
                  <Copy v-else class="h-3.5 w-3.5" />
                  Kopieren
                </button>
              </div>
              <pre class="overflow-x-auto font-mono text-[11px] text-slate-200">{{ buildManualLinuxSnippet(tok) }}</pre>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Direct Client Binary Downloads & SHA-256 Checksums -->
    <div
      v-if="artifacts.length > 0"
      data-testid="agent-artifacts-panel"
      class="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900"
    >
      <div class="border-b border-slate-200 px-5 py-4 dark:border-slate-800">
        <h2 class="text-sm font-bold text-slate-900 dark:text-slate-100">
          Direkt-Downloads der Client-Binaries &amp; SHA-256-Prüfsummen
        </h2>
        <p class="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
          Alle Binaries werden direkt von Ihrem NS8-SoftwareManager-Server ausgeliefert (für manuelle Verteilung, Softwareverteilungssysteme oder Self-Update-Prüfung).
        </p>
      </div>

      <div class="overflow-x-auto">
        <table class="min-w-full divide-y divide-slate-200 text-xs dark:divide-slate-800">
          <thead class="bg-slate-50 text-left font-semibold uppercase tracking-wider text-slate-500 dark:bg-slate-800/70 dark:text-slate-400">
            <tr>
              <th class="px-4 py-3">Komponente</th>
              <th class="px-4 py-3">Datei</th>
              <th class="px-4 py-3">Größe</th>
              <th class="px-4 py-3">SHA-256 Prüfsumme</th>
              <th class="px-4 py-3 text-right">Download</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-100 dark:divide-slate-800/70">
            <tr
              v-for="art in artifacts"
              :key="`${art.platform}/${art.filename}`"
              class="hover:bg-slate-50/80 dark:hover:bg-slate-800/50"
            >
              <td class="px-4 py-3 font-medium text-slate-900 dark:text-slate-100">
                {{ art.label }}
              </td>
              <td class="px-4 py-3 font-mono text-slate-600 dark:text-slate-300">
                {{ art.platform }}/{{ art.filename }}
              </td>
              <td class="px-4 py-3 text-slate-600 dark:text-slate-400">
                {{ art.available ? formatBytes(art.size_bytes) : 'Nicht gebaut' }}
              </td>
              <td class="px-4 py-3 font-mono text-[11px] text-slate-600 dark:text-slate-300">
                <div v-if="art.available && art.sha256" class="inline-flex items-center gap-1.5">
                  <span :title="art.sha256">{{ art.sha256.slice(0, 18) }}…</span>
                  <button
                    type="button"
                    :aria-label="`SHA-256 für ${art.filename} kopieren`"
                    class="rounded border border-slate-200 px-1.5 py-0.5 text-[10px] font-sans font-semibold text-slate-600 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
                    @click="copyRaw(art.sha256, `sha-${art.platform}-${art.filename}`, 'SHA-256 Prüfsumme kopiert')"
                  >
                    {{ copiedKey === `sha-${art.platform}-${art.filename}` ? 'Kopiert' : 'SHA-256' }}
                  </button>
                </div>
                <span v-else class="text-slate-400">—</span>
              </td>
              <td class="px-4 py-3 text-right">
                <a
                  v-if="art.available"
                  :href="art.download_path"
                  download
                  class="inline-flex items-center gap-1 rounded-md bg-slate-900 px-2.5 py-1 font-semibold text-white hover:bg-slate-800 dark:bg-sky-600 dark:hover:bg-sky-500"
                >
                  <Download class="h-3.5 w-3.5" aria-hidden="true" />
                  Herunterladen
                </a>
                <span v-else class="text-slate-400">Nicht verfügbar</span>
              </td>
            </tr>
          </tbody>
        </table>
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
