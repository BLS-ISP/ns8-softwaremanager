<script setup lang="ts">
import { ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Building2, KeyRound, Moon, Sun, UserCheck } from 'lucide-vue-next'
import BrandLogo from '@/components/BrandLogo.vue'
import { useAuth } from '@/composables/useAuth'
import { useTheme } from '@/composables/useTheme'
import type { AuthSource } from '@/api/types'

const router = useRouter()
const route = useRoute()
const auth = useAuth()
const { isDark, toggleTheme } = useTheme()

const username = ref('')
const password = ref('')
const authSource = ref<AuthSource>('local')
const loading = ref(false)
const error = ref<string | null>(null)

async function handleLogin() {
  error.value = null
  if (!username.value.trim() || !password.value) {
    error.value = 'Bitte geben Sie Benutzername und Kennwort ein.'
    return
  }

  loading.value = true
  try {
    await auth.login({
      username: username.value.trim(),
      password: password.value,
      auth_source: authSource.value,
    })
    const redirect = typeof route.query.redirect === 'string' ? route.query.redirect : '/'
    await router.push(redirect)
  } catch (err) {
    error.value =
      err instanceof Error
        ? err.message
        : 'Anmeldung fehlgeschlagen. Bitte prüfen Sie Ihre Zugangsdaten.'
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div
    class="relative flex min-h-screen flex-col justify-center bg-slate-100 px-4 py-12 transition-colors sm:px-6 lg:px-8 dark:bg-slate-950"
  >
    <!-- Top-Right Theme Switch -->
    <div class="absolute right-4 top-4 sm:right-6 sm:top-6">
      <button
        type="button"
        data-testid="login-theme-toggle"
        :aria-label="isDark ? 'Hellmodus aktivieren' : 'Dunkelmodus aktivieren'"
        class="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-3 py-1.5 text-xs font-semibold text-slate-700 shadow-xs transition hover:bg-slate-50 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
        @click="toggleTheme"
      >
        <Sun v-if="isDark" class="h-4 w-4 text-amber-400" aria-hidden="true" />
        <Moon v-else class="h-4 w-4 text-slate-600" aria-hidden="true" />
        <span>{{ isDark ? 'Hell' : 'Dunkel' }}</span>
      </button>
    </div>

    <div class="sm:mx-auto sm:w-full sm:max-w-md">
      <div class="mx-auto flex justify-center">
        <BrandLogo size="lg" />
      </div>
      <h1 class="mt-4 text-center text-2xl font-bold tracking-tight text-slate-900 dark:text-white">
        NS8 SoftwareManager
      </h1>
      <p class="mt-1.5 text-center text-sm text-slate-600 dark:text-slate-400">
        Zentrales Endpoint-, Patch- &amp; Software-Management für NethServer 8
      </p>
    </div>

    <div class="mt-8 sm:mx-auto sm:w-full sm:max-w-md">
      <div
        class="rounded-2xl border border-slate-200 bg-white px-6 py-8 shadow-xl sm:px-10 dark:border-slate-800 dark:bg-slate-900/95 dark:shadow-2xl"
      >
        <!-- Auth Source Selector -->
        <div
          class="mb-6 grid grid-cols-2 gap-2 rounded-xl bg-slate-100 p-1 ring-1 ring-slate-200 dark:bg-slate-950 dark:ring-slate-800"
        >
          <button
            type="button"
            data-testid="auth-source-local"
            class="flex items-center justify-center gap-2 rounded-lg py-2 text-xs font-semibold transition"
            :class="
              authSource === 'local'
                ? 'bg-sky-600 text-white shadow'
                : 'text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
            "
            @click="authSource = 'local'"
          >
            <UserCheck class="h-3.5 w-3.5" aria-hidden="true" />
            Lokaler Admin
          </button>
          <button
            type="button"
            data-testid="auth-source-ldap"
            class="flex items-center justify-center gap-2 rounded-lg py-2 text-xs font-semibold transition"
            :class="
              authSource === 'ldap'
                ? 'bg-sky-600 text-white shadow'
                : 'text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200'
            "
            @click="authSource = 'ldap'"
          >
            <Building2 class="h-3.5 w-3.5" aria-hidden="true" />
            NS8 Domain (LDAP/AD)
          </button>
        </div>

        <form class="space-y-5" @submit.prevent="handleLogin">
          <div
            v-if="error"
            data-testid="login-error"
            role="alert"
            class="rounded-lg border border-rose-200 bg-rose-50 px-3.5 py-2.5 text-xs font-medium text-rose-700 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-300"
          >
            {{ error }}
          </div>

          <div>
            <label for="username" class="block text-xs font-semibold text-slate-700 dark:text-slate-300">
              Benutzername
            </label>
            <input
              id="username"
              v-model="username"
              data-testid="login-username"
              type="text"
              autocomplete="username"
              placeholder="admin"
              class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 placeholder-slate-400 dark:border-slate-700 dark:bg-slate-950 dark:text-white dark:placeholder-slate-500"
            />
          </div>

          <div>
            <label for="password" class="block text-xs font-semibold text-slate-700 dark:text-slate-300">
              Kennwort
            </label>
            <input
              id="password"
              v-model="password"
              data-testid="login-password"
              type="password"
              autocomplete="current-password"
              placeholder="••••••••"
              class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 placeholder-slate-400 dark:border-slate-700 dark:bg-slate-950 dark:text-white dark:placeholder-slate-500"
            />
          </div>

          <button
            type="submit"
            data-testid="login-submit"
            :disabled="loading"
            class="flex w-full items-center justify-center gap-2 rounded-lg bg-gradient-to-r from-cyan-600 to-blue-600 px-4 py-2.5 text-sm font-semibold text-white shadow-sm transition hover:from-cyan-500 hover:to-blue-500 disabled:opacity-60"
          >
            <KeyRound class="h-4 w-4" aria-hidden="true" />
            {{ loading ? 'Anmeldung läuft…' : 'Anmelden' }}
          </button>
        </form>

        <div class="mt-6 border-t border-slate-200 pt-4 text-center text-[11px] text-slate-500 dark:border-slate-800 dark:text-slate-400">
          Abgesichert über Ed25519-Auftragssignierung &amp; NS8 Traefik TLS
        </div>
      </div>
    </div>
  </div>
</template>
