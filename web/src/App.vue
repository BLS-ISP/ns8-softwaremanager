<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { RouterLink, RouterView, useRoute, useRouter } from 'vue-router'
import {
  CalendarClock,
  FolderKanban,
  KeyRound,
  Laptop,
  LayoutDashboard,
  LogOut,
  Menu,
  Moon,
  Package,
  PanelLeftClose,
  PanelLeftOpen,
  Rocket,
  RotateCcw,
  ShieldCheck,
  Sun,
  Wrench,
  X,
} from 'lucide-vue-next'
import BrandLogo from '@/components/BrandLogo.vue'
import ToastContainer from '@/components/ToastContainer.vue'
import { useAuth } from '@/composables/useAuth'
import { useTheme } from '@/composables/useTheme'

const SIDEBAR_COLLAPSED_KEY = 'swm_sidebar_collapsed'

const route = useRoute()
const router = useRouter()
const auth = useAuth()
const { isDark, toggleTheme, syncThemeFromStorage } = useTheme()

const mobileOpen = ref(false)
const sidebarCollapsed = ref(readInitialCollapsed())
const isPublicRoute = computed(() => Boolean(route.meta.public))

function readInitialCollapsed(): boolean {
  try {
    return localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === '1'
  } catch {
    return false
  }
}

function toggleSidebarCollapsed(): void {
  sidebarCollapsed.value = !sidebarCollapsed.value
  try {
    localStorage.setItem(SIDEBAR_COLLAPSED_KEY, sidebarCollapsed.value ? '1' : '0')
  } catch {
    // ignore storage restrictions
  }
}

const navItems = [
  { to: '/', label: 'Dashboard', icon: LayoutDashboard },
  { to: '/devices', label: 'Geräte & Flotte', icon: Laptop },
  { to: '/groups', label: 'Gerätegruppen', icon: FolderKanban },
  { to: '/patches', label: 'OS-Patching', icon: Wrench },
  { to: '/catalog', label: 'Software-Katalog', icon: Package },
  { to: '/deployments', label: 'Bereitstellungen', icon: Rocket },
  { to: '/jobs', label: 'Aufträge & Wartung', icon: CalendarClock },
  { to: '/policies', label: 'Richtlinien & Agent', icon: RotateCcw },
  { to: '/enrollment', label: 'Client-Enrollment', icon: KeyRound },
]

const currentSectionLabel = computed(() => {
  if (route.path.startsWith('/devices/')) return 'Gerätedetails & Inventar'
  const match = navItems.find((item) => isActive(item.to))
  return match?.label || 'NS8 SoftwareManager'
})

function isActive(targetPath: string): boolean {
  if (targetPath === '/') return route.path === '/'
  return route.path === targetPath || route.path.startsWith(`${targetPath}/`)
}

async function handleLogout() {
  auth.logout()
  await router.push('/login')
}

function onGlobalKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && mobileOpen.value) {
    mobileOpen.value = false
  }
}

function onUnauthorized() {
  if (!route.meta.public) {
    router.push('/login')
  }
}

onMounted(() => {
  syncThemeFromStorage()
  if (auth.isAuthenticated.value && !auth.user.value) {
    auth.fetchCurrentUser()
  }
  if (typeof window !== 'undefined') {
    window.addEventListener('swm:unauthorized', onUnauthorized)
    window.addEventListener('keydown', onGlobalKeyDown)
  }
})

onBeforeUnmount(() => {
  if (typeof window !== 'undefined') {
    window.removeEventListener('swm:unauthorized', onUnauthorized)
    window.removeEventListener('keydown', onGlobalKeyDown)
  }
})
</script>

<template>
  <div class="min-h-full bg-slate-50 text-slate-900 dark:bg-slate-950 dark:text-slate-100">
    <!-- Public Routes (Login) -->
    <RouterView v-if="isPublicRoute" />

    <!-- Protected Enterprise Admin Shell -->
    <div v-else class="flex min-h-screen">
      <!-- Mobile/Tablet Drawer Backdrop -->
      <div
        v-if="mobileOpen"
        data-testid="mobile-drawer-backdrop"
        class="fixed inset-0 z-40 bg-slate-950/65 backdrop-blur-xs lg:hidden"
        @click="mobileOpen = false"
      />

      <!-- Responsive Collapsible Sidebar Drawer -->
      <aside
        data-testid="app-sidebar"
        aria-label="Hauptnavigation"
        class="fixed inset-y-0 left-0 z-50 flex flex-col border-r border-slate-800 bg-slate-900 text-slate-300 transition-all duration-200 dark:border-slate-800/90 dark:bg-slate-900 lg:static lg:translate-x-0"
        :class="[
          mobileOpen ? 'translate-x-0' : '-translate-x-full',
          sidebarCollapsed ? 'w-64 lg:w-[4.5rem]' : 'w-64',
        ]"
      >
        <!-- Brand Header -->
        <div
          class="flex h-16 items-center justify-between border-b border-slate-800 px-4"
          :class="{ 'lg:justify-center lg:px-2': sidebarCollapsed }"
        >
          <RouterLink
            to="/"
            class="flex min-w-0 items-center gap-2.5"
            @click="mobileOpen = false"
          >
            <BrandLogo size="md" />
            <div :class="{ 'lg:hidden': sidebarCollapsed }" class="min-w-0">
              <div class="truncate text-sm font-bold leading-none text-white">SoftwareManager</div>
              <div class="mt-1 text-[10px] font-semibold uppercase tracking-wider text-cyan-400/90">
                NethServer 8 Modul
              </div>
            </div>
          </RouterLink>

          <button
            type="button"
            data-testid="mobile-drawer-close"
            aria-label="Navigation schließen"
            class="rounded-lg p-1.5 text-slate-400 hover:bg-slate-800 hover:text-white lg:hidden"
            @click="mobileOpen = false"
          >
            <X class="h-5 w-5" />
          </button>
        </div>

        <!-- Navigation Links -->
        <nav class="flex-1 space-y-1 overflow-y-auto px-3 py-4">
          <RouterLink
            v-for="item in navItems"
            :key="item.to"
            :to="item.to"
            :title="sidebarCollapsed ? item.label : undefined"
            class="group flex items-center gap-3 rounded-lg px-3 py-2.5 text-xs font-semibold transition-colors"
            :class="[
              isActive(item.to)
                ? 'bg-gradient-to-r from-cyan-600 to-blue-600 text-white shadow-sm'
                : 'text-slate-300 hover:bg-slate-800/90 hover:text-white',
              sidebarCollapsed ? 'lg:justify-center lg:px-2' : '',
            ]"
            @click="mobileOpen = false"
          >
            <component :is="item.icon" class="h-4 w-4 shrink-0" aria-hidden="true" />
            <span :class="{ 'lg:hidden': sidebarCollapsed }" class="truncate">{{ item.label }}</span>
          </RouterLink>
        </nav>

        <!-- Current Admin User & Collapse Footer -->
        <div class="border-t border-slate-800 p-3.5">
          <div
            class="flex items-center justify-between gap-2"
            :class="{ 'lg:flex-col lg:gap-2.5': sidebarCollapsed }"
          >
            <div :class="{ 'lg:hidden': sidebarCollapsed }" class="min-w-0">
              <div class="truncate text-xs font-semibold text-white">
                {{ auth.user.value?.username || 'Administrator' }}
              </div>
              <div class="text-[11px] text-slate-400">
                {{ auth.user.value?.auth_source === 'ldap' ? 'NS8 LDAP / AD' : 'Lokales Konto' }}
              </div>
            </div>

            <div class="flex items-center gap-1.5" :class="{ 'lg:flex-col': sidebarCollapsed }">
              <button
                type="button"
                data-testid="sidebar-collapse-btn"
                :aria-label="sidebarCollapsed ? 'Seitenleiste ausklappen' : 'Seitenleiste einklappen'"
                :title="sidebarCollapsed ? 'Seitenleiste ausklappen' : 'Seitenleiste einklappen'"
                class="hidden rounded-lg bg-slate-800 p-2 text-slate-300 transition hover:bg-slate-700 hover:text-white lg:inline-flex"
                @click="toggleSidebarCollapsed"
              >
                <PanelLeftOpen v-if="sidebarCollapsed" class="h-4 w-4" />
                <PanelLeftClose v-else class="h-4 w-4" />
              </button>

              <button
                type="button"
                data-testid="logout-btn"
                aria-label="Abmelden"
                title="Abmelden"
                class="rounded-lg bg-slate-800 p-2 text-slate-300 transition hover:bg-rose-600 hover:text-white"
                @click="handleLogout"
              >
                <LogOut class="h-4 w-4" />
              </button>
            </div>
          </div>
        </div>
      </aside>

      <!-- Main Workspace -->
      <div class="flex min-w-0 flex-1 flex-col">
        <!-- Sticky Top Utility Bar (Mobile Drawer Trigger + Breadcrumb + Theme Switch) -->
        <header
          class="sticky top-0 z-30 flex h-14 items-center justify-between gap-3 border-b border-slate-200 bg-white/90 px-4 backdrop-blur-md sm:px-6 lg:px-8 dark:border-slate-800 dark:bg-slate-900/90"
        >
          <div class="flex min-w-0 items-center gap-3">
            <button
              type="button"
              data-testid="mobile-drawer-open"
              aria-label="Navigation öffnen"
              :aria-expanded="mobileOpen"
              class="rounded-lg p-1.5 text-slate-600 hover:bg-slate-100 lg:hidden dark:text-slate-300 dark:hover:bg-slate-800"
              @click="mobileOpen = true"
            >
              <Menu class="h-5 w-5" />
            </button>

            <BrandLogo size="sm" class="lg:hidden" />

            <div class="truncate text-xs font-semibold text-slate-700 sm:text-sm dark:text-slate-200">
              {{ currentSectionLabel }}
            </div>
          </div>

          <div class="flex items-center gap-2.5">
            <span
              class="hidden items-center gap-1.5 rounded-full border border-emerald-200 bg-emerald-50/80 px-2.5 py-1 text-[11px] font-medium text-emerald-800 sm:inline-flex dark:border-emerald-500/25 dark:bg-emerald-500/10 dark:text-emerald-300"
            >
              <ShieldCheck class="h-3.5 w-3.5 text-emerald-600 dark:text-emerald-400" aria-hidden="true" />
              Ed25519 Signatur aktiv
            </span>

            <button
              type="button"
              data-testid="theme-toggle-btn"
              :aria-label="isDark ? 'Hellmodus aktivieren' : 'Dunkelmodus aktivieren'"
              :title="isDark ? 'Zu hellem Design wechseln' : 'Zu dunklem Design wechseln'"
              class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-slate-50 px-2.5 py-1.5 text-xs font-semibold text-slate-700 transition hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
              @click="toggleTheme"
            >
              <Sun v-if="isDark" class="h-4 w-4 text-amber-400" aria-hidden="true" />
              <Moon v-else class="h-4 w-4 text-slate-600" aria-hidden="true" />
              <span class="hidden sm:inline">{{ isDark ? 'Hell' : 'Dunkel' }}</span>
            </button>
          </div>
        </header>

        <main class="flex-1 p-4 sm:p-6 lg:p-8">
          <div class="mx-auto max-w-7xl">
            <RouterView />
          </div>
        </main>
      </div>
    </div>

    <!-- Global Toast Notification Stack -->
    <ToastContainer />
  </div>
</template>
