<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import { Rocket, Send, Store, Trash2, X } from 'lucide-vue-next'
import { adminApi } from '@/api/client'
import type { CatalogItem, Deployment, DeploymentMode, Device, DeviceGroup } from '@/api/types'

const props = defineProps<{
  open: boolean
  catalog: CatalogItem[]
  groups: DeviceGroup[]
  devices: Device[]
  preselectedPackageId?: string | null
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'created', deployment: Deployment): void
}>()

const packageId = ref('')
const mode = ref<DeploymentMode>('required')
const targetGroupIds = ref<string[]>([])
const targetDeviceIds = ref<string[]>([])
const respectMaintenanceWindow = ref(true)
const saving = ref(false)
const error = ref<string | null>(null)

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.open && !saving.value) {
    emit('close')
  }
}

watch(
  () => [props.open, props.preselectedPackageId],
  ([isOpen]) => {
    if (typeof window !== 'undefined') {
      if (isOpen) {
        window.addEventListener('keydown', onKeyDown)
      } else {
        window.removeEventListener('keydown', onKeyDown)
      }
    }
    if (!isOpen) return
    error.value = null
    packageId.value = props.preselectedPackageId || props.catalog[0]?.id || ''
    mode.value = 'required'
    targetGroupIds.value = []
    targetDeviceIds.value = []
    respectMaintenanceWindow.value = true
  },
  { immediate: true },
)

onBeforeUnmount(() => {
  if (typeof window !== 'undefined') {
    window.removeEventListener('keydown', onKeyDown)
  }
})

const modeOptions: Array<{
  value: DeploymentMode
  title: string
  subtitle: string
  icon: typeof Rocket
}> = [
  {
    value: 'required',
    title: 'Pflicht (Required)',
    subtitle: 'Automatische Hintergrund-Installation & Updates auf Zielsystemen',
    icon: Rocket,
  },
  {
    value: 'available',
    title: 'Verfügbar (Software Center)',
    subtitle: 'Self-Service-Angebot ohne Admin-Rechte im lokalen Software Center',
    icon: Store,
  },
  {
    value: 'uninstall',
    title: 'Deinstallation (Uninstall)',
    subtitle: 'Entfernt das Paket kontrolliert von zugewiesenen Systemen',
    icon: Trash2,
  },
]

async function handleSubmit() {
  error.value = null
  if (!packageId.value) {
    error.value = 'Bitte wählen Sie ein Software-Paket aus dem Katalog aus.'
    return
  }
  if (targetGroupIds.value.length === 0 && targetDeviceIds.value.length === 0) {
    error.value = 'Bitte wählen Sie mindestens eine Gerätegruppe oder ein Einzelgerät als Ziel aus.'
    return
  }

  saving.value = true
  try {
    const created = await adminApi.createDeployment({
      package_id: packageId.value,
      target_group_ids: targetGroupIds.value,
      target_device_ids: targetDeviceIds.value,
      mode: mode.value,
      respect_maintenance_window: respectMaintenanceWindow.value,
    })
    emit('created', created)
    emit('close')
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Fehler beim Anlegen der Bereitstellung'
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
    aria-label="Neue Software-Bereitstellung erstellen"
    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/65 p-4 backdrop-blur-sm"
    @click.self="emit('close')"
  >
    <div
      class="flex max-h-[92vh] w-full max-w-2xl flex-col overflow-hidden rounded-xl border border-slate-200 bg-white shadow-2xl dark:border-slate-800 dark:bg-slate-900"
    >
      <div class="flex items-center justify-between border-b border-slate-800 bg-slate-900 px-6 py-4 text-white dark:bg-slate-950">
        <div class="flex items-center gap-3">
          <div class="rounded-lg bg-sky-500/20 p-2 text-sky-400">
            <Send class="h-5 w-5" aria-hidden="true" />
          </div>
          <div>
            <h3 class="text-base font-semibold">Neue Software-Bereitstellung erstellen</h3>
            <p class="text-xs text-slate-400">
              Rollout an Gerätegruppen oder Einzelgeräte zuweisen
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

      <form class="flex-1 space-y-5 overflow-y-auto p-6" @submit.prevent="handleSubmit">
        <div
          v-if="error"
          data-testid="deployment-error"
          class="rounded-lg border border-rose-200 bg-rose-50 px-4 py-3 text-sm font-medium text-rose-700 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-300"
        >
          {{ error }}
        </div>

        <!-- Package Select -->
        <div>
          <label class="block text-xs font-semibold text-slate-700 dark:text-slate-300">
            Katalog-Paket auswählen *
          </label>
          <select
            v-model="packageId"
            data-testid="deployment-package-select"
            class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-950"
          >
            <option value="" disabled>— Bitte Paket wählen —</option>
            <option v-for="pkg in catalog" :key="pkg.id" :value="pkg.id">
              {{ pkg.name }} ({{ pkg.version }}) — {{ pkg.os_family.toUpperCase() }} / {{ pkg.source_type }}
            </option>
          </select>
        </div>

        <!-- Deployment Mode Cards -->
        <div>
          <label class="mb-2 block text-xs font-semibold text-slate-700 dark:text-slate-300">
            Bereitstellungs-Modus *
          </label>
          <div class="grid grid-cols-1 gap-3 sm:grid-cols-3">
            <button
              v-for="opt in modeOptions"
              :key="opt.value"
              type="button"
              :data-testid="`deployment-mode-${opt.value}`"
              class="flex flex-col items-start rounded-xl border p-3.5 text-left transition-all"
              :class="
                mode === opt.value
                  ? 'border-sky-600 bg-sky-50/70 ring-2 ring-sky-500/20 dark:border-sky-500 dark:bg-sky-500/15 dark:ring-sky-400/20'
                  : 'border-slate-200 bg-white hover:border-slate-300 dark:border-slate-800 dark:bg-slate-950/60 dark:hover:border-slate-700'
              "
              @click="mode = opt.value"
            >
              <component
                :is="opt.icon"
                class="h-5 w-5"
                :class="mode === opt.value ? 'text-sky-600 dark:text-sky-400' : 'text-slate-500 dark:text-slate-400'"
                aria-hidden="true"
              />
              <span class="mt-2 text-xs font-bold text-slate-900 dark:text-slate-100">{{ opt.title }}</span>
              <span class="mt-1 text-[11px] leading-snug text-slate-600 dark:text-slate-400">{{ opt.subtitle }}</span>
            </button>
          </div>
        </div>

        <!-- Target Groups & Devices -->
        <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div>
            <label class="mb-1.5 block text-xs font-semibold text-slate-700 dark:text-slate-300">
              Ziel-Gerätegruppen ({{ targetGroupIds.length }} gewählt)
            </label>
            <div class="max-h-40 space-y-1.5 overflow-y-auto rounded-lg border border-slate-200 bg-slate-50 p-2.5 dark:border-slate-800 dark:bg-slate-950/60">
              <div v-if="groups.length === 0" class="py-3 text-center text-xs text-slate-400">
                Keine Gruppen angelegt
              </div>
              <label
                v-for="grp in groups"
                :key="grp.id"
                class="flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-xs hover:bg-white dark:hover:bg-slate-800"
              >
                <input
                  v-model="targetGroupIds"
                  :value="grp.id"
                  :data-testid="`deployment-group-${grp.id}`"
                  type="checkbox"
                  class="h-3.5 w-3.5 rounded border-slate-300 text-sky-600"
                />
                <span class="font-medium text-slate-800 dark:text-slate-200">{{ grp.name }}</span>
              </label>
            </div>
          </div>

          <div>
            <label class="mb-1.5 block text-xs font-semibold text-slate-700 dark:text-slate-300">
              Einzelgeräte ({{ targetDeviceIds.length }} gewählt)
            </label>
            <div class="max-h-40 space-y-1.5 overflow-y-auto rounded-lg border border-slate-200 bg-slate-50 p-2.5 dark:border-slate-800 dark:bg-slate-950/60">
              <div v-if="devices.length === 0" class="py-3 text-center text-xs text-slate-400">
                Keine Geräte vorhanden
              </div>
              <label
                v-for="dev in devices"
                :key="dev.id"
                class="flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-xs hover:bg-white dark:hover:bg-slate-800"
              >
                <input
                  v-model="targetDeviceIds"
                  :value="dev.id"
                  :data-testid="`deployment-device-${dev.id}`"
                  type="checkbox"
                  class="h-3.5 w-3.5 rounded border-slate-300 text-sky-600"
                />
                <span class="truncate font-medium text-slate-800 dark:text-slate-200">{{ dev.hostname }}</span>
                <span class="text-[10px] uppercase text-slate-400">{{ dev.os_family }}</span>
              </label>
            </div>
          </div>
        </div>

        <!-- Maintenance Window Toggle -->
        <label
          v-if="mode !== 'available'"
          class="flex items-center gap-3 rounded-lg border border-slate-200 bg-slate-50 p-3.5 dark:border-slate-800 dark:bg-slate-950/60"
        >
          <input
            v-model="respectMaintenanceWindow"
            data-testid="deployment-respect-mw"
            type="checkbox"
            class="h-4 w-4 rounded border-slate-300 text-sky-600"
          />
          <div>
            <div class="text-sm font-medium text-slate-900 dark:text-slate-100">
              Wartungsfenster der Zielgruppe strikt einhalten
            </div>
            <div class="text-xs text-slate-500 dark:text-slate-400">
              Automatische Installationen starten nur innerhalb definierter Wartungsfenster (bzw. per Catch-up-Regel).
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
            data-testid="deployment-submit-btn"
            :disabled="saving"
            class="rounded-lg bg-sky-600 px-4 py-2 text-sm font-semibold text-white shadow-sm hover:bg-sky-700 disabled:opacity-60"
          >
            {{ saving ? 'Wird ausgerollt…' : 'Bereitstellung aktivieren' }}
          </button>
        </div>
      </form>
    </div>
  </div>
</template>
