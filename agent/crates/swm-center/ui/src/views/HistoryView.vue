<template>
  <div>
    <div class="view-header">
      <div>
        <h2 class="view-title">Aktivitäten &amp; Verlauf</h2>
        <p class="view-subtitle">
          Übersicht laufender und abgeschlossener Software- und Update-Vorgänge auf diesem Gerät.
        </p>
      </div>
    </div>

    <section v-if="activeJobs.length > 0" style="margin-bottom: 24px;" aria-label="Laufende Vorgänge">
      <h3 class="section-heading">Laufende Vorgänge ({{ activeJobs.length }})</h3>
      <div
        v-for="job in activeJobs"
        :key="job.job_id"
        class="active-job-card"
        role="status"
      >
        <div style="display: flex; align-items: center; justify-content: space-between; gap: 10px;">
          <div>
            <strong>{{ job.target_name }}</strong>
            <span v-if="job.version"> (v{{ job.version }})</span>
            <span> – {{ job.message || 'Wird installiert…' }}</span>
          </div>
          <span class="badge badge-progress">Wird installiert…</span>
        </div>
        <div class="progress-track" role="progressbar" aria-label="Laufender Vorgang">
          <div class="progress-bar-indeterminate"></div>
        </div>
      </div>
    </section>

    <div v-if="history.length === 0" class="empty-panel" role="status">
      Noch keine abgeschlossenen Aktivitäten protokolliert.
    </div>

    <section v-else aria-label="Abgeschlossene Aktivitäten">
      <h3 class="section-heading">Protokollierte Ereignisse</h3>
      <div class="list-panel">
        <div
          v-for="entry in history"
          :key="entry.job_id + entry.timestamp"
          class="list-row"
        >
          <div>
            <div style="font-weight: 600; font-size: 14px; color: var(--text-main);">
              {{ entry.target_name }}
              <span v-if="entry.version" style="font-weight: 400; color: var(--text-muted);">
                (v{{ entry.version }})
              </span>
            </div>
            <div style="font-size: 12px; color: var(--text-muted); margin-top: 3px;">
              {{ actionLabel(entry.action) }} · {{ entry.timestamp }}
              <span v-if="entry.message"> · {{ entry.message }}</span>
              <span v-if="entry.exit_code !== null && entry.exit_code !== undefined">
                · Exit-Code {{ entry.exit_code }}
              </span>
            </div>
          </div>

          <div style="display: flex; gap: 8px; align-items: center; flex-shrink: 0;">
            <span v-if="entry.reboot_required" class="badge badge-warning">
              Neustart erforderlich
            </span>
            <span
              class="badge"
              :class="
                entry.status === 'succeeded'
                  ? 'badge-installed'
                  : entry.status === 'waiting_for_process'
                    ? 'badge-warning'
                    : 'badge-danger'
              "
            >
              {{ historyStatusLabel(entry.status) }}
            </span>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import type { JobActivityEntry } from '../api';

defineProps<{
  activeJobs: JobActivityEntry[];
  history: JobActivityEntry[];
}>();

function actionLabel(action: string): string {
  switch (action.toLowerCase()) {
    case 'install':
      return 'Installation';
    case 'update':
      return 'Aktualisierung';
    case 'uninstall':
      return 'Deinstallation';
    case 'scan':
      return 'Update-Scan';
    default:
      return action;
  }
}

function historyStatusLabel(status: string): string {
  switch (status) {
    case 'succeeded':
      return 'Erfolgreich';
    case 'waiting_for_process':
      return 'Wartet auf Schließen der App';
    case 'failed':
      return 'Fehlgeschlagen';
    default:
      return status;
  }
}
</script>
