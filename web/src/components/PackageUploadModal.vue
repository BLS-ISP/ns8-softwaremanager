<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { CheckCircle2, FileUp, PackagePlus, ShieldCheck, Sparkles, X } from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type { CatalogCreatePayload, CatalogItem, OsFamily, SourceType } from '@/api/types'
import { formatBytes } from '@/utils/format'

type DetectionTypeOption =
  | 'none'
  | 'file'
  | 'registry'
  | 'package'
  | 'app_bundle'
  | 'pkgutil'
  | 'brew'
  | 'brew_cask'

const props = defineProps<{
  open: boolean
  editItem?: CatalogItem | null
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'saved', item: CatalogItem): void
}>()

const activeTab = ref<'general' | 'execution' | 'detection'>('general')

const name = ref('')
const version = ref('')
const vendor = ref('')
const category = ref('Produktivität')
const description = ref('')
const iconUrl = ref('')
const osFamily = ref<OsFamily>('windows')
const sourceType = ref<SourceType>('winget')
const packageRef = ref('')

const installArgs = ref('')
const uninstallArgs = ref('')
const allowUninstall = ref(true)
const blockedProcessesInput = ref('')
const returnCodesZero = ref('success')
const returnCodes3010 = ref('reboot_required')
const returnCodes1641 = ref('reboot_required')

const detectionType = ref<DetectionTypeOption>('none')
const detectionPath = ref('')
const detectionBundleId = ref('')
const detectionValueName = ref('')
const detectionVersion = ref('')

const selectedFile = ref<File | null>(null)
const uploading = ref(false)
const uploadPercent = ref(0)
const uploadChunkStatus = ref('')
const uploadedSha256 = ref<string | null>(null)
const saving = ref(false)
const validationError = ref<string | null>(null)

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.open && !saving.value && !uploading.value) {
    emit('close')
  }
}

const availableSources = computed<{ value: SourceType; label: string }[]>(() => {
  if (osFamily.value === 'windows') {
    return [
      { value: 'winget', label: 'WinGet (Microsoft Paketmanager)' },
      { value: 'custom', label: 'Eigenes Paket (.msi / .exe / .zip / .ps1)' },
    ]
  }
  if (osFamily.value === 'macos') {
    return [
      { value: 'brew_cask', label: 'Homebrew Cask (Desktop-App .app)' },
      { value: 'brew', label: 'Homebrew Formula (CLI-Paket)' },
      { value: 'custom', label: 'Eigenes macOS-Paket (.pkg, .dmg, .app.zip, .sh)' },
    ]
  }
  return [
    { value: 'apt', label: 'APT (Debian / Ubuntu / Mint)' },
    { value: 'dnf', label: 'DNF (RHEL / Rocky / Alma / Fedora)' },
    { value: 'flatpak', label: 'Flatpak (Flathub Desktop-Apps)' },
    { value: 'custom', label: 'Eigenes Paket (.deb / .rpm / .sh)' },
  ]
})

const availableDetectionTypes = computed<{ value: DetectionTypeOption; label: string }[]>(() => {
  if (osFamily.value === 'macos') {
    return [
      { value: 'none', label: 'Automatisch (Homebrew / Paketmanager)' },
      { value: 'app_bundle', label: 'App-Bundle unter /Applications/*.app' },
      { value: 'pkgutil', label: 'macOS Installer-Receipt (pkgutil)' },
      { value: 'brew_cask', label: 'Homebrew Cask (brew_cask)' },
      { value: 'brew', label: 'Homebrew Formula (brew)' },
      { value: 'file', label: 'Dateipfad & Version' },
    ]
  }
  if (osFamily.value === 'windows') {
    return [
      { value: 'none', label: 'Automatisch (Paketmanager)' },
      { value: 'file', label: 'Dateipfad & Version' },
      { value: 'registry', label: 'Windows Registry-Schlüssel' },
      { value: 'package', label: 'Installierter Paketname' },
    ]
  }
  return [
    { value: 'none', label: 'Automatisch (Paketmanager)' },
    { value: 'file', label: 'Dateipfad & Version' },
    { value: 'package', label: 'Installierter Paketname' },
  ]
})

const fileAccept = computed(() =>
  osFamily.value === 'macos'
    ? '.pkg,.dmg,.zip,.sh'
    : '.msi,.exe,.zip,.deb,.rpm,.pkg,.dmg,.ps1,.sh,.bin,*',
)

watch(osFamily, (newOs) => {
  const validSource = availableSources.value.some((s) => s.value === sourceType.value)
  if (!validSource) {
    sourceType.value = newOs === 'windows' ? 'winget' : newOs === 'macos' ? 'brew_cask' : 'apt'
  }
  const validDet = availableDetectionTypes.value.some((d) => d.value === detectionType.value)
  if (!validDet) {
    detectionType.value = 'none'
  }
})

watch(
  () => [props.open, props.editItem],
  ([isOpen]) => {
    if (typeof window !== 'undefined') {
      if (isOpen) {
        window.addEventListener('keydown', onKeyDown)
      } else {
        window.removeEventListener('keydown', onKeyDown)
      }
    }
    if (!isOpen) return
    activeTab.value = 'general'
    validationError.value = null
    selectedFile.value = null
    uploading.value = false
    uploadPercent.value = 0
    uploadChunkStatus.value = ''

    const item = props.editItem
    if (item) {
      name.value = item.name
      version.value = item.version
      vendor.value = item.vendor || ''
      category.value = item.category || 'Produktivität'
      description.value = item.description || ''
      iconUrl.value = item.icon_url || ''
      osFamily.value = item.os_family
      sourceType.value = item.source_type
      packageRef.value = item.package_ref
      installArgs.value = item.install_args || ''
      uninstallArgs.value = item.uninstall_args || ''
      allowUninstall.value = item.allow_uninstall
      blockedProcessesInput.value = (item.blocked_processes || []).join(', ')
      uploadedSha256.value = item.sha256 || null

      const rule = (item.detection_rule || {}) as Record<string, string>
      detectionType.value = (rule.type as DetectionTypeOption) || 'none'
      detectionPath.value =
        rule.path ||
        rule.key_path ||
        rule.package_name ||
        rule.package_id ||
        rule.package ||
        ''
      detectionBundleId.value = rule.bundle_id || ''
      detectionValueName.value = rule.value_name || ''
      detectionVersion.value = rule.min_version || rule.version || rule.expected_version || ''
    } else {
      name.value = ''
      version.value = ''
      vendor.value = ''
      category.value = 'Produktivität'
      description.value = ''
      iconUrl.value = ''
      osFamily.value = 'windows'
      sourceType.value = 'winget'
      packageRef.value = ''
      installArgs.value = ''
      uninstallArgs.value = ''
      allowUninstall.value = true
      blockedProcessesInput.value = ''
      uploadedSha256.value = null
      detectionType.value = 'none'
      detectionPath.value = ''
      detectionBundleId.value = ''
      detectionValueName.value = ''
      detectionVersion.value = ''
    }
  },
  { immediate: true },
)

onBeforeUnmount(() => {
  if (typeof window !== 'undefined') {
    window.removeEventListener('keydown', onKeyDown)
  }
})

function onFileSelected(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0] || null
  selectedFile.value = file
  if (file && !packageRef.value.trim()) {
    packageRef.value = file.name
  }
}

function applyMacosDetectionPreset(preset: 'app_bundle' | 'pkgutil') {
  detectionType.value = preset
  if (preset === 'app_bundle' && !detectionPath.value.trim()) {
    const cleanName = (name.value || 'AppName').trim().replace(/\.app$/i, '')
    detectionPath.value = `/Applications/${cleanName}.app`
  } else if (preset === 'pkgutil' && !detectionPath.value.trim()) {
    const slug = (name.value || 'app')
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
    detectionPath.value = `com.vendor.${slug || 'pkg'}`
  }
}

function validateForm(): string | null {
  if (!name.value.trim()) {
    return 'Bitte geben Sie einen Anzeigenamen für das Software-Paket ein.'
  }
  if (!version.value.trim()) {
    return 'Bitte geben Sie eine gültige Versionsnummer an (z. B. 24.08).'
  }
  if (sourceType.value !== 'custom' && !packageRef.value.trim()) {
    return 'Bitte geben Sie die Paket-ID / Referenz des Paketmanagers an (z. B. Mozilla.Firefox oder firefox).'
  }
  if (
    sourceType.value === 'custom' &&
    !props.editItem &&
    !selectedFile.value &&
    !packageRef.value.trim()
  ) {
    return 'Für ein eigenes Paket muss eine Installationsdatei ausgewählt oder eine Paket-Referenz angegeben werden.'
  }
  if (detectionType.value === 'brew' || detectionType.value === 'brew_cask') {
    if (!detectionPath.value.trim() && !packageRef.value.trim()) {
      return 'Bitte geben Sie den Homebrew-Paketnamen für die Erkennungsregel an.'
    }
  } else if (detectionType.value !== 'none' && !detectionPath.value.trim()) {
    return 'Bitte geben Sie den Pfad, Bundle-Pfad, Receipt-Schlüssel oder Paketnamen für die Erkennungsregel an.'
  }
  return null
}

function buildPayload(): CatalogCreatePayload {
  const blocked = blockedProcessesInput.value
    .split(',')
    .map((p) => p.trim())
    .filter(Boolean)

  const returnCodes: Record<string, string> = {
    '0': returnCodesZero.value || 'success',
    '3010': returnCodes3010.value || 'reboot_required',
    '1641': returnCodes1641.value || 'reboot_required',
  }

  let detectionRule: Record<string, unknown> = {}
  if (detectionType.value === 'none') {
    if (
      osFamily.value === 'macos' &&
      (sourceType.value === 'brew' || sourceType.value === 'brew_cask') &&
      packageRef.value.trim()
    ) {
      detectionRule = {
        type: sourceType.value,
        package: packageRef.value.trim(),
        ...(detectionVersion.value.trim() ? { min_version: detectionVersion.value.trim() } : {}),
      }
    }
  } else if (detectionType.value === 'app_bundle') {
    detectionRule = {
      type: 'app_bundle',
      path: detectionPath.value.trim(),
      min_version: detectionVersion.value.trim() || version.value.trim(),
      ...(detectionBundleId.value.trim() ? { bundle_id: detectionBundleId.value.trim() } : {}),
    }
  } else if (detectionType.value === 'pkgutil') {
    detectionRule = {
      type: 'pkgutil',
      package_id: detectionPath.value.trim(),
      min_version: detectionVersion.value.trim() || version.value.trim(),
    }
  } else if (detectionType.value === 'brew' || detectionType.value === 'brew_cask') {
    detectionRule = {
      type: detectionType.value,
      package: detectionPath.value.trim() || packageRef.value.trim(),
      ...(detectionVersion.value.trim() ? { min_version: detectionVersion.value.trim() } : {}),
    }
  } else if (detectionType.value === 'file') {
    detectionRule = {
      type: 'file',
      path: detectionPath.value.trim(),
      version: detectionVersion.value.trim() || version.value.trim(),
    }
  } else if (detectionType.value === 'registry') {
    detectionRule = {
      type: 'registry',
      path: detectionPath.value.trim(),
      value_name: detectionValueName.value.trim() || 'DisplayVersion',
      version: detectionVersion.value.trim() || version.value.trim(),
    }
  } else if (detectionType.value === 'package') {
    detectionRule = {
      type: 'package',
      package_name: detectionPath.value.trim(),
      version: detectionVersion.value.trim() || version.value.trim(),
    }
  }

  return {
    name: name.value.trim(),
    version: version.value.trim(),
    vendor: vendor.value.trim(),
    description: description.value.trim(),
    category: category.value.trim(),
    icon_url: iconUrl.value.trim(),
    os_family: osFamily.value,
    source_type: sourceType.value,
    package_ref: packageRef.value.trim() || selectedFile.value?.name || 'custom-package',
    install_args: installArgs.value.trim(),
    uninstall_args: uninstallArgs.value.trim(),
    blocked_processes: blocked,
    allow_uninstall: allowUninstall.value,
    return_codes: returnCodes,
    detection_rule: detectionRule,
  }
}

async function handleSubmit() {
  validationError.value = validateForm()
  if (validationError.value) return

  saving.value = true
  try {
    const payload = buildPayload()
    let savedItem: CatalogItem
    if (props.editItem) {
      savedItem = await adminApi.updateCatalogItem(props.editItem.id, payload)
    } else {
      savedItem = await adminApi.createCatalogItem(payload)
    }

    if (sourceType.value === 'custom' && selectedFile.value) {
      uploading.value = true
      uploadPercent.value = 0
      const chunkRes = await adminApi.uploadCatalogFileChunked(
        savedItem.id,
        selectedFile.value,
        (percent, current, total) => {
          uploadPercent.value = percent
          uploadChunkStatus.value = `Chunk ${current} von ${total} (${percent}%)`
        },
      )
      if (chunkRes.sha256) {
        uploadedSha256.value = chunkRes.sha256
      }
      if (chunkRes.catalog_item) {
        savedItem = chunkRes.catalog_item
      } else {
        savedItem = {
          ...savedItem,
          sha256: chunkRes.sha256 ?? savedItem.sha256,
          file_size: chunkRes.file_size ?? selectedFile.value.size,
          filename: selectedFile.value.name,
        }
      }
      uploading.value = false
    }

    emit('saved', savedItem)
    emit('close')
  } catch (err) {
    uploading.value = false
    validationError.value =
      err instanceof Error ? err.message : 'Fehler beim Speichern des Katalog-Pakets'
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div
    v-if="open"
    role="dialog"
    aria-modal="true"
    :aria-label="editItem ? 'Software-Paket bearbeiten' : 'Neues Software-Paket anlegen'"
    class="!mt-0 fixed inset-0 z-50 flex items-center justify-center bg-slate-950/65 p-4 backdrop-blur-sm"
    @click.self="emit('close')"
  >
    <div
      class="flex max-h-[92vh] w-full max-w-3xl flex-col overflow-hidden rounded-xl border border-slate-200 bg-white shadow-2xl dark:border-slate-800 dark:bg-slate-900"
    >
      <!-- Header -->
      <div class="flex items-center justify-between border-b border-slate-800 bg-slate-900 px-6 py-4 text-white dark:bg-slate-950">
        <div class="flex items-center gap-3">
          <div class="rounded-lg bg-sky-500/20 p-2 text-sky-400">
            <PackagePlus class="h-5 w-5" aria-hidden="true" />
          </div>
          <div>
            <h3 class="text-base font-semibold">
              {{ editItem ? 'Software-Paket bearbeiten' : 'Neues Software-Paket anlegen' }}
            </h3>
            <p class="text-xs text-slate-400">
              Nativer Paketmanager (WinGet, APT, DNF, Flatpak, Homebrew) oder eigenes NS8-Repository-Paket
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

      <!-- Wizard Step Tabs -->
      <div class="flex overflow-x-auto border-b border-slate-200 bg-slate-50 px-6 dark:border-slate-800 dark:bg-slate-950/50">
        <button
          type="button"
          class="whitespace-nowrap border-b-2 px-4 py-3 text-xs font-semibold transition-colors"
          :class="
            activeTab === 'general'
              ? 'border-sky-600 text-sky-700 dark:border-sky-400 dark:text-sky-400'
              : 'border-transparent text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
          "
          @click="activeTab = 'general'"
        >
          1. Basisdaten &amp; Quelle
        </button>
        <button
          type="button"
          class="whitespace-nowrap border-b-2 px-4 py-3 text-xs font-semibold transition-colors"
          :class="
            activeTab === 'execution'
              ? 'border-sky-600 text-sky-700 dark:border-sky-400 dark:text-sky-400'
              : 'border-transparent text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
          "
          @click="activeTab = 'execution'"
        >
          2. Parameter &amp; Prozess-Sperre
        </button>
        <button
          type="button"
          data-testid="tab-detection-upload"
          class="whitespace-nowrap border-b-2 px-4 py-3 text-xs font-semibold transition-colors"
          :class="
            activeTab === 'detection'
              ? 'border-sky-600 text-sky-700 dark:border-sky-400 dark:text-sky-400'
              : 'border-transparent text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
          "
          @click="activeTab = 'detection'"
        >
          3. Erkennungsregel &amp; Datei-Upload
        </button>
      </div>

      <!-- Form Content -->
      <form class="flex-1 space-y-5 overflow-y-auto p-6" @submit.prevent="handleSubmit">
        <div
          v-if="validationError"
          data-testid="catalog-validation-error"
          class="rounded-lg border border-rose-200 bg-rose-50 px-4 py-3 text-sm font-medium text-rose-700 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-300"
        >
          {{ validationError }}
        </div>

        <!-- TAB 1: GENERAL -->
        <div v-show="activeTab === 'general'" class="space-y-4">
          <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div>
              <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Ziel-Plattform *</label>
              <select
                v-model="osFamily"
                data-testid="catalog-os-family"
                class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
              >
                <option value="windows">Windows (10/11 &amp; Windows Server)</option>
                <option value="linux">Linux (Debian/Ubuntu &amp; RHEL/Rocky/Fedora)</option>
                <option value="macos">macOS (Apple Silicon &amp; Intel)</option>
              </select>
            </div>

            <div>
              <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Paket-Quelle *</label>
              <select
                :key="`src-${osFamily}`"
                v-model="sourceType"
                data-testid="catalog-source-type"
                class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
              >
                <option v-for="src in availableSources" :key="src.value" :value="src.value">
                  {{ src.label }}
                </option>
              </select>
            </div>

            <div>
              <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Anzeigename *</label>
              <input
                v-model="name"
                data-testid="catalog-name"
                type="text"
                placeholder="z. B. Mozilla Firefox ESR"
                class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
              />
            </div>

            <div>
              <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Ziel-Version *</label>
              <input
                v-model="version"
                data-testid="catalog-version"
                type="text"
                placeholder="z. B. 128.4.0"
                class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
              />
            </div>

            <div>
              <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Hersteller / Herausgeber</label>
              <input
                v-model="vendor"
                type="text"
                placeholder="z. B. Mozilla Corporation"
                class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
              />
            </div>

            <div>
              <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Kategorie (Software Center)</label>
              <input
                v-model="category"
                type="text"
                placeholder="z. B. Browser, Produktivität, Entwickler"
                class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
              />
            </div>
          </div>

          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">
              {{
                sourceType === 'custom'
                  ? 'Dateiname / Paket-Referenz'
                  : sourceType === 'brew_cask'
                    ? 'Homebrew Cask Token *'
                    : sourceType === 'brew'
                      ? 'Homebrew Formula Name *'
                      : 'Paket-ID im Paketmanager *'
              }}
            </label>
            <input
              v-model="packageRef"
              data-testid="catalog-package-ref"
              type="text"
              :placeholder="
                sourceType === 'winget'
                  ? 'z. B. Mozilla.Firefox.ESR oder 7zip.7zip'
                  : sourceType === 'brew_cask'
                    ? 'z. B. firefox, google-chrome oder visual-studio-code'
                    : sourceType === 'brew'
                      ? 'z. B. jq, wget oder ripgrep'
                      : sourceType === 'flatpak'
                        ? 'z. B. org.mozilla.firefox'
                        : sourceType === 'custom'
                          ? osFamily === 'macos'
                            ? 'Wird beim Datei-Upload automatisch vorbelegt (z. B. Firefox.dmg oder CorpVPN.pkg)'
                            : 'Wird beim Datei-Upload automatisch vorbelegt (z. B. setup.msi)'
                          : 'z. B. firefox-esr oder vim'
              "
              class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-sm dark:border-slate-700 dark:bg-slate-950"
            />
            <p
              v-if="osFamily === 'macos'"
              class="mt-1 text-xs text-slate-500 dark:text-slate-400"
            >
              <template v-if="sourceType === 'brew_cask' || sourceType === 'brew'">
                Der Agent führt Homebrew automatisch im Kontext des lokalen Konsolen-/Brew-Eigentümers aus (niemals unsicher als <code>root</code>).
              </template>
              <template v-else>
                <code>.pkg</code> wird via <code>installer -pkg -target /</code> installiert; <code>.dmg</code> &amp; <code>.app.zip</code> kopieren <code>.app</code>-Bundles nach <code>/Applications</code> und entfernen das Quarantäne-Attribut.
              </template>
            </p>
          </div>

          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Beschreibung für Endbenutzer</label>
            <textarea
              v-model="description"
              rows="2"
              placeholder="Kurze Beschreibung für die Anzeige im lokalen Software Center..."
              class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
            />
          </div>

          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Icon-URL (optional)</label>
            <input
              v-model="iconUrl"
              type="text"
              placeholder="https://..."
              class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
            />
          </div>
        </div>

        <!-- TAB 2: EXECUTION & PROCESS BLOCKING -->
        <div v-show="activeTab === 'execution'" class="space-y-4">
          <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div>
              <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">
                Silent-Installationsparameter
              </label>
              <input
                v-model="installArgs"
                type="text"
                :placeholder="osFamily === 'macos' ? 'Optional für .sh-Skripte (z. B. --silent)' : 'z. B. /qn /norestart oder /S'"
                class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-sm dark:border-slate-700 dark:bg-slate-950"
              />
              <p class="mt-1 text-xs text-slate-500 dark:text-slate-400">
                <template v-if="osFamily === 'macos'">
                  Bei Homebrew (<code>NONINTERACTIVE=1</code>), <code>.pkg</code> und <code>.dmg</code> erfolgt die Installation vollautomatisch ohne Rückfragen.
                </template>
                <template v-else>
                  Bei WinGet wird <code>--scope machine --silent</code> automatisch durch den Agenten erzwungen.
                </template>
              </p>
            </div>

            <div>
              <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">
                Silent-Deinstallationsbefehl / Parameter
              </label>
              <input
                v-model="uninstallArgs"
                type="text"
                :placeholder="osFamily === 'macos' ? 'z. B. rm -rf /Applications/MyApp.app' : 'z. B. /x {GUID} /qn /norestart'"
                class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-sm dark:border-slate-700 dark:bg-slate-950"
              />
            </div>
          </div>

          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">
              Blockierende Prozesse vor Update prüfen (kommagetrennt)
            </label>
            <input
              v-model="blockedProcessesInput"
              type="text"
              :placeholder="osFamily === 'macos' ? 'z. B. firefox, Slack, Microsoft Word' : 'z. B. firefox.exe, outlook.exe'"
              class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-sm dark:border-slate-700 dark:bg-slate-950"
            />
            <p class="mt-1 text-xs text-slate-500 dark:text-slate-400">
              Läuft einer dieser Prozesse, verschiebt der Agent das Update sicher und informiert den Benutzer im Software Center.
            </p>
          </div>

          <label class="flex items-center gap-3 rounded-lg border border-slate-200 bg-slate-50 p-3.5 dark:border-slate-800 dark:bg-slate-950/60">
            <input
              v-model="allowUninstall"
              type="checkbox"
              class="h-4 w-4 rounded border-slate-300 text-sky-600 focus:ring-sky-500"
            />
            <div>
              <div class="text-sm font-medium text-slate-900 dark:text-slate-100">
                Deinstallation im Endbenutzer-Software-Center erlauben
              </div>
              <div class="text-xs text-slate-500 dark:text-slate-400">
                Wenn deaktiviert, wird der „Deinstallieren“-Button im lokalen Software Center ausgeblendet und per IPC abgewiesen.
              </div>
            </div>
          </label>

          <div class="rounded-lg border border-slate-200 p-4 dark:border-slate-800">
            <div class="mb-2 text-xs font-semibold uppercase tracking-wider text-slate-600 dark:text-slate-400">
              Exit-Code-Auswertung (MSI / Installer Standard)
            </div>
            <div class="grid grid-cols-1 gap-3 text-xs sm:grid-cols-3">
              <div class="rounded bg-emerald-50 p-2.5 text-emerald-800 dark:bg-emerald-500/15 dark:text-emerald-300">
                <span class="font-mono font-bold">0</span> → Erfolgreich installiert
              </div>
              <div class="rounded bg-amber-50 p-2.5 text-amber-800 dark:bg-amber-500/15 dark:text-amber-300">
                <span class="font-mono font-bold">3010</span> → Erfolg + Neustart nötig
              </div>
              <div class="rounded bg-amber-50 p-2.5 text-amber-800 dark:bg-amber-500/15 dark:text-amber-300">
                <span class="font-mono font-bold">1641</span> → Neustart initiiert
              </div>
            </div>
          </div>
        </div>

        <!-- TAB 3: DETECTION RULES & CHUNKED UPLOAD -->
        <div v-show="activeTab === 'detection'" class="space-y-5">
          <!-- Chunked File Upload Box -->
          <div class="rounded-xl border-2 border-dashed border-slate-300 bg-slate-50 p-5 dark:border-slate-700 dark:bg-slate-950/60">
            <div class="flex items-start gap-3">
              <FileUp class="mt-0.5 h-6 w-6 shrink-0 text-sky-600 dark:text-sky-400" aria-hidden="true" />
              <div class="flex-1">
                <h4 class="text-sm font-semibold text-slate-900 dark:text-slate-100">
                  Installationsdatei für lokales NS8-Repository (Chunked Upload)
                </h4>
                <p class="mt-0.5 text-xs text-slate-600 dark:text-slate-400">
                  <template v-if="osFamily === 'macos'">
                    Unterstützt macOS-Installer (<code>.pkg</code>, <code>.dmg</code>, <code>.app.zip</code>, <code>.sh</code>) mit Streaming-Upload, automatischer SHA-256-Prüfsumme und HTTP-Range-Download.
                  </template>
                  <template v-else>
                    Unterstützt große Pakete (.msi, .exe, .zip, .deb, .rpm, .pkg, .dmg, .ps1, .sh) mit Streaming-Upload, automatischer SHA-256-Prüfsumme und HTTP-Range-Download für VPN-Clients.
                  </template>
                </p>

                <input
                  type="file"
                  data-testid="catalog-file-input"
                  :accept="fileAccept"
                  class="mt-3 block w-full text-xs text-slate-600 file:mr-3 file:rounded-lg file:border-0 file:bg-sky-600 file:px-3.5 file:py-2 file:text-xs file:font-semibold file:text-white hover:file:bg-sky-700 dark:text-slate-300"
                  @change="onFileSelected"
                />

                <div v-if="selectedFile" class="mt-2 text-xs font-medium text-slate-700 dark:text-slate-300">
                  Ausgewählt: <span class="font-mono">{{ selectedFile.name }}</span>
                  ({{ formatBytes(selectedFile.size) }})
                </div>

                <!-- Live Chunk Progress Bar -->
                <div v-if="uploading || uploadPercent > 0" class="mt-3 space-y-1">
                  <div class="flex justify-between text-xs font-medium text-sky-700 dark:text-sky-400">
                    <span>{{ uploadChunkStatus || 'Datei wird hochgeladen…' }}</span>
                    <span>{{ uploadPercent }}%</span>
                  </div>
                  <div class="h-2 w-full overflow-hidden rounded-full bg-slate-200 dark:bg-slate-800">
                    <div
                      data-testid="chunk-upload-progress"
                      class="h-full rounded-full bg-sky-600 transition-all duration-200"
                      :style="{ width: `${uploadPercent}%` }"
                    />
                  </div>
                </div>

                <div
                  v-if="uploadedSha256"
                  class="mt-3 flex items-center gap-2 rounded-lg bg-emerald-50 px-3 py-2 text-xs text-emerald-800 dark:bg-emerald-500/15 dark:text-emerald-300"
                >
                  <ShieldCheck class="h-4 w-4 shrink-0 text-emerald-600 dark:text-emerald-400" aria-hidden="true" />
                  <span class="truncate font-mono">SHA-256: {{ uploadedSha256 }}</span>
                </div>
              </div>
            </div>
          </div>

          <!-- Detection Rule Builder -->
          <div class="space-y-3 rounded-xl border border-slate-200 p-4 dark:border-slate-800">
            <div class="flex flex-wrap items-center justify-between gap-2">
              <div>
                <h4 class="text-sm font-semibold text-slate-900 dark:text-slate-100">Erkennungsregel (Detection Rule)</h4>
                <p class="text-xs text-slate-500 dark:text-slate-400">
                  Prüft vor und nach der Ausführung, ob die Software bereits in der Zielversion vorhanden ist.
                </p>
              </div>
              <select
                :key="`det-${osFamily}`"
                v-model="detectionType"
                data-testid="detection-rule-type"
                class="rounded-lg border border-slate-300 bg-white px-3 py-1.5 text-xs font-semibold text-slate-800 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-200"
              >
                <option v-for="opt in availableDetectionTypes" :key="opt.value" :value="opt.value">
                  {{ opt.label }}
                </option>
              </select>
            </div>

            <!-- Guided macOS Presets & Auto-Brew Hint -->
            <div
              v-if="osFamily === 'macos'"
              class="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-xs dark:border-slate-800 dark:bg-slate-950/60"
            >
              <span class="text-slate-600 dark:text-slate-300">
                <template v-if="detectionType === 'none' && (sourceType === 'brew_cask' || sourceType === 'brew')">
                  Automatische Homebrew-Regel aktiv:
                  <code class="font-mono font-semibold text-sky-700 dark:text-sky-300">
                    {{ sourceType }}:{{ packageRef || 'paketname' }}
                  </code>
                </template>
                <template v-else>
                  macOS-Erkennungsvorlagen für <code>Info.plist</code>-Versionen oder <code>.pkg</code>-Receipts:
                </template>
              </span>
              <div class="flex flex-wrap items-center gap-1.5">
                <button
                  type="button"
                  data-testid="preset-macos-app-bundle"
                  class="inline-flex items-center gap-1 rounded-md border border-slate-300 bg-white px-2.5 py-1 text-[11px] font-semibold text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
                  @click="applyMacosDetectionPreset('app_bundle')"
                >
                  <Sparkles class="h-3 w-3 text-sky-600 dark:text-sky-400" aria-hidden="true" />
                  App-Bundle (/Applications/*.app)
                </button>
                <button
                  type="button"
                  data-testid="preset-macos-pkgutil"
                  class="inline-flex items-center gap-1 rounded-md border border-slate-300 bg-white px-2.5 py-1 text-[11px] font-semibold text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
                  @click="applyMacosDetectionPreset('pkgutil')"
                >
                  pkgutil Receipt
                </button>
              </div>
            </div>

            <div v-if="detectionType !== 'none'" class="grid grid-cols-1 gap-3 pt-2 sm:grid-cols-2">
              <div class="sm:col-span-2">
                <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">
                  {{
                    detectionType === 'app_bundle'
                      ? 'App-Bundle-Pfad unter /Applications *'
                      : detectionType === 'pkgutil'
                        ? 'macOS Installer-Receipt ID (pkgutil --pkg-info) *'
                        : detectionType === 'brew_cask'
                          ? 'Homebrew Cask Token *'
                          : detectionType === 'brew'
                            ? 'Homebrew Formula Name *'
                            : detectionType === 'file'
                              ? 'Vollständiger Dateipfad *'
                              : detectionType === 'registry'
                                ? 'Registry-Pfad (HKLM) *'
                                : 'Paketname *'
                  }}
                </label>
                <input
                  v-model="detectionPath"
                  data-testid="detection-rule-path"
                  type="text"
                  :placeholder="
                    detectionType === 'app_bundle'
                      ? '/Applications/Firefox.app'
                      : detectionType === 'pkgutil'
                        ? 'com.vendor.pkg oder org.mozilla.firefox.pkg'
                        : detectionType === 'brew_cask'
                          ? packageRef || 'firefox'
                          : detectionType === 'brew'
                            ? packageRef || 'jq'
                            : detectionType === 'file'
                              ? 'C:\\Program Files\\7-Zip\\7z.exe oder /usr/local/bin/tool'
                              : detectionType === 'registry'
                                ? 'HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\7-Zip'
                                : '7zip'
                  "
                  class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-xs dark:border-slate-700 dark:bg-slate-950"
                />
              </div>

              <div v-if="detectionType === 'app_bundle'">
                <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">
                  Bundle-ID (CFBundleIdentifier, optional)
                </label>
                <input
                  v-model="detectionBundleId"
                  data-testid="detection-rule-bundle-id"
                  type="text"
                  placeholder="z. B. org.mozilla.firefox"
                  class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-xs dark:border-slate-700 dark:bg-slate-950"
                />
              </div>

              <div v-if="detectionType === 'registry'">
                <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">Registry-Wertname</label>
                <input
                  v-model="detectionValueName"
                  type="text"
                  placeholder="DisplayVersion"
                  class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-xs dark:border-slate-700 dark:bg-slate-950"
                />
              </div>

              <div>
                <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">
                  {{ detectionType === 'app_bundle' ? 'Mindestversion (CFBundleShortVersionString)' : 'Mindestversion (optional)' }}
                </label>
                <input
                  v-model="detectionVersion"
                  data-testid="detection-rule-version"
                  type="text"
                  :placeholder="version || '24.08'"
                  class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-xs dark:border-slate-700 dark:bg-slate-950"
                />
              </div>
            </div>
          </div>
        </div>

        <!-- Footer -->
        <div class="flex flex-wrap items-center justify-between gap-3 border-t border-slate-200 pt-4 dark:border-slate-800">
          <div class="flex items-center gap-1.5 text-xs text-slate-500 dark:text-slate-400">
            <CheckCircle2 class="h-4 w-4 text-emerald-600 dark:text-emerald-400" aria-hidden="true" />
            Ed25519-Manifest-Signierung erfolgt automatisch durch den Server
          </div>
          <div class="flex items-center gap-2.5">
            <button
              type="button"
              class="rounded-lg border border-slate-300 bg-white px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
              @click="emit('close')"
            >
              Abbrechen
            </button>
            <button
              type="submit"
              data-testid="catalog-submit-btn"
              :disabled="saving || uploading"
              class="inline-flex items-center gap-2 rounded-lg bg-sky-600 px-4 py-2 text-sm font-semibold text-white shadow-sm hover:bg-sky-700 disabled:opacity-60"
            >
              {{ saving || uploading ? 'Wird gespeichert…' : editItem ? 'Änderungen speichern' : 'Paket speichern' }}
            </button>
          </div>
        </div>
      </form>
    </div>
  </div>
</template>
