import { computed, ref } from 'vue'

export type ThemeMode = 'light' | 'dark'

const THEME_STORAGE_KEY = 'swm_theme'

function detectInitialTheme(): ThemeMode {
  try {
    const stored = localStorage.getItem(THEME_STORAGE_KEY)
    if (stored === 'dark' || stored === 'light') {
      return stored
    }
  } catch {
    // ignore storage errors
  }

  if (
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-color-scheme: dark)').matches
  ) {
    return 'dark'
  }
  return 'light'
}

function applyDomTheme(mode: ThemeMode): void {
  if (typeof document === 'undefined') return
  const root = document.documentElement
  if (mode === 'dark') {
    root.classList.add('dark')
    root.style.colorScheme = 'dark'
  } else {
    root.classList.remove('dark')
    root.style.colorScheme = 'light'
  }
}

const currentTheme = ref<ThemeMode>(detectInitialTheme())
applyDomTheme(currentTheme.value)

export function useTheme() {
  const isDark = computed(() => currentTheme.value === 'dark')

  function setTheme(mode: ThemeMode): void {
    currentTheme.value = mode
    try {
      localStorage.setItem(THEME_STORAGE_KEY, mode)
    } catch {
      // ignore storage errors
    }
    applyDomTheme(mode)
  }

  function syncThemeFromStorage(): void {
    const detected = detectInitialTheme()
    currentTheme.value = detected
    applyDomTheme(detected)
  }

  function toggleTheme(): void {
    setTheme(currentTheme.value === 'dark' ? 'light' : 'dark')
  }

  return {
    theme: currentTheme,
    isDark,
    setTheme,
    toggleTheme,
    syncThemeFromStorage,
  }
}
