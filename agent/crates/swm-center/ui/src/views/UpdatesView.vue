<template>
  <div>
    <div class="view-header">
      <div>
        <h2 class="view-title">Updates</h2>
        <p class="view-subtitle">
          Ausstehende Betriebssystem- und Anwendungs-Updates proaktiv vor dem nächsten Wartungsfenster installieren.
        </p>
      </div>

      <button
        type="button"
        class="btn btn-primary"
        :disabled="!allowUserOsUpdates || busy || (osUpdates.length === 0 && appUpdates.length === 0)"
        data-testid="install-all-updates-btn"
        @click="$emit('trigger-all')"
      >
        {{ busy ? 'Wird installiert…' : 'Jetzt installieren' }}
      </button>
    </div>

    <div
      v-if="!allowUserOsUpdates"
      class="policy-info-banner"
      role="status"
      data-testid="os-updates-policy-banner"
    >
      <span>
        <strong>Durch IT-Richtlinie verwalteter Modus — Installationen werden zentral gesteuert.</strong>
        Manuelle Betriebssystem-Updates sind deaktiviert und werden automatisch im geplanten Wartungsfenster ausgerollt.
      </span>
    </div>

    <div v-if="busy" class="active-job-card" role="status" aria-live="polite" style="margin-bottom: 18px;">
      <div style="font-size: 13.5px; font-weight: 600;">
        Freigegebene Updates werden im Hintergrund installiert…
      </div>
      <div class="progress-track" role="progressbar" aria-label="Update-Fortschritt">
        <div class="progress-bar-indeterminate"></div>
      </div>
    </div>

    <div
      v-if="osUpdates.length === 0 && appUpdates.length === 0"
      class="empty-panel up-to-date"
      role="status"
    >
      <div style="font-size: 15px; font-weight: 600;">
        Ihr System ist auf dem aktuellen Stand – keine ausstehenden Updates.
      </div>
    </div>

    <section v-if="osUpdates.length > 0" style="margin-bottom: 26px;" aria-label="Betriebssystem-Updates">
      <h3 class="section-heading">Betriebssystem-Updates ({{ osUpdates.length }})</h3>
      <div class="list-panel">
        <div
          v-for="u in osUpdates"
          :key="u.update_id"
          class="list-row"
        >
          <div>
            <div style="font-weight: 600; font-size: 14px; color: var(--text-main);">
              {{ u.title }}
            </div>
            <div style="font-size: 12px; color: var(--text-muted); margin-top: 3px;">
              ID: <strong>{{ u.update_id }}</strong> · Kategorie: {{ u.category }}
              <span v-if="u.size_bytes > 0"> · {{ formatSize(u.size_bytes) }}</span>
              <span v-if="u.reboot_behavior === 'always_requires_reboot'"> · Neustart erforderlich</span>
            </div>
          </div>
          <span
            class="badge"
            :class="
              u.severity === 'critical' || u.category === 'security'
                ? 'badge-danger'
                : 'badge-warning'
            "
          >
            {{ severityLabel(u.severity) }}
          </span>
        </div>
      </div>
    </section>

    <section v-if="appUpdates.length > 0" aria-label="Software-Updates">
      <h3 class="section-heading">Software-Updates ({{ appUpdates.length }})</h3>
      <div class="list-panel">
        <div
          v-for="app in appUpdates"
          :key="app.package_id"
          class="list-row"
        >
          <div style="display: flex; align-items: center; gap: 12px;">
            <div class="app-icon avatar-browser" style="width: 38px; height: 38px; font-size: 13px;" aria-hidden="true">
              {{ (app.name.slice(0, 2) || 'SW').toUpperCase() }}
            </div>
            <div>
              <div style="font-weight: 600; font-size: 14px; color: var(--text-main);">
                {{ app.name }}
              </div>
              <div style="font-size: 12px; color: var(--text-muted); margin-top: 2px;">
                Installiert: v{{ app.installed_version || '–' }} → Neu: <strong>v{{ app.version }}</strong>
              </div>
            </div>
          </div>
          <button
            type="button"
            class="btn btn-primary"
            :disabled="!allowUserOsUpdates || !allowSelfService || busy"
            @click="$emit('trigger-app', app.package_id)"
          >
            Jetzt installieren
          </button>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import type { CatalogViewItem, PendingOsUpdate } from '../api';

withDefaults(
  defineProps<{
    osUpdates: PendingOsUpdate[];
    appUpdates: CatalogViewItem[];
    busy?: boolean;
    allowUserOsUpdates?: boolean;
    allowSelfService?: boolean;
  }>(),
  {
    allowUserOsUpdates: true,
    allowSelfService: true,
  }
);

defineEmits<{
  (e: 'trigger-all'): void;
  (e: 'trigger-app', packageId: string): void;
}>();

function formatSize(bytes: number): string {
  const mb = bytes / (1024 * 1024);
  return `${mb.toFixed(1)} MB`;
}

function severityLabel(sev: string): string {
  switch (sev.toLowerCase()) {
    case 'critical':
      return 'Kritisch';
    case 'important':
      return 'Wichtig';
    case 'moderate':
      return 'Mittel';
    default:
      return sev;
  }
}
</script>
