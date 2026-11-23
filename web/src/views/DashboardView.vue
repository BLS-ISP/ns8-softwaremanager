<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { RouterLink } from 'vue-router'
import {
  Activity,
  AlertTriangle,
  ArrowRight,
  ArrowUpRight,
  CheckCircle2,
  Clock,
  Download,
  KeyRound,
  Laptop,
  Monitor,
  Package,
  RefreshCw,
  RotateCcw,
  ShieldAlert,
  Terminal,
} from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type { DashboardSummary, JobItem } from '@/api/types'
import EmptyState from '@/components/EmptyState.vue'
import JobLogModal from '@/components/JobLogModal.vue'
import StatusBadge from '@/components/StatusBadge.vue'
import { formatDateTime } from '@/utils/format'

const summary = ref<DashboardSummary>({
  total_devices: 0,
  online_devices: 0,
  pending_approval_devices: 0,
  reboot_required_devices: 0,
  patch_compliance_percent: 100,
  pending_critical_patches: 0,
  recent_jobs: [],
  os_breakdown: { windows: 0, linux: 0 },
})

const loading = ref(false)
const error = ref<string | null>(null)
const selectedJobId = ref<string | null>(null)
const selectedJobInitial = ref<JobItem | null>(null)

const windowsPercent = computed(() => {
  const total =
    (summary.value.os_breakdown?.windows || 0) + (summary.value.os_breakdown?.linux || 0)
  if (!total) return 50
  return Math.round(((summary.value.os_breakdown?.windows || 0) / total) * 100)
})

const onlinePercent = computed(() => {
  if (!summary.value.total_devices) return 0
  return Math.round((summary.value.online_devices / summary.value.total_devices) * 100)
})

async function loadDashboard() {
  loading.value = true
  error.value = null
  try {
    summary.value = await adminApi.getDashboardSummary()
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Dashboard-Daten konnten nicht geladen werden'
  } finally {
    loading.value = false
  }
}

function openJobLog(job: JobItem) {
  selectedJobInitial.value = job
  selectedJobId.value = job.id
}

onMounted(() => {
  loadDashboard()
})
</script>

<template>
  <div class="space-y-6">
    <!-- Page Header -->
    <div class="flex flex-col justify-between gap-4 sm:flex-row sm:items-center">
      <div>
        <h1 class="text-2xl font-bold tracking-tight text-slate-900 dark:text-slate-50">
          Flotten- &amp; Patch-Compliance-Dashboard
        </h1>
        <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
          Echtzeit-Überblick über Windows- &amp; Linux-Systeme, Sicherheitsupdates und aktive Rollouts
        </p>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <RouterLink
          to="/enrollment"
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-3.5 py-2 text-xs font-semibold text-white shadow-sm transition hover:bg-sky-700"
        >
          <KeyRound class="h-3.5 w-3.5" aria-hidden="true" />
          Client anbinden
        </RouterLink>
        <button
          type="button"
          aria-label="Dashboard-Daten aktualisieren"
          class="inline-flex items-center gap-2 rounded-lg border border-slate-300 bg-white px-3.5 py-2 text-xs font-semibold text-slate-700 shadow-xs transition hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
          :disabled="loading"
          @click="loadDashboard"
        >
          <RefreshCw class="h-4 w-4" :class="{ 'animate-spin': loading }" />
          Aktualisieren
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

    <!-- Pending Approval Banner -->
    <div
      v-if="summary.pending_approval_devices > 0"
      data-testid="pending-approval-banner"
      class="flex flex-col items-start justify-between gap-3 rounded-xl border border-amber-300 bg-amber-50 px-5 py-4 sm:flex-row sm:items-center dark:border-amber-500/40 dark:bg-amber-500/10"
    >
      <div class="flex items-center gap-3">
        <AlertTriangle class="h-5 w-5 shrink-0 text-amber-600 dark:text-amber-400" aria-hidden="true" />
        <div class="text-sm text-amber-900 dark:text-amber-200">
          <strong>{{ summary.pending_approval_devices }}</strong> neu registrierte(s) Gerät(e) warten auf Ihre Administrator-Freigabe.
        </div>
      </div>
      <RouterLink
        to="/devices?status=pending"
        class="rounded-lg bg-amber-600 px-3.5 py-1.5 text-xs font-semibold text-white transition hover:bg-amber-700"
      >
        Jetzt prüfen &amp; freigeben
      </RouterLink>
    </div>

    <!-- First-Run Zero-Devices Onboarding Guide -->
    <div
      v-if="!loading && summary.total_devices === 0"
      data-testid="zero-devices-onboarding"
      class="rounded-xl border border-sky-200 bg-white p-6 shadow-xs dark:border-sky-500/30 dark:bg-slate-900"
    >
      <div class="flex flex-col justify-between gap-4 border-b border-slate-100 pb-5 lg:flex-row lg:items-center dark:border-slate-800">
        <div class="flex items-start gap-3.5">
          <div class="rounded-xl bg-sky-50 p-3 text-sky-600 dark:bg-sky-500/15 dark:text-sky-400">
            <Download class="h-6 w-6" aria-hidden="true" />
          </div>
          <div>
            <span class="inline-flex items-center rounded-full bg-sky-50 px-2.5 py-0.5 text-[11px] font-semibold text-sky-700 dark:bg-sky-500/15 dark:text-sky-300">
              Schnellstart · Client-Anbindung
            </span>
            <h2 class="mt-1 text-base font-bold text-slate-900 dark:text-slate-50">
              So kommen Management-Agent &amp; Software Center auf Ihre Windows- und Linux-Clients
            </h2>
            <p class="mt-1 max-w-3xl text-xs leading-relaxed text-slate-600 dark:text-slate-400">
              Clients verbinden sich ausschließlich ausgehend über HTTPS/WSS (Port 443) mit diesem Server. Der Installer richtet sowohl den Hintergrunddienst (<code class="font-mono text-slate-800 dark:text-slate-200">swm-agent</code>) als auch auf Arbeitsplätzen die Self-Service-App <strong class="text-slate-800 dark:text-slate-200">„NS8 Software Center“</strong> (<code class="font-mono text-slate-800 dark:text-slate-200">swm-center</code>) ohne lokale Admin-Rechte ein.
            </p>
          </div>
        </div>

        <RouterLink
          to="/enrollment"
          class="inline-flex shrink-0 items-center gap-2 rounded-lg bg-sky-600 px-4 py-2.5 text-xs font-semibold text-white shadow-xs transition hover:bg-sky-700"
        >
          Rollout-Assistent &amp; Downloads öffnen
          <ArrowRight class="h-4 w-4" aria-hidden="true" />
        </RouterLink>
      </div>

      <div class="mt-5 grid grid-cols-1 gap-4 md:grid-cols-3">
        <div class="rounded-lg border border-slate-200 bg-slate-50/70 p-4 dark:border-slate-800 dark:bg-slate-800/40">
          <div class="flex items-center gap-2 text-xs font-bold text-slate-900 dark:text-slate-100">
            <span class="flex h-5 w-5 items-center justify-center rounded-full bg-sky-600 text-[11px] text-white">1</span>
            Enrollment-Token erstellen
          </div>
          <p class="mt-2 text-xs leading-relaxed text-slate-600 dark:text-slate-400">
            Erzeugen Sie unter <strong>Client-Rollout &amp; Agent</strong> mit einem Klick ein Token — wahlweise mit automatischer Freigabe (<em>Auto-Approve</em> für GPO/Ansible) oder manueller Prüfung.
          </p>
        </div>

        <div class="rounded-lg border border-slate-200 bg-slate-50/70 p-4 dark:border-slate-800 dark:bg-slate-800/40">
          <div class="flex items-center gap-2 text-xs font-bold text-slate-900 dark:text-slate-100">
            <span class="flex h-5 w-5 items-center justify-center rounded-full bg-sky-600 text-[11px] text-white">2</span>
            Auf Einzelplatz oder Flotte verteilen
          </div>
          <p class="mt-2 text-xs leading-relaxed text-slate-600 dark:text-slate-400">
            Kopieren Sie den <strong>PowerShell- / Bash-Einzeiler</strong> für Tests oder laden Sie das fertige, idempotente <strong>AD-GPO-Startskript (<code>.ps1</code>)</strong> bzw. <strong>Ansible-Playbook (<code>.yml</code>)</strong> herunter.
          </p>
        </div>

        <div class="rounded-lg border border-slate-200 bg-slate-50/70 p-4 dark:border-slate-800 dark:bg-slate-800/40">
          <div class="flex items-center gap-2 text-xs font-bold text-slate-900 dark:text-slate-100">
            <Monitor class="h-4 w-4 text-sky-600 dark:text-sky-400" aria-hidden="true" />
            Zentrale Steuerung &amp; Self-Service
          </div>
          <p class="mt-2 text-xs leading-relaxed text-slate-600 dark:text-slate-400">
            Nach der Registrierung steuern Sie OS-Updates und Pflicht-Software zentral, während Benutzer freigegebene Apps selbst im <strong>NS8 Software Center</strong> (Startmenü / System-Tray) installieren.
          </p>
        </div>
      </div>
    </div>

    <!-- Refined 4-Card KPI Hierarchy -->
    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
      <!-- 1. Managed Fleet & Online Ratio -->
      <div
        class="flex flex-col justify-between rounded-xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900"
      >
        <div>
          <div class="flex items-center justify-between">
            <span class="text-xs font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400">
              Verwaltete Systeme
            </span>
            <div class="rounded-lg bg-sky-50 p-2 text-sky-600 dark:bg-sky-500/15 dark:text-sky-400">
              <Laptop class="h-4 w-4" aria-hidden="true" />
            </div>
          </div>
          <div class="mt-3 flex items-baseline gap-2.5">
            <span
              data-testid="kpi-total-devices"
              class="text-3xl font-bold tabular-nums text-slate-900 dark:text-slate-50"
            >
              {{ summary.total_devices }}
            </span>
            <span
              data-testid="kpi-online-devices"
              class="inline-flex items-center gap-1.5 rounded-full bg-emerald-50 px-2.5 py-0.5 text-xs font-semibold text-emerald-700 dark:bg-emerald-500/15 dark:text-emerald-300"
            >
              <span class="h-1.5 w-1.5 rounded-full bg-emerald-500" />
              {{ summary.online_devices }} Online
            </span>
          </div>
        </div>
        <div class="mt-4 border-t border-slate-100 pt-3 text-xs text-slate-500 dark:border-slate-800 dark:text-slate-400">
          {{ Math.max(0, summary.total_devices - summary.online_devices) }} Offline ({{ onlinePercent }}% verbunden)
        </div>
      </div>

      <!-- 2. Patch Compliance -->
      <div
        class="flex flex-col justify-between rounded-xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900"
      >
        <div>
          <div class="flex items-center justify-between">
            <span class="text-xs font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400">
              Patch-Compliance
            </span>
            <div class="rounded-lg bg-emerald-50 p-2 text-emerald-600 dark:bg-emerald-500/15 dark:text-emerald-400">
              <CheckCircle2 class="h-4 w-4" aria-hidden="true" />
            </div>
          </div>
          <div class="mt-3 flex items-baseline justify-between gap-2">
            <span
              data-testid="kpi-compliance"
              class="text-3xl font-bold tabular-nums text-slate-900 dark:text-slate-50"
            >
              {{ Number(summary.patch_compliance_percent ?? 0).toFixed(1) }}%
            </span>
            <span
              class="rounded-md px-2 py-0.5 text-[11px] font-semibold"
              :class="
                summary.patch_compliance_percent >= 85
                  ? 'bg-emerald-50 text-emerald-700 dark:bg-emerald-500/15 dark:text-emerald-300'
                  : summary.patch_compliance_percent >= 60
                    ? 'bg-amber-50 text-amber-700 dark:bg-amber-500/15 dark:text-amber-300'
                    : 'bg-rose-50 text-rose-700 dark:bg-rose-500/15 dark:text-rose-300'
              "
            >
              {{ summary.patch_compliance_percent >= 85 ? 'Ziel erreicht' : 'Prüfung nötig' }}
            </span>
          </div>
        </div>
        <div class="mt-4">
          <div class="h-2 w-full overflow-hidden rounded-full bg-slate-100 dark:bg-slate-800">
            <div
              class="h-full rounded-full transition-all duration-300"
              :class="
                summary.patch_compliance_percent >= 85
                  ? 'bg-emerald-500'
                  : summary.patch_compliance_percent >= 60
                    ? 'bg-amber-500'
                    : 'bg-rose-500'
              "
              :style="{ width: `${Math.min(100, Math.max(0, summary.patch_compliance_percent || 0))}%` }"
            />
          </div>
        </div>
      </div>

      <!-- 3. Critical OS Patches -->
      <div
        class="flex flex-col justify-between rounded-xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900"
      >
        <div>
          <div class="flex items-center justify-between">
            <span class="text-xs font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400">
              Kritische OS-Patches
            </span>
            <div class="rounded-lg bg-rose-50 p-2 text-rose-600 dark:bg-rose-500/15 dark:text-rose-400">
              <ShieldAlert class="h-4 w-4" aria-hidden="true" />
            </div>
          </div>
          <div class="mt-3 flex items-baseline gap-2">
            <span
              data-testid="kpi-critical-patches"
              class="text-3xl font-bold tabular-nums"
              :class="
                summary.pending_critical_patches > 0
                  ? 'text-rose-600 dark:text-rose-400'
                  : 'text-slate-900 dark:text-slate-50'
              "
            >
              {{ summary.pending_critical_patches }}
            </span>
            <span class="text-xs text-slate-500 dark:text-slate-400">ausstehend</span>
          </div>
        </div>
        <div class="mt-4 border-t border-slate-100 pt-3 dark:border-slate-800">
          <RouterLink
            to="/patches"
            class="inline-flex items-center gap-1 text-xs font-semibold text-sky-600 hover:text-sky-700 dark:text-sky-400 dark:hover:text-sky-300"
          >
            Patch-Freigabe öffnen
            <ArrowUpRight class="h-3.5 w-3.5" aria-hidden="true" />
          </RouterLink>
        </div>
      </div>

      <!-- 4. Pending Reboots -->
      <div
        class="flex flex-col justify-between rounded-xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900"
      >
        <div>
          <div class="flex items-center justify-between">
            <span class="text-xs font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400">
              Neustart ausstehend
            </span>
            <div class="rounded-lg bg-amber-50 p-2 text-amber-600 dark:bg-amber-500/15 dark:text-amber-400">
              <RotateCcw class="h-4 w-4" aria-hidden="true" />
            </div>
          </div>
          <div class="mt-3 flex items-baseline gap-2">
            <span
              data-testid="kpi-reboot-devices"
              class="text-3xl font-bold tabular-nums"
              :class="
                summary.reboot_required_devices > 0
                  ? 'text-amber-600 dark:text-amber-400'
                  : 'text-slate-900 dark:text-slate-50'
              "
            >
              {{ summary.reboot_required_devices }}
            </span>
            <span class="text-xs text-slate-500 dark:text-slate-400">Endpunkte</span>
          </div>
        </div>
        <div class="mt-4 border-t border-slate-100 pt-3 dark:border-slate-800">
          <RouterLink
            to="/policies"
            class="inline-flex items-center gap-1 text-xs font-semibold text-sky-600 hover:text-sky-700 dark:text-sky-400 dark:hover:text-sky-300"
          >
            Neustart-Richtlinien
            <ArrowUpRight class="h-3.5 w-3.5" aria-hidden="true" />
          </RouterLink>
        </div>
      </div>
    </div>

    <!-- Second Row: OS Breakdown & Recent Jobs -->
    <div class="grid grid-cols-1 gap-6 lg:grid-cols-3">
      <!-- OS Breakdown Card -->
      <div
        class="rounded-xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900"
      >
        <div class="flex items-center justify-between">
          <h2 class="text-sm font-bold text-slate-900 dark:text-slate-100">Plattform-Verteilung</h2>
          <Activity class="h-4 w-4 text-slate-400" aria-hidden="true" />
        </div>
        <div class="mt-5 space-y-4">
          <div class="flex items-center justify-between rounded-lg bg-slate-50 px-3 py-2.5 text-sm dark:bg-slate-800/60">
            <span class="flex items-center gap-2.5 font-medium text-slate-700 dark:text-slate-200">
              <svg
                class="h-4 w-4 shrink-0 text-sky-600 dark:text-sky-400"
                viewBox="0 0 24 24"
                fill="currentColor"
                aria-hidden="true"
              >
                <path d="M2.5 3h8.6v8.6H2.5V3zm10.4 0h8.6v8.6h-8.6V3zM2.5 13.4h8.6V22H2.5v-8.6zm10.4 0h8.6V22h-8.6v-8.6z" />
              </svg>
              Windows-Systeme
            </span>
            <span
              data-testid="os-windows-count"
              class="font-bold tabular-nums text-slate-900 dark:text-slate-50"
            >
              {{ summary.os_breakdown?.windows ?? 0 }}
            </span>
          </div>

          <div class="flex items-center justify-between rounded-lg bg-slate-50 px-3 py-2.5 text-sm dark:bg-slate-800/60">
            <span class="flex items-center gap-2.5 font-medium text-slate-700 dark:text-slate-200">
              <svg
                class="h-4 w-4 shrink-0 text-orange-600 dark:text-amber-400"
                viewBox="0 0 24 24"
                fill="none"
                aria-hidden="true"
              >
                <path
                  d="M12 2.5C9.5 2.5 8 4.4 8 6.9c0 1.4.4 2.4-.7 3.9-1.3 1.8-2.3 3.7-2.3 5.7 0 2.7 1.8 4.5 4.3 4.5h5.4c2.5 0 4.3-1.8 4.3-4.5 0-2-1-3.9-2.3-5.7-1.1-1.5-.7-2.5-.7-3.9 0-2.5-1.5-4.4-4-4.4Z"
                  fill="currentColor"
                  fill-opacity="0.18"
                  stroke="currentColor"
                  stroke-width="1.75"
                  stroke-linejoin="round"
                />
                <circle cx="10.6" cy="6.6" r="0.85" fill="currentColor" />
                <circle cx="13.4" cy="6.6" r="0.85" fill="currentColor" />
                <path
                  d="M10.5 8.5L12 9.6l1.5-1.1"
                  stroke="currentColor"
                  stroke-width="1.5"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
              Linux-Systeme
            </span>
            <span
              data-testid="os-linux-count"
              class="font-bold tabular-nums text-slate-900 dark:text-slate-50"
            >
              {{ summary.os_breakdown?.linux ?? 0 }}
            </span>
          </div>

          <div class="flex h-2.5 w-full overflow-hidden rounded-full bg-slate-100 dark:bg-slate-800">
            <div class="bg-sky-500 transition-all" :style="{ width: `${windowsPercent}%` }" />
            <div class="bg-orange-500 transition-all" :style="{ width: `${100 - windowsPercent}%` }" />
          </div>

          <div class="grid grid-cols-2 gap-2 pt-1">
            <RouterLink
              to="/patches"
              class="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-center text-xs font-semibold text-slate-700 transition hover:bg-slate-100 dark:border-slate-800 dark:bg-slate-800/70 dark:text-slate-200 dark:hover:bg-slate-800"
            >
              OS-Patching öffnen
            </RouterLink>
            <RouterLink
              to="/catalog"
              class="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-center text-xs font-semibold text-slate-700 transition hover:bg-slate-100 dark:border-slate-800 dark:bg-slate-800/70 dark:text-slate-200 dark:hover:bg-slate-800"
            >
              Software-Katalog
            </RouterLink>
          </div>
        </div>
      </div>

      <!-- Recent Jobs Table -->
      <div
        class="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-xs lg:col-span-2 dark:border-slate-800 dark:bg-slate-900"
      >
        <div class="flex items-center justify-between border-b border-slate-200 px-5 py-4 dark:border-slate-800">
          <div class="flex items-center gap-2">
            <Clock class="h-4 w-4 text-slate-400" aria-hidden="true" />
            <h2 class="text-sm font-bold text-slate-900 dark:text-slate-100">
              Letzte Aufträge &amp; Rollout-Aktivitäten
            </h2>
          </div>
          <RouterLink
            to="/jobs"
            class="text-xs font-semibold text-sky-600 hover:text-sky-700 dark:text-sky-400 dark:hover:text-sky-300"
          >
            Alle Aufträge anzeigen →
          </RouterLink>
        </div>

        <div v-if="!summary.recent_jobs || summary.recent_jobs.length === 0" class="p-6">
          <EmptyState
            :icon="Package"
            title="Noch keine Rollout-Aufträge ausgeführt"
            description="Sobald Endpunkte angebunden sind und OS-Patches oder Software-Pakete freigegeben werden, erscheinen alle signierten Aufträge hier in Echtzeit."
          >
            <template #actions>
              <RouterLink
                to="/enrollment"
                class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-3.5 py-2 text-xs font-semibold text-white hover:bg-sky-700"
              >
                <KeyRound class="h-3.5 w-3.5" aria-hidden="true" />
                Gerät registrieren
              </RouterLink>
              <RouterLink
                to="/catalog"
                class="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-3.5 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
              >
                <Package class="h-3.5 w-3.5" aria-hidden="true" />
                Paket anlegen
              </RouterLink>
            </template>
          </EmptyState>
        </div>

        <div v-else class="overflow-x-auto">
          <table class="min-w-full divide-y divide-slate-200 text-sm dark:divide-slate-800">
            <thead
              class="bg-slate-50 text-left text-xs font-semibold uppercase tracking-wider text-slate-500 dark:bg-slate-800/70 dark:text-slate-400"
            >
              <tr>
                <th class="px-4 py-3">Gerät</th>
                <th class="px-4 py-3">Typ / Aktion</th>
                <th class="px-4 py-3">Status</th>
                <th class="px-4 py-3">Zeitpunkt</th>
                <th class="px-4 py-3 text-right">Log</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-slate-100 dark:divide-slate-800/70">
              <tr
                v-for="job in summary.recent_jobs"
                :key="job.id"
                class="hover:bg-slate-50/80 dark:hover:bg-slate-800/50"
              >
                <td class="px-4 py-3 font-medium text-slate-900 dark:text-slate-100">
                  {{ job.device_hostname || job.hostname || job.device_id }}
                </td>
                <td class="px-4 py-3 text-xs text-slate-700 dark:text-slate-300">
                  <span class="font-semibold">{{ job.job_type }}</span> ·
                  <span class="font-mono">{{ job.action }}</span>
                </td>
                <td class="px-4 py-3">
                  <StatusBadge variant="job" :value="job.status" />
                </td>
                <td class="px-4 py-3 text-xs text-slate-500 dark:text-slate-400">
                  {{ formatDateTime(job.completed_at || job.created_at) }}
                </td>
                <td class="px-4 py-3 text-right">
                  <button
                    type="button"
                    aria-label="Auftragsprotokoll anzeigen"
                    class="inline-flex items-center gap-1 rounded-md border border-slate-200 px-2.5 py-1 text-xs font-medium text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
                    @click="openJobLog(job)"
                  >
                    <Terminal class="h-3.5 w-3.5" aria-hidden="true" />
                    Details
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>

    <JobLogModal
      :open="Boolean(selectedJobId)"
      :job-id="selectedJobId"
      :initial-job="selectedJobInitial"
      @close="selectedJobId = null"
    />
  </div>
</template>
