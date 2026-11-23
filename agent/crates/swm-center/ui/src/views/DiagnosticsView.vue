<template>
  <div>
    <div class="view-header">
      <div>
        <h2 class="view-title">System &amp; Diagnose</h2>
        <p class="view-subtitle">
          Gerätestatus, aktive IT-Gruppenrichtlinien und Support-Diagnose für den Helpdesk.
        </p>
      </div>

      <div style="display: flex; gap: 10px; flex-wrap: wrap;">
        <button
          type="button"
          class="btn btn-outline"
          data-testid="diagnostics-copy-btn"
          @click="copyReport"
        >
          {{ copied ? 'Bericht kopiert!' : 'Diagnosebericht kopieren' }}
        </button>

        <button
          type="button"
          class="btn btn-primary"
          :disabled="syncing"
          data-testid="diagnostics-sync-btn"
          @click="$emit('sync')"
        >
          {{ syncing ? 'Synchronisiere…' : 'Jetzt mit Server synchronisieren' }}
        </button>
      </div>
    </div>

    <div
      v-if="feedbackMessage"
      class="policy-info-banner"
      role="status"
      aria-live="polite"
      style="margin-bottom: 18px;"
    >
      <span>{{ feedbackMessage }}</span>
    </div>

    <div class="diag-grid">
      <section class="diag-card" aria-label="Gerät und Verbindung">
        <h3 class="section-heading" style="margin-bottom: 14px;">Gerät &amp; Serververbindung</h3>
        <div class="diag-kv-list">
          <div class="diag-kv-row">
            <span class="diag-kv-label">Hostname</span>
            <strong class="diag-kv-val" data-testid="diag-hostname">{{ status.hostname || '–' }}</strong>
          </div>
          <div class="diag-kv-row">
            <span class="diag-kv-label">Geräte-ID</span>
            <code class="diag-code" data-testid="diag-device-id">{{ status.device_id || '–' }}</code>
          </div>
          <div class="diag-kv-row">
            <span class="diag-kv-label">Server-URL</span>
            <code class="diag-code" data-testid="diag-server-url">{{ status.server_url || '–' }}</code>
          </div>
          <div class="diag-kv-row">
            <span class="diag-kv-label">Verbindungsstatus</span>
            <span
              class="badge"
              :class="status.connected ? 'badge-installed' : 'badge-warning'"
              data-testid="diag-connection-status"
            >
              {{ status.connected ? 'Verbunden (Echtzeit-WSS)' : 'Getrennt / Lokaler Modus' }}
            </span>
          </div>
          <div class="diag-kv-row">
            <span class="diag-kv-label">Freigabestatus</span>
            <span class="badge badge-available">{{ status.device_status || 'approved' }}</span>
          </div>
          <div class="diag-kv-row">
            <span class="diag-kv-label">Agent-Version</span>
            <strong class="diag-kv-val" data-testid="diag-agent-version">v{{ status.agent_version || '1.0.0' }}</strong>
          </div>
        </div>
      </section>

      <section class="diag-card" aria-label="Aktive Gruppenrichtlinien">
        <h3 class="section-heading" style="margin-bottom: 14px;">Aktive Gruppenrichtlinien</h3>
        <div class="diag-kv-list">
          <div class="diag-kv-row">
            <div>
              <div class="diag-kv-label" style="color: var(--text-main); font-weight: 600;">Tray-Schutz</div>
              <div style="font-size: 12px; color: var(--text-muted);">Dauerhafter Hintergrundbetrieb in der Benutzersitzung</div>
            </div>
            <span
              class="badge"
              :class="enforceTray ? 'badge-installed' : 'badge-available'"
              data-testid="policy-enforce-tray"
            >
              {{ enforceTray ? 'Erzwungen (Geschützt)' : 'Optional' }}
            </span>
          </div>

          <div class="diag-kv-row">
            <div>
              <div class="diag-kv-label" style="color: var(--text-main); font-weight: 600;">Self-Service</div>
              <div style="font-size: 12px; color: var(--text-muted);">Eigenständige App-Installationen durch Benutzer</div>
            </div>
            <span
              class="badge"
              :class="allowSelfService ? 'badge-installed' : 'badge-warning'"
              data-testid="policy-self-service"
            >
              {{ allowSelfService ? 'Freigegeben' : 'Zentral verwaltet (Gesperrt)' }}
            </span>
          </div>

          <div class="diag-kv-row">
            <div>
              <div class="diag-kv-label" style="color: var(--text-main); font-weight: 600;">OS-Updates</div>
              <div style="font-size: 12px; color: var(--text-muted);">Manuelles Anstoßen von Betriebssystem-Updates</div>
            </div>
            <span
              class="badge"
              :class="allowOsUpdates ? 'badge-installed' : 'badge-warning'"
              data-testid="policy-os-updates"
            >
              {{ allowOsUpdates ? 'Freigegeben' : 'Zentral verwaltet (Gesperrt)' }}
            </span>
          </div>

          <div class="diag-kv-row">
            <div>
              <div class="diag-kv-label" style="color: var(--text-main); font-weight: 600;">Benachrichtigungsmodus</div>
              <div style="font-size: 12px; color: var(--text-muted);">Desktop-Hinweise bei Hintergrund-Rollouts</div>
            </div>
            <span class="badge badge-available" data-testid="policy-notifications">
              {{ silentNotifications ? 'Stiller Modus' : 'Standard' }}
            </span>
          </div>

          <div class="diag-kv-row">
            <div>
              <div class="diag-kv-label" style="color: var(--text-main); font-weight: 600;">Neustart-Richtlinie</div>
              <div style="font-size: 12px; color: var(--text-muted);">Steuerung erforderlicher Systemneustarts</div>
            </div>
            <span class="badge badge-available">
              {{ rebootPolicyLabel }}
            </span>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import type { CenterStatusSnapshot } from '../api';

const props = defineProps<{
  status: CenterStatusSnapshot;
  syncing?: boolean;
  syncMessage?: string | null;
}>();

defineEmits<{
  (e: 'sync'): void;
}>();

const copied = ref(false);
const localCopyNote = ref<string | null>(null);

const enforceTray = computed(() => props.status.desktop_policy?.enforce_tray !== false);
const allowSelfService = computed(() => props.status.desktop_policy?.allow_self_service !== false);
const allowOsUpdates = computed(() => props.status.desktop_policy?.allow_user_os_updates !== false);
const silentNotifications = computed(
  () => props.status.desktop_policy?.silent_notifications === true
);

const feedbackMessage = computed(() => localCopyNote.value || props.syncMessage || null);

const rebootPolicyLabel = computed(() => {
  switch (props.status.reboot_state.policy) {
    case 'grace_period':
      return `Karenzzeit (${props.status.reboot_state.grace_period_minutes} Min.)`;
    case 'maintenance_window':
      return 'Wartungsfenster';
    default:
      return 'Kein automatischer Neustart';
  }
});

function buildDiagnosticReportText(st: CenterStatusSnapshot): string {
  const pol = st.desktop_policy;
  return [
    '=== NS8 Software Center – Diagnosebericht ===',
    `Zeitstempel: ${new Date().toISOString()}`,
    `Hostname: ${st.hostname || '-'}`,
    `Geraete-ID: ${st.device_id || '-'}`,
    `Server-URL: ${st.server_url || '-'}`,
    `Verbindung: ${st.connected ? 'Verbunden (WSS)' : 'Lokaler Modus / Offline'}`,
    `Geraetestatus: ${st.device_status || '-'}`,
    `Agent-Version: v${st.agent_version || '1.0.0'}`,
    `Ausstehende OS-Updates: ${st.pending_os_updates.length}`,
    `Laufende Auftraege: ${st.active_jobs.length}`,
    `Neustart erforderlich: ${st.reboot_state.reboot_required ? 'Ja' : 'Nein'} (${st.reboot_state.policy})`,
    `Richtlinie Tray-Schutz: ${pol?.enforce_tray !== false ? 'Aktiv' : 'Aus'}`,
    `Richtlinie Self-Service: ${pol?.allow_self_service !== false ? 'Erlaubt' : 'Gesperrt'}`,
    `Richtlinie OS-Updates: ${pol?.allow_user_os_updates !== false ? 'Erlaubt' : 'Gesperrt'}`,
    `Stille Benachrichtigungen: ${pol?.silent_notifications ? 'Ja' : 'Nein'}`,
  ].join('\n');
}

async function copyReport() {
  const report = buildDiagnosticReportText(props.status);
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(report);
    } else {
      const ta = document.createElement('textarea');
      ta.value = report;
      ta.style.position = 'fixed';
      ta.style.opacity = '0';
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
    }
    copied.value = true;
    localCopyNote.value = 'Diagnosebericht wurde in die Zwischenablage kopiert.';
    setTimeout(() => {
      copied.value = false;
      localCopyNote.value = null;
    }, 3500);
  } catch {
    localCopyNote.value = 'Zwischenablage konnte in dieser Sitzung nicht beschrieben werden.';
  }
}
</script>
