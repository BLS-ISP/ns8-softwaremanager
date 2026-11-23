<template>
  <div>
    <div class="view-header">
      <div>
        <h2 class="view-title">Anwendungen</h2>
        <p class="view-subtitle">
          Freigegebene Unternehmenssoftware ohne lokale Administratorrechte installieren oder aktualisieren.
        </p>
      </div>
      <div class="badge badge-available" aria-live="polite">
        {{ installedCount }} von {{ items.length }} installiert
      </div>
    </div>

    <div
      v-if="!allowSelfService"
      class="policy-info-banner"
      role="status"
      data-testid="self-service-policy-banner"
    >
      <span>
        <strong>Durch IT-Richtlinie verwalteter Modus — Installationen werden zentral gesteuert.</strong>
        Manuelle Installationen und Deinstallationen sind für diese Gerätegruppe deaktiviert.
      </span>
    </div>

    <div class="toolbar">
      <div class="search-box">
        <svg
          class="search-icon"
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          aria-hidden="true"
        >
          <circle cx="11" cy="11" r="7" />
          <line x1="21" y1="21" x2="16.65" y2="16.65" stroke-linecap="round" />
        </svg>
        <input
          v-model="searchQuery"
          type="search"
          class="search-input"
          placeholder="Programmname, Hersteller oder Beschreibung suchen…"
          aria-label="Anwendungen durchsuchen"
          data-testid="catalog-search"
        />
        <button
          v-if="searchQuery"
          type="button"
          class="search-clear-btn"
          aria-label="Suche zurücksetzen"
          @click="searchQuery = ''"
        >
          ×
        </button>
      </div>

      <div class="category-chips" role="group" aria-label="Kategorie-Filter">
        <button
          v-for="cat in categories"
          :key="cat.name"
          type="button"
          class="pill-btn"
          :class="{ active: selectedCategory === cat.name }"
          :aria-pressed="selectedCategory === cat.name"
          :data-testid="`category-chip-${cat.name}`"
          @click="selectedCategory = cat.name"
        >
          <span>{{ cat.name }}</span>
          <span class="chip-count">({{ cat.count }})</span>
        </button>
      </div>
    </div>

    <div v-if="filteredItems.length === 0" class="empty-panel" role="status">
      <p style="margin: 0 0 10px; font-weight: 600; color: var(--text-main);">
        Keine freigegebenen Anwendungen für den gewählten Filter gefunden.
      </p>
      <button
        v-if="searchQuery || selectedCategory !== 'Alle'"
        type="button"
        class="btn btn-outline"
        @click="resetFilters"
      >
        Filter zurücksetzen
      </button>
    </div>

    <div v-else class="cards-grid">
      <article
        v-for="item in filteredItems"
        :key="item.package_id"
        class="app-card"
        :aria-label="item.name"
        :data-testid="`app-card-${item.package_id}`"
      >
        <div>
          <div class="card-head">
            <img
              v-if="item.icon_url && !brokenIcons[item.package_id]"
              :src="item.icon_url"
              :alt="`${item.name} Symbol`"
              class="app-icon-img"
              @error="markIconBroken(item.package_id)"
            />
            <div
              v-else
              class="app-icon"
              :class="avatarVariant(item.category, item.name)"
              aria-hidden="true"
              :data-testid="`app-avatar-${item.package_id}`"
            >
              {{ monogram(item.name) }}
            </div>

            <div style="flex: 1; min-width: 0;">
              <div class="card-title-row">
                <h3 class="card-title" :title="item.name">{{ item.name }}</h3>
                <span
                  class="badge"
                  :class="statusBadgeClass(effectiveStatus(item))"
                  :data-testid="`status-pill-${item.package_id}`"
                >
                  {{ statusLabel(effectiveStatus(item)) }}
                </span>
              </div>
              <div class="card-vendor">
                {{ item.vendor || 'Unternehmenspaket' }} · v{{ item.version }}
                <span v-if="item.installed_version"> (Installiert: v{{ item.installed_version }})</span>
              </div>
            </div>
          </div>

          <p class="card-desc">
            {{ item.description || 'Bereitgestellt über NS8-SoftwareManager.' }}
          </p>

          <div
            v-if="item.status === 'waiting_for_process' && item.blocked_processes.length > 0"
            class="card-callout"
            role="status"
          >
            Bitte schließen Sie vor dem Fortfahren: <strong>{{ item.blocked_processes.join(', ') }}</strong>
          </div>

          <div
            v-else-if="item.last_message"
            class="card-callout"
            role="status"
          >
            {{ item.last_message }}
          </div>

          <div
            v-if="isProgressActive(item)"
            class="progress-track"
            role="progressbar"
            aria-label="Vorgang wird ausgeführt"
            aria-busy="true"
            :data-testid="`progress-bar-${item.package_id}`"
          >
            <div class="progress-bar-indeterminate"></div>
          </div>
        </div>

        <div class="card-footer">
          <span class="card-meta-tag">
            {{ item.category || 'Allgemein' }} · {{ sourceBadge(item.source_type) }}
          </span>

          <div style="display: flex; gap: 8px; flex-wrap: wrap;">
            <button
              v-if="canInstall(item)"
              type="button"
              class="btn btn-primary"
              :disabled="!allowSelfService || busyId === item.package_id"
              :data-testid="`install-btn-${item.package_id}`"
              @click="$emit('trigger', item.package_id, 'install')"
            >
              {{ item.status === 'failed' ? 'Erneut versuchen' : 'Installieren' }}
            </button>

            <button
              v-if="item.status === 'update_available'"
              type="button"
              class="btn btn-primary"
              :disabled="!allowSelfService || busyId === item.package_id"
              :data-testid="`update-btn-${item.package_id}`"
              @click="$emit('trigger', item.package_id, 'update')"
            >
              Aktualisieren
            </button>

            <button
              v-if="canUninstall(item)"
              type="button"
              class="btn btn-danger"
              :disabled="!allowSelfService || busyId === item.package_id"
              :data-testid="`uninstall-btn-${item.package_id}`"
              @click="requestUninstall(item)"
            >
              Deinstallieren
            </button>
          </div>
        </div>
      </article>
    </div>

    <div
      v-if="pendingUninstallItem"
      class="modal-backdrop"
      role="dialog"
      aria-modal="true"
      aria-labelledby="uninstall-confirm-title"
      data-testid="confirm-uninstall-modal"
      @click.self="pendingUninstallItem = null"
    >
      <div class="modal-panel">
        <h3 id="uninstall-confirm-title" style="margin: 0 0 10px; font-size: 18px;">
          Anwendung deinstallieren?
        </h3>
        <p style="margin: 0 0 20px; font-size: 14px; color: var(--text-secondary); line-height: 1.5;">
          Möchten Sie <strong>{{ pendingUninstallItem.name }}</strong> (v{{ pendingUninstallItem.installed_version || pendingUninstallItem.version }}) wirklich von diesem Gerät entfernen?
        </p>
        <div style="display: flex; justify-content: flex-end; gap: 10px;">
          <button
            type="button"
            class="btn btn-outline"
            data-testid="cancel-uninstall-btn"
            @click="pendingUninstallItem = null"
          >
            Abbrechen
          </button>
          <button
            type="button"
            class="btn btn-danger-solid"
            data-testid="confirm-uninstall-btn"
            @click="confirmUninstall"
          >
            Jetzt deinstallieren
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue';
import type { CatalogViewItem } from '../api';

const props = withDefaults(
  defineProps<{
    items: CatalogViewItem[];
    busyId?: string | null;
    allowSelfService?: boolean;
  }>(),
  {
    allowSelfService: true,
  }
);

const emit = defineEmits<{
  (e: 'trigger', packageId: string, action: 'install' | 'update' | 'uninstall'): void;
}>();

const searchQuery = ref('');
const selectedCategory = ref('Alle');
const pendingUninstallItem = ref<CatalogViewItem | null>(null);
const brokenIcons = reactive<Record<string, boolean>>({});

function markIconBroken(pkgId: string) {
  brokenIcons[pkgId] = true;
}

const installedCount = computed(
  () =>
    props.items.filter(
      (i) =>
        i.status === 'installed' ||
        i.status === 'update_available' ||
        i.status === 'reboot_required'
    ).length
);

const categories = computed(() => {
  const counts = new Map<string, number>();
  for (const item of props.items) {
    const c = item.category?.trim();
    if (c) {
      counts.set(c, (counts.get(c) ?? 0) + 1);
    }
  }
  const result = [{ name: 'Alle', count: props.items.length }];
  for (const [name, count] of counts.entries()) {
    result.push({ name, count });
  }
  return result;
});

const filteredItems = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  return props.items.filter((item) => {
    if (selectedCategory.value !== 'Alle' && item.category !== selectedCategory.value) {
      return false;
    }
    if (!q) return true;
    return (
      item.name.toLowerCase().includes(q) ||
      item.vendor.toLowerCase().includes(q) ||
      item.description.toLowerCase().includes(q) ||
      item.category.toLowerCase().includes(q)
    );
  });
});

function resetFilters() {
  searchQuery.value = '';
  selectedCategory.value = 'Alle';
}

function monogram(name: string): string {
  const words = name
    .trim()
    .split(/[\s-]+/)
    .filter(Boolean);
  if (words.length >= 2) {
    return (words[0][0] + words[1][0]).toUpperCase();
  }
  return (name.trim().slice(0, 2) || 'SW').toUpperCase();
}

function avatarVariant(category: string, name: string): string {
  const key = (category || '').toLowerCase();
  if (key.includes('browser') || key.includes('web')) return 'avatar-browser';
  if (key.includes('sicher') || key.includes('security') || key.includes('schutz')) return 'avatar-security';
  if (key.includes('entwickl') || key.includes('dev') || key.includes('code')) return 'avatar-dev';
  if (key.includes('werkzeug') || key.includes('tool') || key.includes('system')) return 'avatar-tools';
  if (key.includes('kommunik') || key.includes('office') || key.includes('mail')) return 'avatar-comm';
  if (key.includes('medien') || key.includes('design') || key.includes('grafik')) return 'avatar-media';

  const variants = [
    'avatar-default',
    'avatar-browser',
    'avatar-dev',
    'avatar-comm',
    'avatar-security',
    'avatar-tools',
  ];
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = (hash * 31 + name.charCodeAt(i)) >>> 0;
  }
  return variants[hash % variants.length];
}

function sourceBadge(src: string): string {
  switch (src.toLowerCase()) {
    case 'winget':
      return 'WinGet';
    case 'apt':
      return 'APT';
    case 'dnf':
      return 'DNF';
    case 'flatpak':
      return 'Flatpak';
    case 'brew':
      return 'Homebrew';
    case 'brew_cask':
      return 'Homebrew Cask';
    default:
      return 'NS8 Paket';
  }
}

function effectiveStatus(item: CatalogViewItem): string {
  if (props.busyId === item.package_id && item.status !== 'downloading') {
    return 'installing';
  }
  return item.status;
}

function isProgressActive(item: CatalogViewItem): boolean {
  return (
    props.busyId === item.package_id ||
    item.status === 'downloading' ||
    item.status === 'installing'
  );
}

function statusLabel(status: string): string {
  switch (status) {
    case 'downloading':
      return 'Wird heruntergeladen…';
    case 'installing':
      return 'Wird installiert…';
    case 'waiting_for_process':
      return 'Wartet auf Schließen der App';
    case 'installed':
      return 'Installiert';
    case 'update_available':
      return 'Update verfügbar';
    case 'reboot_required':
      return 'Neustart ausstehend';
    case 'failed':
      return 'Fehlgeschlagen';
    default:
      return 'Verfügbar';
  }
}

function statusBadgeClass(status: string): string {
  switch (status) {
    case 'installed':
      return 'badge-installed';
    case 'update_available':
      return 'badge-update';
    case 'downloading':
    case 'installing':
      return 'badge-progress';
    case 'waiting_for_process':
    case 'reboot_required':
      return 'badge-warning';
    case 'failed':
      return 'badge-danger';
    default:
      return 'badge-available';
  }
}

function canInstall(item: CatalogViewItem): boolean {
  return (
    item.status === 'available' ||
    item.status === 'failed' ||
    item.status === 'waiting_for_process'
  );
}

function canUninstall(item: CatalogViewItem): boolean {
  if (!item.allow_uninstall) return false;
  return (
    item.status === 'installed' ||
    item.status === 'update_available' ||
    item.status === 'reboot_required'
  );
}

function requestUninstall(item: CatalogViewItem) {
  if (!props.allowSelfService) return;
  pendingUninstallItem.value = item;
}

function confirmUninstall() {
  if (!pendingUninstallItem.value) return;
  const id = pendingUninstallItem.value.package_id;
  pendingUninstallItem.value = null;
  emit('trigger', id, 'uninstall');
}

function onKeyDown(ev: KeyboardEvent) {
  if (ev.key === 'Escape' && pendingUninstallItem.value) {
    pendingUninstallItem.value = null;
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeyDown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', onKeyDown);
});
</script>
