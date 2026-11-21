<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  variant:
    | 'online'
    | 'offline'
    | 'approved'
    | 'pending'
    | 'revoked'
    | 'reboot'
    | 'ok'
    | 'job'
    | 'severity'
  value?: string
  label?: string
}>()

const badgeConfig = computed(() => {
  if (props.variant === 'online') {
    return {
      text: props.label || 'Online',
      classes:
        'bg-emerald-50 text-emerald-700 ring-emerald-600/20 dark:bg-emerald-500/15 dark:text-emerald-300 dark:ring-emerald-400/30',
      dot: 'bg-emerald-500 animate-pulse',
    }
  }
  if (props.variant === 'offline') {
    return {
      text: props.label || 'Offline',
      classes:
        'bg-slate-100 text-slate-600 ring-slate-500/20 dark:bg-slate-800 dark:text-slate-300 dark:ring-slate-600/40',
      dot: 'bg-slate-400 dark:bg-slate-500',
    }
  }
  if (props.variant === 'approved') {
    return {
      text: props.label || 'Freigegeben',
      classes:
        'bg-emerald-50 text-emerald-700 ring-emerald-600/20 dark:bg-emerald-500/15 dark:text-emerald-300 dark:ring-emerald-400/30',
      dot: 'bg-emerald-500',
    }
  }
  if (props.variant === 'pending') {
    return {
      text: props.label || 'Freigabe ausstehend',
      classes:
        'bg-amber-50 text-amber-800 ring-amber-600/25 dark:bg-amber-500/15 dark:text-amber-300 dark:ring-amber-400/30',
      dot: 'bg-amber-500',
    }
  }
  if (props.variant === 'revoked') {
    return {
      text: props.label || 'Gesperrt',
      classes:
        'bg-rose-50 text-rose-700 ring-rose-600/20 dark:bg-rose-500/15 dark:text-rose-300 dark:ring-rose-400/30',
      dot: 'bg-rose-500',
    }
  }
  if (props.variant === 'reboot') {
    return {
      text: props.label || 'Neustart erforderlich',
      classes:
        'bg-amber-50 text-amber-800 ring-amber-600/25 dark:bg-amber-500/15 dark:text-amber-300 dark:ring-amber-400/30',
      dot: 'bg-amber-500',
    }
  }
  if (props.variant === 'ok') {
    return {
      text: props.label || 'Aktuell',
      classes:
        'bg-emerald-50 text-emerald-700 ring-emerald-600/20 dark:bg-emerald-500/15 dark:text-emerald-300 dark:ring-emerald-400/30',
      dot: 'bg-emerald-500',
    }
  }
  if (props.variant === 'severity') {
    const v = (props.value || '').toLowerCase()
    if (v === 'critical') {
      return {
        text: 'Kritisch',
        classes:
          'bg-rose-50 text-rose-700 ring-rose-600/25 dark:bg-rose-500/15 dark:text-rose-300 dark:ring-rose-400/30',
        dot: 'bg-rose-600 dark:bg-rose-400',
      }
    }
    if (v === 'important' || v === 'security' || v === 'high') {
      return {
        text: v === 'security' ? 'Sicherheit' : 'Wichtig',
        classes:
          'bg-orange-50 text-orange-700 ring-orange-600/25 dark:bg-orange-500/15 dark:text-orange-300 dark:ring-orange-400/30',
        dot: 'bg-orange-500 dark:bg-orange-400',
      }
    }
    if (v === 'moderate' || v === 'medium') {
      return {
        text: 'Mittel',
        classes:
          'bg-amber-50 text-amber-700 ring-amber-600/20 dark:bg-amber-500/15 dark:text-amber-300 dark:ring-amber-400/30',
        dot: 'bg-amber-500 dark:bg-amber-400',
      }
    }
    return {
      text: props.label || props.value || 'Optional',
      classes:
        'bg-slate-100 text-slate-700 ring-slate-500/20 dark:bg-slate-800 dark:text-slate-300 dark:ring-slate-600/40',
      dot: 'bg-slate-400 dark:bg-slate-500',
    }
  }

  // job status
  const j = (props.value || '').toLowerCase()
  if (j === 'succeeded' || j === 'completed' || j === 'success') {
    return {
      text: 'Erfolgreich',
      classes:
        'bg-emerald-50 text-emerald-700 ring-emerald-600/20 dark:bg-emerald-500/15 dark:text-emerald-300 dark:ring-emerald-400/30',
      dot: 'bg-emerald-500',
    }
  }
  if (j === 'running' || j === 'in_progress') {
    return {
      text: 'Wird ausgeführt',
      classes:
        'bg-sky-50 text-sky-700 ring-sky-600/20 dark:bg-sky-500/15 dark:text-sky-300 dark:ring-sky-400/30',
      dot: 'bg-sky-500 animate-pulse',
    }
  }
  if (j === 'failed' || j === 'error') {
    return {
      text: 'Fehlgeschlagen',
      classes:
        'bg-rose-50 text-rose-700 ring-rose-600/20 dark:bg-rose-500/15 dark:text-rose-300 dark:ring-rose-400/30',
      dot: 'bg-rose-500',
    }
  }
  if (j === 'waiting_for_process') {
    return {
      text: 'Prozess geöffnet',
      classes:
        'bg-amber-50 text-amber-800 ring-amber-600/25 dark:bg-amber-500/15 dark:text-amber-300 dark:ring-amber-400/30',
      dot: 'bg-amber-500',
    }
  }
  return {
    text:
      j === 'queued' || j === 'pending'
        ? 'Warteschlange'
        : props.label || props.value || 'Ausstehend',
    classes:
      'bg-slate-100 text-slate-700 ring-slate-500/20 dark:bg-slate-800 dark:text-slate-300 dark:ring-slate-600/40',
    dot: 'bg-slate-400 dark:bg-slate-500',
  }
})
</script>

<template>
  <span
    class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium ring-1 ring-inset"
    :class="badgeConfig.classes"
  >
    <span class="h-1.5 w-1.5 rounded-full" :class="badgeConfig.dot" />
    {{ badgeConfig.text }}
  </span>
</template>
