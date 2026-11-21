import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import DashboardView from '@/views/DashboardView.vue'
import DevicesView from '@/views/DevicesView.vue'
import { createAppRouter } from '@/router'
import { saveAuthSession } from '@/api/client'

describe('DashboardView & DevicesView', () => {
  beforeEach(() => {
    localStorage.clear()
    saveAuthSession('admin-test-token')
    vi.restoreAllMocks()
  })

  it('renders fleet KPI metrics, OS breakdown, and pending approval banner on Dashboard', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          total_devices: 24,
          online_devices: 19,
          pending_approval_devices: 3,
          reboot_required_devices: 4,
          patch_compliance_percent: 87.5,
          pending_critical_patches: 6,
          recent_jobs: [
            {
              id: 'job-1',
              device_id: 'dev-1',
              device_hostname: 'srv-db01',
              job_type: 'os_patch',
              action: 'install',
              status: 'succeeded',
              created_at: '2026-11-20T20:00:00Z',
            },
          ],
          os_breakdown: { windows: 15, linux: 9 },
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      ),
    )

    const router = createAppRouter(true)
    await router.push('/')
    await router.isReady()

    const wrapper = mount(DashboardView, {
      global: { plugins: [router] },
    })
    await flushPromises()

    expect(wrapper.get('[data-testid="kpi-total-devices"]').text()).toBe('24')
    expect(wrapper.get('[data-testid="kpi-online-devices"]').text()).toContain('19 Online')
    expect(wrapper.get('[data-testid="kpi-compliance"]').text()).toBe('87.5%')
    expect(wrapper.get('[data-testid="kpi-critical-patches"]').text()).toBe('6')
    expect(wrapper.get('[data-testid="kpi-reboot-devices"]').text()).toContain('4')
    expect(wrapper.get('[data-testid="os-windows-count"]').text()).toBe('15')
    expect(wrapper.get('[data-testid="os-linux-count"]').text()).toBe('9')
    expect(wrapper.find('[data-testid="pending-approval-banner"]').exists()).toBe(true)
    expect(wrapper.text()).toContain('srv-db01')
  })

  it('approves a pending device via POST /api/v1/admin/devices/{id}/approve and updates badge', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input)
      if (url.startsWith('/api/v1/admin/devices/dev-pending-1/approve') && init?.method === 'POST') {
        return new Response(
          JSON.stringify({
            id: 'dev-pending-1',
            hostname: 'ws-buchhaltung-01',
            os_family: 'windows',
            os_name: 'Windows 11 Pro',
            os_version: '24H2',
            arch: 'amd64',
            agent_version: '1.0.0',
            status: 'approved',
            is_online: true,
            reboot_required: false,
            ip_addresses: ['10.0.10.25'],
            logged_in_users: ['mueller'],
            group_ids: [],
            groups: [],
            tags: [],
          }),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        )
      }
      if (url.startsWith('/api/v1/admin/devices')) {
        return new Response(
          JSON.stringify([
            {
              id: 'dev-pending-1',
              hostname: 'ws-buchhaltung-01',
              os_family: 'windows',
              os_name: 'Windows 11 Pro',
              os_version: '24H2',
              arch: 'amd64',
              agent_version: '1.0.0',
              status: 'pending',
              is_online: true,
              reboot_required: true,
              ip_addresses: ['10.0.10.25'],
              logged_in_users: ['mueller'],
              group_ids: [],
              groups: [],
              tags: ['finanzen'],
            },
          ]),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        )
      }
      if (url.startsWith('/api/v1/admin/groups')) {
        return new Response(JSON.stringify([]), {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        })
      }
      return new Response(JSON.stringify({}), { status: 200 })
    })

    const router = createAppRouter(true)
    await router.push('/devices')
    await router.isReady()

    const wrapper = mount(DevicesView, {
      global: { plugins: [router] },
    })
    await flushPromises()

    expect(wrapper.text()).toContain('ws-buchhaltung-01')
    expect(wrapper.text()).toContain('Freigabe ausstehend')
    expect(wrapper.text()).toContain('Neustart erforderlich')

    const approveBtn = wrapper.get('[data-testid="approve-device-dev-pending-1"]')
    await approveBtn.trigger('click')
    await flushPromises()

    const approveCalls = fetchMock.mock.calls.filter(
      ([u, opts]) =>
        String(u) === '/api/v1/admin/devices/dev-pending-1/approve' && opts?.method === 'POST',
    )
    expect(approveCalls.length).toBe(1)
    expect(wrapper.text()).toContain('Freigegeben')
    expect(wrapper.find('[data-testid="approve-device-dev-pending-1"]').exists()).toBe(false)
  })
})
