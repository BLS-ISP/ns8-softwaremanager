import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  adminApi,
  clearAuthSession,
  getStoredToken,
  saveAuthSession,
} from '@/api/client'
import { createAppRouter } from '@/router'
import { applyHeadlessToggle } from '@/utils/format'

describe('API Client & Auth Guard', () => {
  beforeEach(() => {
    localStorage.clear()
    vi.restoreAllMocks()
  })

  it('injects Authorization Bearer header on protected /api/v1/admin/* requests', async () => {
    saveAuthSession('jwt-secret-token-123', {
      id: 'u-1',
      username: 'admin',
      role: 'admin',
      auth_source: 'local',
    })

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          id: 'u-1',
          username: 'admin',
          role: 'admin',
          auth_source: 'local',
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      ),
    )

    const me = await adminApi.getMe()
    expect(me.username).toBe('admin')
    expect(fetchMock).toHaveBeenCalledTimes(1)

    const [url, init] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/admin/auth/me')
    const headers = init?.headers as Headers
    expect(headers.get('Authorization')).toBe('Bearer jwt-secret-token-123')
  })

  it('stores JWT session on login without sending stale Authorization header', async () => {
    saveAuthSession('old-expired-token')

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          access_token: 'fresh-ldap-jwt',
          token_type: 'bearer',
          user: { id: 'u-2', username: 'domainadmin', role: 'admin', auth_source: 'ldap' },
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      ),
    )

    const res = await adminApi.login({
      username: 'domainadmin',
      password: 'secret',
      auth_source: 'ldap',
    })

    expect(res.access_token).toBe('fresh-ldap-jwt')
    expect(getStoredToken()).toBe('fresh-ldap-jwt')

    const [url, init] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/admin/auth/login')
    const headers = init?.headers as Headers
    expect(headers.get('Authorization')).toBeNull()
    expect(JSON.parse(String(init?.body))).toEqual({
      username: 'domainadmin',
      password: 'secret',
      auth_source: 'ldap',
    })
  })

  it('clears session and dispatches swm:unauthorized on HTTP 401', async () => {
    saveAuthSession('invalid-jwt')
    const eventSpy = vi.fn()
    window.addEventListener('swm:unauthorized', eventSpy)

    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ detail: 'Invalid token' }), {
        status: 401,
        headers: { 'Content-Type': 'application/json' },
      }),
    )

    await expect(adminApi.listDevices()).rejects.toThrow('Invalid token')
    expect(getStoredToken()).toBeNull()
    expect(eventSpy).toHaveBeenCalledTimes(1)

    window.removeEventListener('swm:unauthorized', eventSpy)
  })

  it('uploads custom installer in sequential chunks with progress callback', async () => {
    saveAuthSession('valid-jwt')

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (_input, init) => {
      const form = init?.body as FormData
      const chunkIdx = Number(form.get('chunk_index'))
      const total = Number(form.get('total_chunks'))
      const completed = chunkIdx === total - 1
      return new Response(
        JSON.stringify({
          package_id: 'pkg-99',
          chunk_index: chunkIdx,
          total_chunks: total,
          completed,
          sha256: completed ? 'a'.repeat(64) : null,
          file_size: completed ? 2500 : null,
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      )
    })

    const content = new Uint8Array(2500).fill(65)
    const file = new File([content], 'installer.msi', { type: 'application/octet-stream' })
    const progressUpdates: Array<[number, number, number]> = []

    const result = await adminApi.uploadCatalogFileChunked(
      'pkg-99',
      file,
      (pct, current, total) => progressUpdates.push([pct, current, total]),
      1000, // 1000-byte chunks -> 3 chunks
    )

    expect(fetchMock).toHaveBeenCalledTimes(3)
    expect(progressUpdates).toEqual([
      [33, 1, 3],
      [67, 2, 3],
      [100, 3, 3],
    ])
    expect(result.completed).toBe(true)
    expect(result.sha256).toBe('a'.repeat(64))
  })

  it('toggles headless query flag in PowerShell and Bash enrollment commands', () => {
    const ps1 = "irm 'https://swm.example.org/dl/install.ps1?token=swm_tok_123&headless=0' | iex"
    expect(applyHeadlessToggle(ps1, true)).toContain('headless=1')
    expect(applyHeadlessToggle(applyHeadlessToggle(ps1, true), false)).toContain('headless=0')
  })

  it('redirects unauthenticated users to /login and authenticated users away from /login', async () => {
    clearAuthSession()
    // Stub fetch for any mounted view calls during navigation
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          total_devices: 0,
          online_devices: 0,
          pending_approval_devices: 0,
          reboot_required_devices: 0,
          patch_compliance_percent: 100,
          pending_critical_patches: 0,
          recent_jobs: [],
          os_breakdown: { windows: 0, linux: 0 },
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      ),
    )

    const router = createAppRouter(true)
    await router.push('/devices')
    await router.isReady()

    expect(router.currentRoute.value.path).toBe('/login')
    expect(router.currentRoute.value.query.redirect).toBe('/devices')

    saveAuthSession('valid-token-xyz')
    await router.push('/login')
    expect(router.currentRoute.value.path).toBe('/')
  })
})
