import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import App from '@/App.vue'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import ToastContainer from '@/components/ToastContainer.vue'
import OsBadge from '@/components/OsBadge.vue'
import DevicesView from '@/views/DevicesView.vue'
import CatalogView from '@/views/CatalogView.vue'
import EnrollmentView from '@/views/EnrollmentView.vue'
import { saveAuthSession } from '@/api/client'
import { useTheme } from '@/composables/useTheme'
import { useToast } from '@/composables/useToast'
import { createAppRouter } from '@/router'

describe('Theme Toggle, Safe ConfirmDialog, ToastContainer & Responsive Shell UX', () => {
  beforeEach(() => {
    localStorage.clear()
    document.documentElement.classList.remove('dark')
    const { setTheme } = useTheme()
    setTheme('light')
    const { clearAll } = useToast()
    clearAll()
    saveAuthSession('admin-ux-token', {
      id: 'u-1',
      username: 'admin',
      role: 'admin',
      auth_source: 'local',
    })
    vi.restoreAllMocks()
  })

  it('toggles Light and Dark mode in App shell and persists preference to localStorage', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          total_devices: 5,
          online_devices: 4,
          pending_approval_devices: 0,
          reboot_required_devices: 1,
          patch_compliance_percent: 92.0,
          pending_critical_patches: 0,
          recent_jobs: [],
          os_breakdown: { windows: 3, linux: 2 },
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      ),
    )

    const router = createAppRouter(true)
    await router.push('/')
    await router.isReady()

    const wrapper = mount(App, {
      global: { plugins: [router] },
    })
    await flushPromises()

    // Custom brand emblem is rendered in the sidebar
    expect(wrapper.find('[data-testid="brand-logo"]').exists()).toBe(true)

    const themeBtn = wrapper.get('[data-testid="theme-toggle-btn"]')
    expect(document.documentElement.classList.contains('dark')).toBe(false)
    expect(themeBtn.attributes('aria-label')).toContain('Dunkelmodus')

    await themeBtn.trigger('click')
    expect(localStorage.getItem('swm_theme')).toBe('dark')
    expect(document.documentElement.classList.contains('dark')).toBe(true)
    expect(themeBtn.attributes('aria-label')).toContain('Hellmodus')

    // Mobile drawer opens and closes via Escape key
    await wrapper.get('[data-testid="mobile-drawer-open"]').trigger('click')
    expect(wrapper.find('[data-testid="mobile-drawer-backdrop"]').exists()).toBe(true)
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    await flushPromises()
    expect(wrapper.find('[data-testid="mobile-drawer-backdrop"]').exists()).toBe(false)
  })

  it('closes ConfirmDialog on Escape key and cancels without triggering action', async () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        open: true,
        title: 'Paket löschen?',
        message: 'Diese Aktion kann nicht rückgängig gemacht werden.',
        confirmLabel: 'Löschen',
      },
    })

    expect(wrapper.find('[data-testid="confirm-dialog"]').exists()).toBe(true)
    expect(wrapper.get('[data-testid="confirm-dialog"]').attributes('role')).toBe('alertdialog')

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    await flushPromises()

    expect(wrapper.emitted('cancel')).toBeTruthy()
    expect(wrapper.emitted('confirm')).toBeFalsy()
  })

  it('requires confirmation before revoking or deleting a device and emits toast feedback', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input)
      if (url === '/api/v1/admin/devices/dev-win-1/revoke' && init?.method === 'POST') {
        return new Response(
          JSON.stringify({
            id: 'dev-win-1',
            hostname: 'ws-design-01',
            os_family: 'windows',
            arch: 'amd64',
            agent_version: '1.0.0',
            status: 'revoked',
            is_online: false,
            reboot_required: false,
          }),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        )
      }
      if (url === '/api/v1/admin/devices/dev-win-1' && init?.method === 'DELETE') {
        return new Response(JSON.stringify({ status: 'deleted', id: 'dev-win-1' }), {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        })
      }
      if (url.startsWith('/api/v1/admin/devices')) {
        return new Response(
          JSON.stringify([
            {
              id: 'dev-win-1',
              hostname: 'ws-design-01',
              os_family: 'windows',
              os_name: 'Windows 11 Pro',
              arch: 'amd64',
              agent_version: '1.0.0',
              status: 'approved',
              is_online: true,
              reboot_required: false,
              ip_addresses: ['192.168.10.20'],
              group_ids: [],
              groups: [],
              tags: [],
            },
          ]),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        )
      }
      return new Response(JSON.stringify([]), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    })

    const router = createAppRouter(true)
    await router.push('/devices')
    await router.isReady()

    const wrapper = mount(DevicesView, {
      global: { plugins: [router] },
    })
    await flushPromises()

    // Click revoke -> opens ConfirmDialog WITHOUT firing POST .../revoke yet
    await wrapper.get('[data-testid="revoke-device-dev-win-1"]').trigger('click')
    expect(wrapper.find('[data-testid="confirm-dialog"]').exists()).toBe(true)
    expect(
      fetchMock.mock.calls.some(([u]) => String(u).endsWith('/dev-win-1/revoke')),
    ).toBe(false)

    // Confirm revoke -> calls API and pushes warning toast
    await wrapper.get('[data-testid="confirm-dialog-confirm"]').trigger('click')
    await flushPromises()

    expect(
      fetchMock.mock.calls.some(
        ([u, o]) => String(u) === '/api/v1/admin/devices/dev-win-1/revoke' && o?.method === 'POST',
      ),
    ).toBe(true)
    const { toasts } = useToast()
    expect(toasts.value.some((t) => t.title.includes('Gerät gesperrt'))).toBe(true)

    // Click delete -> cancel first -> then confirm delete
    await wrapper.get('[data-testid="delete-device-dev-win-1"]').trigger('click')
    expect(wrapper.find('[data-testid="confirm-dialog"]').exists()).toBe(true)
    await wrapper.get('[data-testid="confirm-dialog-cancel"]').trigger('click')
    expect(wrapper.find('[data-testid="confirm-dialog"]').exists()).toBe(false)
    expect(
      fetchMock.mock.calls.some(
        ([u, o]) => String(u) === '/api/v1/admin/devices/dev-win-1' && o?.method === 'DELETE',
      ),
    ).toBe(false)

    await wrapper.get('[data-testid="delete-device-dev-win-1"]').trigger('click')
    await wrapper.get('[data-testid="confirm-dialog-confirm"]').trigger('click')
    await flushPromises()
    expect(
      fetchMock.mock.calls.some(
        ([u, o]) => String(u) === '/api/v1/admin/devices/dev-win-1' && o?.method === 'DELETE',
      ),
    ).toBe(true)
  })

  it('requires confirmation before deleting catalog packages and revoking enrollment tokens', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input)
      if (url.startsWith('/api/v1/admin/catalog') && (!init?.method || init.method === 'GET')) {
        return new Response(
          JSON.stringify([
            {
              id: 'pkg-vlc',
              name: 'VLC media player',
              version: '3.0.21',
              vendor: 'VideoLAN',
              os_family: 'linux',
              source_type: 'flatpak',
              package_ref: 'org.videolan.VLC',
              blocked_processes: [],
              allow_uninstall: true,
              return_codes: {},
              detection_rule: {},
            },
          ]),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        )
      }
      if (url === '/api/v1/admin/catalog/pkg-vlc' && init?.method === 'DELETE') {
        return new Response(JSON.stringify({ status: 'deleted', id: 'pkg-vlc' }), {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        })
      }
      if (url === '/api/v1/admin/enrollment-tokens' && (!init?.method || init.method === 'GET')) {
        return new Response(
          JSON.stringify([
            {
              id: 'tok-9',
              name: 'Pilot Laptops',
              token: 'swm_tok_9',
              expires_at: '2026-12-31T00:00:00Z',
              max_uses: 10,
              uses_count: 1,
              auto_approve: true,
              group_id: null,
              revoked: false,
              install_ps1_cmd: 'irm https://swm.lan/dl/install.ps1?token=swm_tok_9&headless=0 | iex',
              install_sh_cmd: 'curl -fsSL https://swm.lan/dl/install.sh?token=swm_tok_9&headless=0 | sudo bash',
            },
          ]),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        )
      }
      if (url === '/api/v1/admin/enrollment-tokens/tok-9' && init?.method === 'DELETE') {
        return new Response(JSON.stringify({ status: 'deleted', id: 'tok-9' }), {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        })
      }
      return new Response(JSON.stringify([]), { status: 200 })
    })

    const router = createAppRouter(true)
    await router.push('/catalog')
    await router.isReady()

    const catalogWrapper = mount(CatalogView, { global: { plugins: [router] } })
    await flushPromises()
    await catalogWrapper.get('[data-testid="delete-catalog-pkg-vlc"]').trigger('click')
    expect(catalogWrapper.find('[data-testid="confirm-dialog"]').exists()).toBe(true)
    await catalogWrapper.get('[data-testid="confirm-dialog-confirm"]').trigger('click')
    await flushPromises()
    expect(
      fetchMock.mock.calls.some(
        ([u, o]) => String(u) === '/api/v1/admin/catalog/pkg-vlc' && o?.method === 'DELETE',
      ),
    ).toBe(true)

    const enrollWrapper = mount(EnrollmentView)
    await flushPromises()
    await enrollWrapper.get('[data-testid="revoke-token-tok-9"]').trigger('click')
    expect(enrollWrapper.find('[data-testid="confirm-dialog"]').exists()).toBe(true)
    await enrollWrapper.get('[data-testid="confirm-dialog-confirm"]').trigger('click')
    await flushPromises()
    expect(
      fetchMock.mock.calls.some(
        ([u, o]) => String(u) === '/api/v1/admin/enrollment-tokens/tok-9' && o?.method === 'DELETE',
      ),
    ).toBe(true)
  })

  it('renders ToastContainer notifications and OsBadge inline platform icons', async () => {
    const toast = useToast()
    toast.success('Rollout erfolgreich', '3 Systeme aktualisiert.')
    toast.error('Netzwerkfehler', 'Zeitüberschreitung.')

    const toastWrapper = mount(ToastContainer)
    const items = toastWrapper.findAll('[data-testid="toast-item"]')
    expect(items.length).toBe(2)
    expect(toastWrapper.text()).toContain('Rollout erfolgreich')
    expect(toastWrapper.text()).toContain('Netzwerkfehler')

    await items[0].get('[data-testid="toast-dismiss-btn"]').trigger('click')
    expect(toastWrapper.findAll('[data-testid="toast-item"]').length).toBe(1)

    const winBadge = mount(OsBadge, { props: { osFamily: 'windows', osName: 'Windows 11' } })
    const linBadge = mount(OsBadge, { props: { osFamily: 'linux', osName: 'Rocky Linux 9' } })
    expect(winBadge.get('[data-testid="os-badge"]').attributes('data-os-family')).toBe('windows')
    expect(linBadge.get('[data-testid="os-badge"]').attributes('data-os-family')).toBe('linux')
    expect(winBadge.find('svg').exists()).toBe(true)
    expect(linBadge.find('svg').exists()).toBe(true)
  })
})
