<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { Clock, Package, Plus, RefreshCw, Rocket, Store, Trash2 } from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type { CatalogItem, Deployment, Device, DeviceGroup } from '@/api/types'
import { useToast } from '@/composables/useToast'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import DeploymentCreateModal from '@/components/DeploymentCreateModal.vue'
import EmptyState from '@/components/EmptyState.vue'
import { formatDateTime, formatDeploymentMode } from '@/utils/format'

const route = useRoute()
const toast = useToast()

const deployments = ref<Deployment[]>([])
const catalog = ref<CatalogItem[]>([])
const groups = ref<DeviceGroup[]>([])
const devices = ref<Device[]>([])
const loading = ref(false)

const modalOpen = ref(false)
const preselectedPkg = ref<string | null>((route.query.package_id as string) || null)
const pendingDeleteDep = ref<Deployment | null>(null)
const deleting = ref(false)

async function loadAll() {
  loading.value = true
  try {
    const [depList, catList, grpList, devList] = await Promise.all([
      adminApi.listDeployments(),
      adminApi.listCatalog(),
      adminApi.listGroups(),
      adminApi.listDevices(),
    ])
    deployments.value = depList
    catalog.value = catList
    groups.value = grpList
    devices.value = devList
    if (preselectedPkg.value) {
      modalOpen.value = true
    }
  } catch (err) {
    toast.error('Bereitstellungen konnten nicht geladen werden', err instanceof Error ? err.message : undefined)
  } finally {
    loading.value = false
  }
}

function getStats(dep: Deployment) {
  const installed = dep.stats?.installed ?? dep.installed_count ?? 0
  const pending = dep.stats?.pending ?? dep.pending_count ?? 0
  const failed = dep.stats?.failed ?? dep.failed_count ?? 0
  const total = dep.stats?.total ?? dep.total_targets ?? installed + pending + failed
  const pct = total > 0 ? Math.round((installed / total) * 100) : 0
  return { total, installed, pending, failed, pct }
}

function getPackageName(dep: Deployment): string {
  if (dep.package_name) {
    return dep.package_version ? `${dep.package_name} (${dep.package_version})` : dep.package_name
  }
  const found = catalog.value.find((c) => c.id === dep.package_id)
  return found ? `${found.name} (${found.version})` : dep.package_id
}

function requestDelete(dep: Deployment) {
  pendingDeleteDep.value = dep
}

async function confirmDelete() {
  if (!pendingDeleteDep.value) return
  deleting.value = true
  const target = pendingDeleteDep.value
  try {
    await adminApi.deleteDeployment(target.id)
    deployments.value = deployments.value.filter((d) => d.id !== target.id)
    toast.success('Bereitstellung entfernt', `${getPackageName(target)} wurde deaktiviert.`)
    pendingDeleteDep.value = null
  } catch (err) {
    toast.error('Löschen fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    deleting.value = false
  }
}

function onCreated() {
  preselectedPkg.value = null
  toast.success('Bereitstellung aktiviert', 'Zielsysteme erhalten nun signierte Manifeste bzw. Aufträge.')
  loadAll()
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
          Software-Bereitstellungen (Deployments)
        </h1>
        <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
          Steuern Sie Pflicht-Rollouts, Deinstallationen und Self-Service-Freigaben im lokalen Software Center
        </p>
      </div>
      <div class="flex items-center gap-2">
        <button
          type="button"
          data-testid="open-deployment-modal-btn"
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white shadow-sm hover:bg-sky-700"
          @click="preselectedPkg = null; modalOpen = true"
        >
          <Plus class="h-4 w-4" aria-hidden="true" />
          Neue Bereitstellung
        </button>
        <button
          type="button"
          aria-label="Bereitstellungen aktualisieren"
          title="Bereitstellungen aktualisieren"
          class="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-3 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
          @click="loadAll"
        >
          <RefreshCw class="h-4 w-4" :class="{ 'animate-spin': loading }" />
        </button>
      </div>
    </div>

    <!-- Empty State -->
    <div v-if="deployments.length === 0 && !loading">
      <EmptyState
        :icon="Rocket"
        title="Noch keine aktiven Bereitstellungen"
        description="Weisen Sie ein Katalog-Paket als Pflicht-Installation (Required) oder als Self-Service-Angebot (Available) für das Endbenutzer-Software-Center zu."
      >
        <template #actions>
          <button
            v-if="catalog.length > 0"
            type="button"
            class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white hover:bg-sky-700"
            @click="preselectedPkg = null; modalOpen = true"
          >
            <Plus class="h-4 w-4" aria-hidden="true" />
            Bereitstellung erstellen
          </button>
          <RouterLink
            v-else
            to="/catalog"
            class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white hover:bg-sky-700"
          >
            <Package class="h-4 w-4" aria-hidden="true" />
            Zuerst Paket im Katalog anlegen
          </RouterLink>
        </template>
      </EmptyState>
    </div>

    <!-- Deployments Table -->
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
              <th class="px-4 py-3.5">Software-Paket</th>
              <th class="px-4 py-3.5">Modus</th>
              <th class="px-4 py-3.5">Ziele</th>
              <th class="px-4 py-3.5">Rollout-Fortschritt</th>
              <th class="px-4 py-3.5">Wartungsfenster</th>
              <th class="px-4 py-3.5">Erstellt</th>
              <th class="px-4 py-3.5 text-right">Aktion</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-100 dark:divide-slate-800/70">
            <tr
              v-for="dep in deployments"
              :key="dep.id"
              :data-testid="`deployment-row-${dep.id}`"
              class="hover:bg-slate-50/80 dark:hover:bg-slate-800/50"
            >
              <td class="px-4 py-3.5 font-semibold text-slate-900 dark:text-slate-100">
                {{ getPackageName(dep) }}
              </td>

              <td class="px-4 py-3.5">
                <span
                  class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-semibold"
                  :class="
                    dep.mode === 'required'
                      ? 'bg-sky-50 text-sky-700 ring-1 ring-inset ring-sky-600/20 dark:bg-sky-500/15 dark:text-sky-300 dark:ring-sky-400/30'
                      : dep.mode === 'available'
                        ? 'bg-cyan-50 text-cyan-800 ring-1 ring-inset ring-cyan-600/20 dark:bg-cyan-500/15 dark:text-cyan-300 dark:ring-cyan-400/30'
                        : 'bg-rose-50 text-rose-700 ring-1 ring-inset ring-rose-600/20 dark:bg-rose-500/15 dark:text-rose-300 dark:ring-rose-400/30'
                  "
                >
                  <Rocket v-if="dep.mode === 'required'" class="h-3 w-3" aria-hidden="true" />
                  <Store v-else-if="dep.mode === 'available'" class="h-3 w-3" aria-hidden="true" />
                  <Trash2 v-else class="h-3 w-3" aria-hidden="true" />
                  {{ formatDeploymentMode(dep.mode) }}
                </span>
              </td>

              <td class="px-4 py-3.5 text-xs text-slate-600 dark:text-slate-300">
                {{ dep.target_group_ids?.length || 0 }} Gruppe(n),
                {{ dep.target_device_ids?.length || 0 }} Einzelgerät(e)
              </td>

              <td class="px-4 py-3.5">
                <div class="w-48 space-y-1">
                  <div class="flex justify-between text-[11px] font-medium">
                    <span class="text-emerald-700 dark:text-emerald-400">{{ getStats(dep).installed }} installiert</span>
                    <span class="text-slate-500 dark:text-slate-400">{{ getStats(dep).pending }} offen</span>
                    <span v-if="getStats(dep).failed > 0" class="text-rose-600 dark:text-rose-400">
                      {{ getStats(dep).failed }} Fehler
                    </span>
                  </div>
                  <div class="h-2 w-full overflow-hidden rounded-full bg-slate-100 dark:bg-slate-800">
                    <div
                      class="h-full rounded-full bg-emerald-500"
                      :style="{ width: `${getStats(dep).pct}%` }"
                    />
                  </div>
                </div>
              </td>

              <td class="px-4 py-3.5 text-xs text-slate-600 dark:text-slate-300">
                <span v-if="dep.respect_maintenance_window" class="inline-flex items-center gap-1 text-slate-700 dark:text-slate-200">
                  <Clock class="h-3.5 w-3.5 text-sky-600 dark:text-sky-400" aria-hidden="true" />
                  Aktiv
                </span>
                <span v-else class="text-slate-400">Sofort</span>
              </td>

              <td class="px-4 py-3.5 text-xs text-slate-500 dark:text-slate-400">
                {{ formatDateTime(dep.created_at) }}
              </td>

              <td class="px-4 py-3.5 text-right">
                <button
                  type="button"
                  :data-testid="`delete-deployment-${dep.id}`"
                  :aria-label="`Bereitstellung ${getPackageName(dep)} löschen`"
                  title="Bereitstellung löschen"
                  class="rounded-md border border-rose-200 bg-rose-50 p-1.5 text-rose-600 hover:bg-rose-100 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-400 dark:hover:bg-rose-500/20"
                  @click="requestDelete(dep)"
                >
                  <Trash2 class="h-3.5 w-3.5" />
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <DeploymentCreateModal
      :open="modalOpen"
      :catalog="catalog"
      :groups="groups"
      :devices="devices"
      :preselected-package-id="preselectedPkg"
      @close="modalOpen = false"
      @created="onCreated"
    />

    <ConfirmDialog
      :open="Boolean(pendingDeleteDep)"
      variant="danger"
      :title="`Bereitstellung für „${pendingDeleteDep ? getPackageName(pendingDeleteDep) : ''}“ löschen?`"
      message="Die Zuweisung wird aufgehoben. Bereits installierte Anwendungen auf den Zielsystemen bleiben unberührt, erhalten über dieses Deployment jedoch keine weiteren Aufträge."
      confirm-label="Bereitstellung löschen"
      :loading="deleting"
      @cancel="pendingDeleteDep = null"
      @confirm="confirmDelete"
    />
  </div>
</template>
