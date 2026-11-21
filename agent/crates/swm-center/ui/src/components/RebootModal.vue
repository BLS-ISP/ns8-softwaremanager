<template>
  <div
    v-if="visible"
    class="modal-backdrop"
    role="dialog"
    aria-modal="true"
    aria-labelledby="reboot-modal-title"
    data-testid="reboot-modal"
  >
    <div class="modal-panel">
      <div style="display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 12px;">
        <div style="display: flex; align-items: center; gap: 10px;">
          <svg
            width="22"
            height="22"
            viewBox="0 0 24 24"
            fill="none"
            stroke="var(--danger)"
            stroke-width="2"
            aria-hidden="true"
          >
            <path d="M12 9v4m0 4h.01" stroke-linecap="round" />
            <path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" stroke-linejoin="round" />
          </svg>
          <h2 id="reboot-modal-title" style="margin: 0; font-size: 19px; color: var(--danger-soft-fg);">
            Systemneustart erforderlich
          </h2>
        </div>
        <span class="badge badge-warning">Updates abgeschlossen</span>
      </div>

      <p style="margin: 0 0 16px; font-size: 14px; color: var(--text-secondary); line-height: 1.55;">
        Wichtige Sicherheits- oder Anwendungsupdates wurden installiert. Bitte speichern Sie geöffnete Dokumente und starten Sie Ihren Computer neu, um den Vorgang abzuschließen.
      </p>

      <div
        v-if="rebootState.policy === 'grace_period' && rebootState.deadline_unix"
        class="countdown-box"
        data-testid="deadline-countdown-box"
      >
        <div class="countdown-label">
          Verbleibende Karenzzeit bis zum Pflicht-Neustart
        </div>
        <div class="countdown-value" data-testid="countdown-value">
          {{ formattedCountdown }}
        </div>
        <div class="progress-track" aria-hidden="true" style="margin-top: 10px;">
          <div class="progress-bar-fill" :style="{ width: `${remainingPercent}%` }"></div>
        </div>
      </div>

      <div style="display: flex; flex-wrap: wrap; gap: 10px; justify-content: flex-end;">
        <button
          v-if="rebootState.policy === 'grace_period'"
          type="button"
          class="btn btn-outline"
          :disabled="deadlinePassed || busy"
          data-testid="snooze-1h-btn"
          @click="$emit('snooze', 60)"
        >
          Später erinnern: 1h
        </button>

        <button
          v-if="rebootState.policy === 'grace_period'"
          type="button"
          class="btn btn-outline"
          :disabled="deadlinePassed || busy"
          data-testid="snooze-4h-btn"
          @click="$emit('snooze', 240)"
        >
          Später erinnern: 4h
        </button>

        <button
          type="button"
          class="btn btn-danger-solid"
          :disabled="busy"
          data-testid="confirm-reboot-btn"
          @click="$emit('confirm')"
        >
          Jetzt neu starten
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import type { RebootState } from '../api';

const props = defineProps<{
  visible: boolean;
  rebootState: RebootState;
  busy?: boolean;
  nowUnixOverride?: number;
}>();

const emit = defineEmits<{
  (e: 'snooze', minutes: number): void;
  (e: 'confirm'): void;
  (e: 'close'): void;
}>();

const currentUnix = ref<number>(Math.floor(Date.now() / 1000));
let timerId: ReturnType<typeof setInterval> | null = null;

function onKeyDown(ev: KeyboardEvent) {
  if (ev.key === 'Escape' && props.visible && !deadlinePassed.value) {
    emit('close');
  }
}

onMounted(() => {
  timerId = setInterval(() => {
    currentUnix.value = Math.floor(Date.now() / 1000);
  }, 1000);
  window.addEventListener('keydown', onKeyDown);
});

onUnmounted(() => {
  if (timerId) {
    clearInterval(timerId);
  }
  window.removeEventListener('keydown', onKeyDown);
});

const effectiveNow = computed(() => props.nowUnixOverride ?? currentUnix.value);

const remainingSeconds = computed(() => {
  if (!props.rebootState.deadline_unix) return 0;
  return Math.max(0, props.rebootState.deadline_unix - effectiveNow.value);
});

const remainingPercent = computed(() => {
  const totalSecs = Math.max(60, (props.rebootState.grace_period_minutes || 240) * 60);
  return Math.min(100, Math.max(0, Math.round((remainingSeconds.value / totalSecs) * 100)));
});

const deadlinePassed = computed(() => {
  if (!props.rebootState.deadline_unix) return false;
  return effectiveNow.value >= props.rebootState.deadline_unix;
});

const formattedCountdown = computed(() => {
  const secs = remainingSeconds.value;
  if (secs <= 0) return '00:00:00 (Fällig)';
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${pad(h)}:${pad(m)}:${pad(s)}`;
});
</script>
