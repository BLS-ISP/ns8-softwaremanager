import type { DeploymentMode, RebootPolicy, SourceType } from '@/api/types'

export const WEEKDAY_LABELS = [
  { value: 0, short: 'Mo', full: 'Montag' },
  { value: 1, short: 'Di', full: 'Dienstag' },
  { value: 2, short: 'Mi', full: 'Mittwoch' },
  { value: 3, short: 'Do', full: 'Donnerstag' },
  { value: 4, short: 'Fr', full: 'Freitag' },
  { value: 5, short: 'Sa', full: 'Samstag' },
  { value: 6, short: 'So', full: 'Sonntag' },
] as const

export function formatDateTime(iso?: string | null): string {
  if (!iso) return '—'
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return new Intl.DateTimeFormat('de-DE', {
    dateStyle: 'short',
    timeStyle: 'short',
  }).format(date)
}

export function formatBytes(bytes?: number | null): string {
  if (bytes === undefined || bytes === null || Number.isNaN(bytes)) return '—'
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let idx = 0
  let value = bytes
  while (value >= 1024 && idx < units.length - 1) {
    value /= 1024
    idx++
  }
  return `${value.toFixed(idx === 0 ? 0 : 1)} ${units[idx]}`
}

export function formatRebootPolicy(policy: RebootPolicy, graceMinutes?: number): string {
  switch (policy) {
    case 'never':
      return 'Kein automatischer Neustart'
    case 'maintenance_window':
      return 'Automatisch im Wartungsfenster'
    case 'grace_period':
      return `Benutzer-Karenzzeit (${graceMinutes ?? 240} Min.)`
    default:
      return String(policy)
  }
}

export function formatSourceType(sourceType: SourceType): string {
  switch (sourceType) {
    case 'winget':
      return 'WinGet'
    case 'apt':
      return 'APT'
    case 'dnf':
      return 'DNF'
    case 'flatpak':
      return 'Flatpak'
    case 'custom':
      return 'Eigenes Paket'
    default:
      return String(sourceType)
  }
}

export function formatDeploymentMode(mode: DeploymentMode): string {
  switch (mode) {
    case 'required':
      return 'Pflicht (Automatisch)'
    case 'available':
      return 'Verfügbar (Software Center)'
    case 'uninstall':
      return 'Deinstallation'
    default:
      return String(mode)
  }
}

export function formatWeekdays(days: number[]): string {
  if (!days || days.length === 0) return 'Keine Tage'
  if (days.length === 7) return 'Täglich'
  return [...days]
    .sort((a, b) => a - b)
    .map((d) => WEEKDAY_LABELS.find((w) => w.value === d)?.short ?? String(d))
    .join(', ')
}

export function applyHeadlessToggle(cmd: string, headless: boolean): string {
  if (!cmd) return ''
  const targetFlag = headless ? 'headless=1' : 'headless=0'
  if (cmd.includes('headless=0') || cmd.includes('headless=1')) {
    return cmd.replace(/headless=[01]/g, targetFlag)
  }
  if (cmd.includes('?token=')) {
    return cmd.replace(/(\?token=[^'"&\s]+)/, `$1&${targetFlag}`)
  }
  return cmd
}
