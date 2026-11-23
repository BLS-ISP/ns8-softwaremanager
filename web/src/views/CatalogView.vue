<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import {
  Edit2,
  Package,
  Plus,
  RefreshCw,
  Rocket,
  Search,
  ShieldCheck,
  Trash2,
} from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type { CatalogItem } from '@/api/types'
import { useToast } from '@/composables/useToast'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import EmptyState from '@/components/EmptyState.vue'
import OsBadge from '@/components/OsBadge.vue'
import PackageUploadModal from '@/components/PackageUploadModal.vue'
import { formatBytes, formatSourceType } from '@/utils/format'

const router = useRouter()
const toast = useToast()

const catalog = ref<CatalogItem[]>([])
const loading = ref(false)
const osFilter = ref('')
const sourceFilter = ref('')
const search = ref('')

const modalOpen = ref(false)
const editingItem = ref<CatalogItem | null>(null)
const pendingDeleteItem = ref<CatalogItem | null>(null)
const deleting = ref(false)

async function loadCatalog() {
  loading.value = true
  try {
    catalog.value = await adminApi.listCatalog({
      os_family: osFilter.value || undefined,
      source_type: sourceFilter.value || undefined,
      search: search.value.trim() || undefined,
    })
  } catch (err) {
    toast.error('Software-Katalog konnte nicht geladen werden', err instanceof Error ? err.message : undefined)
  } finally {
    loading.value = false
  }
}

watch([osFilter, sourceFilter], () => {
  loadCatalog()
})

function openCreate() {
  editingItem.value = null
  modalOpen.value = true
}

function openEdit(item: CatalogItem) {
  editingItem.value = item
  modalOpen.value = true
}

function onSaved(item: CatalogItem) {
  toast.success('Katalog-Paket gespeichert', `${item.name} (${item.version}) wurde aktualisiert.`)
  loadCatalog()
}

function requestDelete(item: CatalogItem) {
  pendingDeleteItem.value = item
}

async function confirmDelete() {
  if (!pendingDeleteItem.value) return
  deleting.value = true
  const target = pendingDeleteItem.value
  try {
    await adminApi.deleteCatalogItem(target.id)
    catalog.value = catalog.value.filter((c) => c.id !== target.id)
    toast.success('Paket aus Katalog gelöscht', `${target.name} (${target.version}) wurde entfernt.`)
    pendingDeleteItem.value = null
  } catch (err) {
    toast.error('Löschen fehlgeschlagen', err instanceof Error ? err.message : undefined)
  } finally {
    deleting.value = false
  }
}

function goToDeploy(item: CatalogItem) {
  router.push({ path: '/deployments', query: { package_id: item.id } })
}

function monogram(name: string): string {
  const cleaned = (name || '').trim()
  if (!cleaned) return 'SW'
  const parts = cleaned.split(/\s+/).filter(Boolean)
  if (parts.length >= 2) {
    return `${parts[0][0]}${parts[1][0]}`.toUpperCase()
  }
  return cleaned.slice(0, 2).toUpperCase()
}

onMounted(() => {
  loadCatalog()
})
</script>

<template>
  <div class="space-y-6">
    <div class="flex flex-col justify-between gap-4 sm:flex-row sm:items-center">
      <div>
        <h1 class="text-2xl font-bold tracking-tight text-slate-900 dark:text-slate-50">
          Software-Katalog &amp; Paket-Repository
        </h1>
        <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
          Native Paketmanager (WinGet, APT, DNF, Flatpak, Homebrew) und eigenes NS8-Repository mit SHA-256 &amp; Detection Rules
        </p>
      </div>
      <div class="flex items-center gap-2">
        <button
          type="button"
          data-testid="open-catalog-modal-btn"
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white shadow-sm hover:bg-sky-700"
          @click="openCreate"
        >
          <Plus class="h-4 w-4" aria-hidden="true" />
          Paket hinzufügen / Upload
        </button>
        <button
          type="button"
          aria-label="Katalog aktualisieren"
          title="Katalog aktualisieren"
          class="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-3 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
          @click="loadCatalog"
        >
          <RefreshCw class="h-4 w-4" :class="{ 'animate-spin': loading }" />
        </button>
      </div>
    </div>

    <!-- Filters -->
    <div
      class="grid grid-cols-1 gap-3 rounded-xl border border-slate-200 bg-white p-4 shadow-xs sm:grid-cols-3 dark:border-slate-800 dark:bg-slate-900"
    >
      <form class="relative" @submit.prevent="loadCatalog">
        <Search class="pointer-events-none absolute left-3 top-2.5 h-4 w-4 text-slate-400" aria-hidden="true" />
        <input
          v-model="search"
          aria-label="Katalog durchsuchen"
          type="text"
          placeholder="Name, Hersteller oder Paket-ID suchen…"
          class="w-full rounded-lg border border-slate-300 py-2 pl-9 pr-3 text-xs dark:border-slate-700 dark:bg-slate-950"
          @change="loadCatalog"
        />
      </form>

      <select
        v-model="osFilter"
        data-testid="catalog-os-filter"
        aria-label="Nach Plattform filtern"
        class="rounded-lg border border-slate-300 bg-white px-3 py-2 text-xs font-medium text-slate-700 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-200"
      >
        <option value="">Alle Plattformen</option>
        <option value="windows">Windows</option>
        <option value="linux">Linux</option>
        <option value="macos">macOS (Apple Silicon &amp; Intel)</option>
      </select>

      <select
        v-model="sourceFilter"
        data-testid="catalog-source-filter"
        aria-label="Nach Paketquelle filtern"
        class="rounded-lg border border-slate-300 bg-white px-3 py-2 text-xs font-medium text-slate-700 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-200"
      >
        <option value="">Alle Paket-Quellen</option>
        <option value="winget">WinGet</option>
        <option value="apt">APT</option>
        <option value="dnf">DNF</option>
        <option value="flatpak">Flatpak</option>
        <option value="brew_cask">Homebrew Cask (macOS .app)</option>
        <option value="brew">Homebrew Formula (macOS CLI)</option>
        <option value="custom">Eigenes NS8-Paket (.msi/.exe/.deb/.rpm/.pkg/.dmg)</option>
      </select>
    </div>

    <!-- Empty State -->
    <div v-if="catalog.length === 0 && !loading">
      <EmptyState
        :icon="Package"
        title="Software-Katalog ist noch leer"
        description="Fügen Sie ein WinGet-, APT-, DNF-, Flatpak- oder Homebrew-Paket hinzu oder laden Sie einen eigenen Installer (.msi, .exe, .deb, .rpm, .pkg, .dmg) in das NS8-Repository hoch."
      >
        <template #actions>
          <button
            type="button"
            class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white hover:bg-sky-700"
            @click="openCreate"
          >
            <Plus class="h-4 w-4" aria-hidden="true" />
            Erstes Software-Paket anlegen
          </button>
        </template>
      </EmptyState>
    </div>

    <!-- Catalog Cards Grid -->
    <div v-else class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
      <div
        v-for="item in catalog"
        :key="item.id"
        :data-testid="`catalog-card-${item.id}`"
        class="flex flex-col justify-between rounded-xl border border-slate-200 bg-white p-5 shadow-xs transition hover:border-slate-300 dark:border-slate-800 dark:bg-slate-900 dark:hover:border-slate-700"
      >
        <div>
          <div class="flex items-start justify-between gap-3">
            <div class="flex min-w-0 items-center gap-3">
              <img
                v-if="item.icon_url"
                :src="item.icon_url"
                :alt="item.name"
                class="h-11 w-11 shrink-0 rounded-xl border border-slate-200 object-contain p-1 dark:border-slate-700 dark:bg-slate-800"
              />
              <div
                v-else
                class="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-cyan-500/15 to-blue-600/15 text-xs font-bold tracking-wider text-sky-700 ring-1 ring-sky-500/20 dark:from-cyan-500/20 dark:to-blue-500/20 dark:text-cyan-300 dark:ring-cyan-400/25"
              >
                {{ monogram(item.name) }}
              </div>
              <div class="min-w-0">
                <h3 class="truncate text-sm font-bold text-slate-900 dark:text-slate-100">
                  {{ item.name }}
                </h3>
                <p class="truncate text-xs text-slate-500 dark:text-slate-400">
                  {{ item.vendor || 'Standard-Repository' }} ·
                  <span class="font-mono font-semibold text-slate-700 dark:text-slate-300">v{{ item.version }}</span>
                </p>
              </div>
            </div>
            <OsBadge :os-family="item.os_family" />
          </div>

          <p class="mt-3 line-clamp-2 text-xs text-slate-600 dark:text-slate-300">
            {{ item.description || 'Keine Beschreibung hinterlegt.' }}
          </p>

          <div class="mt-3 flex flex-wrap items-center gap-1.5 text-[11px]">
            <span class="rounded-md bg-slate-100 px-2 py-0.5 font-semibold text-slate-700 dark:bg-slate-800 dark:text-slate-300">
              {{ formatSourceType(item.source_type) }}
            </span>
            <span class="truncate rounded-md bg-sky-50 px-2 py-0.5 font-mono text-sky-800 dark:bg-sky-500/15 dark:text-sky-300">
              {{ item.package_ref }}
            </span>
            <span
              v-if="item.file_size"
              class="rounded-md bg-slate-100 px-2 py-0.5 text-slate-600 dark:bg-slate-800 dark:text-slate-400"
            >
              {{ formatBytes(item.file_size) }}
            </span>
          </div>

          <div
            v-if="item.sha256"
            class="mt-2.5 flex items-center gap-1.5 truncate rounded bg-emerald-50 px-2 py-1 font-mono text-[10px] text-emerald-800 dark:bg-emerald-500/15 dark:text-emerald-300"
            :title="item.sha256"
          >
            <ShieldCheck class="h-3.5 w-3.5 shrink-0 text-emerald-600 dark:text-emerald-400" aria-hidden="true" />
            SHA-256: {{ item.sha256.slice(0, 20) }}…
          </div>
        </div>

        <div class="mt-4 flex items-center justify-between border-t border-slate-100 pt-3 dark:border-slate-800">
          <button
            type="button"
            class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-3 py-1.5 text-xs font-semibold text-white hover:bg-sky-700"
            @click="goToDeploy(item)"
          >
            <Rocket class="h-3.5 w-3.5" aria-hidden="true" />
            Bereitstellen
          </button>

          <div class="flex items-center gap-1">
            <button
              type="button"
              :aria-label="`Paket ${item.name} bearbeiten`"
              title="Paket bearbeiten / Installer hochladen"
              class="rounded p-1.5 text-slate-500 hover:bg-slate-100 hover:text-slate-800 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100"
              @click="openEdit(item)"
            >
              <Edit2 class="h-4 w-4" />
            </button>
            <button
              type="button"
              :data-testid="`delete-catalog-${item.id}`"
              :aria-label="`Paket ${item.name} löschen`"
              title="Aus Katalog löschen"
              class="rounded p-1.5 text-rose-500 hover:bg-rose-50 dark:text-rose-400 dark:hover:bg-rose-500/15"
              @click="requestDelete(item)"
            >
              <Trash2 class="h-4 w-4" />
            </button>
          </div>
        </div>
      </div>
    </div>

    <PackageUploadModal
      :open="modalOpen"
      :edit-item="editingItem"
      @close="modalOpen = false"
      @saved="onSaved"
    />

    <ConfirmDialog
      :open="Boolean(pendingDeleteItem)"
      variant="danger"
      :title="`Software-Paket „${pendingDeleteItem?.name}“ löschen?`"
      :message="`Das Paket (${pendingDeleteItem?.version || ''}) sowie eventuell im NS8-Repository gespeicherte Installationsdateien werden dauerhaft entfernt.`"
      confirm-label="Paket löschen"
      :loading="deleting"
      @cancel="pendingDeleteItem = null"
      @confirm="confirmDelete"
    />
  </div>
</template>
