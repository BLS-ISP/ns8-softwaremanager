<template>
  <div class="app-shell" :data-theme="theme">
    <aside class="sidebar" aria-label="Seitenleiste">
      <div class="sidebar-top">
        <div class="brand-header" data-testid="brand-header">
          <div class="brand-logo-wrap" aria-hidden="true">
            <svg width="28" height="28" viewBox="0 0 64 64" fill="none" data-testid="brand-logo-svg">
              <defs>
                <linearGradient id="swm-logo-grad" x1="8" y1="6" x2="56" y2="58" gradientUnits="userSpaceOnUse">
                  <stop offset="0%" stop-color="#06b6d4" />
                  <stop offset="52%" stop-color="#2563eb" />
                  <stop offset="100%" stop-color="#4f46e5" />
                </linearGradient>
              </defs>
              <path
                d="M32 4L10 13.2V29.6C10 43.7 19.4 55.8 32 60C44.6 55.8 54 43.7 54 29.6V13.2L32 4Z"
                fill="url(#swm-logo-grad)"
              />
              <polygon points="32,17 45,24.5 32,32 19,24.5" fill="#ffffff" fill-opacity="0.96" />
              <polygon points="19,24.5 32,32 32,46.5 19,39" fill="#bae6fd" fill-opacity="0.86" />
              <polygon points="45,24.5 32,32 32,46.5 45,39" fill="#93c5fd" fill-opacity="0.72" />
            </svg>
          </div>
          <div>
            <h1>Software Center</h1>
            <p>NS8 Endpoint Management</p>
          </div>
        </div>

        <nav aria-label="Hauptnavigation" style="display: flex; flex-direction: column; gap: 6px;">
          <button
            type="button"
            class="nav-btn"
            :class="{ active: activeTab === 'store' }"
            :aria-current="activeTab === 'store' ? 'page' : undefined"
            data-testid="nav-store"
            @click="activeTab = 'store'"
          >
            <span class="nav-btn-label">
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                <rect x="3" y="3" width="7" height="7" rx="1.5" />
                <rect x="14" y="3" width="7" height="7" rx="1.5" />
                <rect x="3" y="14" width="7" height="7" rx="1.5" />
                <rect x="14" y="14" width="7" height="7" rx="1.5" />
              </svg>
              <span>Anwendungen</span>
            </span>
            <span class="nav-badge">{{ catalog.length }}</span>
          </button>

          <button
            type="button"
            class="nav-btn"
            :class="{ active: activeTab === 'updates' }"
            :aria-current="activeTab === 'updates' ? 'page' : undefined"
            data-testid="nav-updates"
            @click="activeTab = 'updates'"
          >
            <span class="nav-btn-label">
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                <path d="M12 3v12m0 0l-4-4m4 4l4-4" stroke-linecap="round" stroke-linejoin="round" />
                <path d="M4 17v2a2 2 0 002 2h12a2 2 0 002-2v-2" stroke-linecap="round" />
              </svg>
              <span>Updates</span>
            </span>
            <span v-if="totalUpdatesCount > 0" class="nav-badge">{{ totalUpdatesCount }}</span>
          </button>

          <button
            type="button"
            class="nav-btn"
            :class="{ active: activeTab === 'history' }"
            :aria-current="activeTab === 'history' ? 'page' : undefined"
            data-testid="nav-history"
            @click="activeTab = 'history'"
          >
            <span class="nav-btn-label">
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                <circle cx="12" cy="12" r="9" />
                <path d="M12 7v5l3 2" stroke-linecap="round" />
              </svg>
              <span>Aktivitäten / Verlauf</span>
            </span>
            <span v-if="status.active_jobs.length > 0" class="nav-badge">
              {{ status.active_jobs.length }}
            </span>
          </button>

          <button
            type="button"
            class="nav-btn"
            :class="{ active: activeTab === 'diagnostics' }"
            :aria-current="activeTab === 'diagnostics' ? 'page' : undefined"
            data-testid="nav-diagnostics"
            @click="activeTab = 'diagnostics'"
          >
            <span class="nav-btn-label">
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" stroke-linejoin="round" />
                <path d="M9 12l2 2 4-4" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
              <span>System &amp; Diagnose</span>
            </span>
          </button>
        </nav>
      </div>

      <div class="sidebar-footer">
        <div>
          <span class="status-dot" :class="status.connected ? 'online' : 'offline'"></span>
          <span>{{ status.connected ? 'Mit NS8-Server verbunden' : 'Lokaler Agent-Modus' }}</span>
        </div>
        <div>Agent v{{ status.agent_version || '1.0.0' }}</div>
      </div>
    </aside>

    <div class="main-area">
      <header class="topbar">
        <div class="topbar-meta">
          <span class="device-pill">
            Geräte-ID: <strong>{{ status.device_id || '–' }}</strong>
          </span>
          <span v-if="status.active_jobs.length > 0" class="badge badge-progress">
            {{ status.active_jobs.length }} Vorgang aktiv…
          </span>
        </div>

        <div class="topbar-actions">
          <span class="badge" :class="status.connected ? 'badge-installed' : 'badge-available'">
            {{ status.connected ? 'Verbunden' : 'Lokaler Modus' }}
          </span>

          <button
            type="button"
            class="theme-toggle"
            :aria-pressed="theme === 'dark'"
            :aria-label="theme === 'dark' ? 'Zum hellen Design wechseln' : 'Zum dunklen Design wechseln'"
            data-testid="theme-toggle-btn"
            @click="toggleTheme"
          >
            <svg
              v-if="theme === 'dark'"
              width="15"
              height="15"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              aria-hidden="true"
            >
              <circle cx="12" cy="12" r="4" />
              <path d="M12 2v2m0 16v2M4.93 4.93l1.41 1.41m11.32 11.32l1.41 1.41M2 12h2m16 0h2M4.93 19.07l1.41-1.41m11.32-11.32l1.41-1.41" stroke-linecap="round" />
            </svg>
            <svg
              v-else
              width="15"
              height="15"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              aria-hidden="true"
            >
              <path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
            <span>{{ theme === 'dark' ? 'Hell' : 'Dunkel' }}</span>
          </button>

          <button
            type="button"
            class="btn btn-outline"
            :disabled="loading"
            aria-label="Status und Katalog aktualisieren"
            @click="refreshAll"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.2"
              :class="{ 'spin-icon': loading }"
              aria-hidden="true"
            >
              <path d="M21 12a9 9 0 11-2.64-6.36" stroke-linecap="round" />
              <polyline points="21 3 21 9 15 9" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
            <span>Aktualisieren</span>
          </button>
        </div>
      </header>

      <div
        v-for="msg in status.admin_messages ?? []"
        :key="msg.id"
        class="admin-msg-banner"
        :class="{ urgent: msg.urgent }"
        role="status"
        aria-live="polite"
        data-testid="admin-message-banner"
      >
        <div class="admin-msg-content">
          <div class="admin-msg-title-row">
            <span class="badge" :class="msg.urgent ? 'badge-danger' : 'badge-update'">
              {{ msg.urgent ? 'Wichtige IT-Mitteilung' : 'IT-Mitteilung' }}
            </span>
            <strong>{{ msg.title }}</strong>
          </div>
          <p class="admin-msg-body">{{ msg.message }}</p>
        </div>
        <button
          type="button"
          class="btn btn-outline"
          :data-testid="`dismiss-message-${msg.id}`"
          :aria-label="`Mitteilung ${msg.title} schließen`"
          @click="onDismissMessage(msg.id)"
        >
          Quittieren
        </button>
      </div>

      <div v-if="status.reboot_state.reboot_required" class="reboot-banner" role="status" aria-live="polite">
        <span>
          <strong>Neustart ausstehend:</strong> Ein installiertes Update erfordert einen Systemneustart.
        </span>
        <button type="button" class="btn btn-danger-solid" @click="rebootModalOpen = true">
          Neustart-Optionen öffnen
        </button>
      </div>

      <div v-if="errorBanner" class="error-banner" role="alert">
        <span>{{ errorBanner }}</span>
        <button
          type="button"
          class="btn btn-outline"
          aria-label="Fehlermeldung schließen"
          @click="errorBanner = null"
        >
          Schließen
        </button>
      </div>

      <main class="content-container">
        <StoreView
          v-if="activeTab === 'store'"
          :items="catalog"
          :busy-id="busyPackageId"
          :allow-self-service="status.desktop_policy?.allow_self_service !== false"
          @trigger="onTriggerPackage"
        />

        <UpdatesView
          v-else-if="activeTab === 'updates'"
          :os-updates="status.pending_os_updates"
          :app-updates="appUpdates"
          :busy="busyUpdates"
          :allow-user-os-updates="status.desktop_policy?.allow_user_os_updates !== false"
          :allow-self-service="status.desktop_policy?.allow_self_service !== false"
          @trigger-all="onTriggerAllUpdates"
          @trigger-app="(id) => onTriggerPackage(id, 'update')"
        />

        <DiagnosticsView
          v-else-if="activeTab === 'diagnostics'"
          :status="status"
          :syncing="busySync"
          :sync-message="syncBanner"
          @sync="onSyncNow"
        />

        <HistoryView
          v-else
          :active-jobs="status.active_jobs"
          :history="status.recent_history"
        />
      </main>
    </div>

    <RebootModal
      :visible="rebootModalOpen && status.reboot_state.reboot_required"
      :reboot-state="status.reboot_state"
      :busy="busyReboot"
      @snooze="onSnoozeReboot"
      @confirm="onConfirmReboot"
      @close="rebootModalOpen = false"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import {
  confirmRebootNow,
  dismissAdminMessage,
  fetchCatalog,
  fetchCenterStatus,
  snoozeReboot,
  syncWithServer,
  triggerCatalogAction,
  triggerPendingUpdates,
  type CatalogViewItem,
  type CenterStatusSnapshot,
} from './api';
import RebootModal from './components/RebootModal.vue';
import StoreView from './views/StoreView.vue';
import UpdatesView from './views/UpdatesView.vue';
import HistoryView from './views/HistoryView.vue';
import DiagnosticsView from './views/DiagnosticsView.vue';

const THEME_STORAGE_KEY = 'swm-center-theme';

function detectInitialTheme(): 'light' | 'dark' {
  try {
    const saved = window.localStorage?.getItem(THEME_STORAGE_KEY);
    if (saved === 'dark' || saved === 'light') {
      return saved;
    }
    if (window.matchMedia?.('(prefers-color-scheme: dark)').matches) {
      return 'dark';
    }
  } catch {
    // Ignore storage errors in restricted WebViews
  }
  return 'light';
}

const theme = ref<'light' | 'dark'>(detectInitialTheme());

function applyThemeToDocument(next: 'light' | 'dark') {
  if (typeof document !== 'undefined') {
    document.documentElement.setAttribute('data-theme', next);
  }
}

applyThemeToDocument(theme.value);

watch(theme, (next) => {
  applyThemeToDocument(next);
  try {
    window.localStorage?.setItem(THEME_STORAGE_KEY, next);
  } catch {
    // Ignore storage write failures
  }
});

function toggleTheme() {
  theme.value = theme.value === 'dark' ? 'light' : 'dark';
}

const activeTab = ref<'store' | 'updates' | 'history' | 'diagnostics'>('store');
const catalog = ref<CatalogViewItem[]>([]);
const status = ref<CenterStatusSnapshot>({
  device_id: '',
  hostname: '',
  server_url: '',
  agent_version: '1.0.0',
  connected: false,
  device_status: 'approved',
  pending_os_updates: [],
  active_jobs: [],
  recent_history: [],
  reboot_state: {
    reboot_required: false,
    policy: 'never',
    grace_period_minutes: 240,
    detected_at_unix: null,
    deadline_unix: null,
    snoozed_until_unix: null,
    triggered_by_job_id: null,
  },
  desktop_policy: {
    enforce_tray: true,
    allow_self_service: true,
    allow_user_os_updates: true,
    silent_notifications: false,
  },
  admin_messages: [],
});

const loading = ref(false);
const busyPackageId = ref<string | null>(null);
const busyUpdates = ref(false);
const busyReboot = ref(false);
const busySync = ref(false);
const syncBanner = ref<string | null>(null);
const rebootModalOpen = ref(false);
const errorBanner = ref<string | null>(null);

let pollTimer: ReturnType<typeof setInterval> | null = null;

const appUpdates = computed(() =>
  catalog.value.filter((item) => item.status === 'update_available')
);

const totalUpdatesCount = computed(
  () => status.value.pending_os_updates.length + appUpdates.value.length
);

async function refreshAll() {
  loading.value = true;
  try {
    const [catData, stData] = await Promise.all([fetchCatalog(), fetchCenterStatus()]);
    catalog.value = catData;
    status.value = stData;
    errorBanner.value = null;

    const now = Math.floor(Date.now() / 1000);
    const rb = stData.reboot_state;
    const snoozed = rb.snoozed_until_unix && now < rb.snoozed_until_unix;
    if (rb.reboot_required && !snoozed) {
      rebootModalOpen.value = true;
    }
  } catch (err: unknown) {
    errorBanner.value = err instanceof Error ? err.message : String(err);
  } finally {
    loading.value = false;
  }
}

async function onTriggerPackage(
  packageId: string,
  action: 'install' | 'update' | 'uninstall'
) {
  busyPackageId.value = packageId;
  errorBanner.value = null;
  try {
    await triggerCatalogAction(packageId, action);
    await refreshAll();
  } catch (err: unknown) {
    errorBanner.value = err instanceof Error ? err.message : String(err);
  } finally {
    busyPackageId.value = null;
  }
}

async function onTriggerAllUpdates() {
  busyUpdates.value = true;
  errorBanner.value = null;
  try {
    await triggerPendingUpdates();
    await refreshAll();
  } catch (err: unknown) {
    errorBanner.value = err instanceof Error ? err.message : String(err);
  } finally {
    busyUpdates.value = false;
  }
}

async function onSnoozeReboot(minutes: number) {
  busyReboot.value = true;
  try {
    status.value.reboot_state = await snoozeReboot(minutes);
    rebootModalOpen.value = false;
  } catch (err: unknown) {
    errorBanner.value = err instanceof Error ? err.message : String(err);
  } finally {
    busyReboot.value = false;
  }
}

async function onConfirmReboot() {
  busyReboot.value = true;
  try {
    status.value.reboot_state = await confirmRebootNow();
    rebootModalOpen.value = false;
  } catch (err: unknown) {
    errorBanner.value = err instanceof Error ? err.message : String(err);
  } finally {
    busyReboot.value = false;
  }
}

async function onSyncNow() {
  busySync.value = true;
  errorBanner.value = null;
  try {
    const res = await syncWithServer();
    syncBanner.value = res?.message || 'Synchronisation mit NS8-Server gestartet.';
    await refreshAll();
  } catch (err: unknown) {
    errorBanner.value = err instanceof Error ? err.message : String(err);
  } finally {
    busySync.value = false;
  }
}

async function onDismissMessage(messageId: string) {
  try {
    const updated = await dismissAdminMessage(messageId);
    if (updated && Array.isArray(updated.admin_messages)) {
      status.value = updated;
    } else if (status.value.admin_messages) {
      status.value.admin_messages = status.value.admin_messages.filter(
        (m) => m.id !== messageId
      );
    }
  } catch (err: unknown) {
    errorBanner.value = err instanceof Error ? err.message : String(err);
  }
}

onMounted(() => {
  void refreshAll();
  pollTimer = setInterval(() => {
    void refreshAll();
  }, 10000);
});

onUnmounted(() => {
  if (pollTimer) clearInterval(pollTimer);
});
</script>
