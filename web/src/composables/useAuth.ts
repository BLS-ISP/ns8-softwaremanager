import { computed, ref } from 'vue'
import {
  adminApi,
  clearAuthSession,
  getStoredToken,
  getStoredUser,
  saveAuthSession,
} from '@/api/client'
import type { AdminUser, LoginPayload } from '@/api/types'

const token = ref<string | null>(getStoredToken())
const user = ref<AdminUser | null>(getStoredUser())

if (typeof window !== 'undefined') {
  window.addEventListener('swm:unauthorized', () => {
    token.value = null
    user.value = null
  })
}

export function useAuth() {
  const isAuthenticated = computed(() => Boolean(token.value || getStoredToken()))

  function syncFromStorage(): void {
    token.value = getStoredToken()
    user.value = getStoredUser()
  }

  async function login(payload: LoginPayload): Promise<void> {
    const response = await adminApi.login(payload)
    token.value = response.access_token
    user.value = response.user
    saveAuthSession(response.access_token, response.user)
  }

  async function fetchCurrentUser(): Promise<AdminUser | null> {
    syncFromStorage()
    if (!token.value) return null
    try {
      const me = await adminApi.getMe()
      user.value = me
      saveAuthSession(token.value, me)
      return me
    } catch {
      token.value = null
      user.value = null
      clearAuthSession()
      return null
    }
  }

  function logout(): void {
    token.value = null
    user.value = null
    clearAuthSession()
  }

  return {
    token,
    user,
    isAuthenticated,
    syncFromStorage,
    login,
    fetchCurrentUser,
    logout,
  }
}
