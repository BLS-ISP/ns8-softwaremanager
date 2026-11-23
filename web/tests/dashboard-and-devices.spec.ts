import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import DashboardView from '@/views/DashboardView.vue'
import DevicesView from '@/views/DevicesView.vue'
import DeviceDetailView from '@/views/DeviceDetailView.vue'
import PoliciesView from '@/views/PoliciesView.vue'
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
    expect(wrapper.find('[data-testid="zero-devices-onboarding"]').exists()).toBe(false)
    expect(wrapper.text()).toContain('srv-db01')
  })

  it('shows first-run client onboarding guide on Dashboard when zero devices are enrolled', async () => {
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
    await router.push('/')
    await router.isReady()

    const wrapper = mount(DashboardView, {
      global: { plugins: [router] },
    })
    await flushPromises()

    expect(wrapper.find('[data-testid="zero-devices-onboarding"]').exists()).toBe(true)
    expect(wrapper.text()).toContain('NS8 Software Center')
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

  it('supports multi-select bulk-action (sync, assign_group, notify) and shows Software Center aktiv badge', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input)
      if (url === '/api/v1/admin/devices/bulk-action' && init?.method === 'POST') {
        return new Response(
          JSON.stringify({ affected: 2, job_ids: ['job-bulk-1', 'job-bulk-2'] }),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        )
      }
      if (url.startsWith('/api/v1/admin/devices')) {
        return new Response(
          JSON.stringify([
            {
              id: 'dev-1',
              hostname: 'ws-office-01',
              os_family: 'windows',
              os_name: 'Windows 11 Pro',
              os_version: '24H2',
              arch: 'amd64',
              agent_version: '1.1.0',
              status: 'approved',
              is_online: true,
              reboot_required: false,
              center_running: true,
              ip_addresses: ['10.0.1.11'],
              logged_in_users: ['anna'],
              group_ids: [],
              groups: [],
              tags: [],
            },
            {
              id: 'dev-2',
              hostname: 'srv-core-02',
              os_family: 'linux',
              os_name: 'Rocky Linux 9',
              os_version: '9.4',
              arch: 'amd64',
              agent_version: '1.1.0',
              status: 'approved',
              is_online: true,
              reboot_required: false,
              center_running: false,
              ip_addresses: ['10.0.1.12'],
              logged_in_users: [],
              group_ids: [],
              groups: [],
              tags: [],
            },
          ]),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        )
      }
      if (url.startsWith('/api/v1/admin/groups')) {
        return new Response(
          JSON.stringify([
            {
              id: 'grp-workstations',
              name: 'Büro-Workstations',
              reboot_policy: 'grace_period',
              grace_period_minutes: 240,
            },
          ]),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        )
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

    // Verify tray badge on dev-1 only
    expect(wrapper.find('[data-testid="center-active-badge-dev-1"]').exists()).toBe(true)
    expect(wrapper.get('[data-testid="center-active-badge-dev-1"]').text()).toContain(
      'Software Center aktiv',
    )
    expect(wrapper.find('[data-testid="center-active-badge-dev-2"]').exists()).toBe(false)

    // Bulk bar hidden initially
    expect(wrapper.find('[data-testid="bulk-actions-bar"]').exists()).toBe(false)

    // Select all devices
    await wrapper.get('[data-testid="select-all-devices"]').trigger('change')
    expect(wrapper.find('[data-testid="bulk-actions-bar"]').exists()).toBe(true)
    expect(wrapper.get('[data-testid="bulk-actions-bar"]').text()).toContain('2 ausgewählt')

    // Trigger bulk sync
    await wrapper.get('[data-testid="bulk-sync-btn"]').trigger('click')
    await flushPromises()

    // Trigger bulk assign_group
    await wrapper.get('[data-testid="bulk-group-select"]').setValue('grp-workstations')
    await wrapper.get('[data-testid="bulk-assign-group-btn"]').trigger('click')
    await flushPromises()

    // Trigger bulk notify via modal
    await wrapper.get('[data-testid="bulk-notify-btn"]').trigger('click')
    expect(wrapper.find('[data-testid="device-notify-modal"]').exists()).toBe(true)
    await wrapper.get('[data-testid="notify-title-input"]').setValue('Wartungsfenster 18:00 Uhr')
    await wrapper
      .get('[data-testid="notify-message-input"]')
      .setValue('Bitte offene Dateien vor Feierabend sichern.')
    await wrapper.get('[data-testid="notify-urgent-checkbox"]').setValue(true)
    await wrapper.get('[data-testid="device-notify-modal"] form').trigger('submit.prevent')
    await flushPromises()

    const bulkCalls = fetchMock.mock.calls.filter(
      ([u, opts]) => String(u) === '/api/v1/admin/devices/bulk-action' && opts?.method === 'POST',
    )
    expect(bulkCalls.length).toBe(3)
    expect(JSON.parse(String(bulkCalls[0][1]?.body))).toEqual({
      device_ids: ['dev-1', 'dev-2'],
      action: 'sync',
    })
    expect(JSON.parse(String(bulkCalls[1][1]?.body))).toEqual({
      device_ids: ['dev-1', 'dev-2'],
      action: 'assign_group',
      group_id: 'grp-workstations',
    })
    expect(JSON.parse(String(bulkCalls[2][1]?.body))).toEqual({
      device_ids: ['dev-1', 'dev-2'],
      action: 'notify',
      title: 'Wartungsfenster 18:00 Uhr',
      message: 'Bitte offene Dateien vor Feierabend sichern.',
      urgent: true,
    })
  })

  it('sends single-device sync, desktop notification, and reboot control commands from DeviceDetailView', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input)
      if (
        (url === '/api/v1/admin/devices/dev-ws-7/sync' ||
          url === '/api/v1/admin/devices/dev-ws-7/notify' ||
          url === '/api/v1/admin/devices/dev-ws-7/reboot') &&
        init?.method === 'POST'
      ) {
        return new Response(
          JSON.stringify({
            id: 'job-ctrl-1',
            device_id: 'dev-ws-7',
            job_type: 'control',
            action: 'execute',
            status: 'queued',
          }),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        )
      }
      if (url === '/api/v1/admin/devices/dev-ws-7') {
        return new Response(
          JSON.stringify({
            id: 'dev-ws-7',
            hostname: 'ws-technik-07',
            os_family: 'windows',
            os_name: 'Windows 11 Enterprise',
            os_version: '24H2',
            arch: 'amd64',
            agent_version: '1.1.0',
            status: 'approved',
            is_online: true,
            reboot_required: true,
            center_running: true,
            ip_addresses: ['10.20.0.7'],
            logged_in_users: ['weber'],
            group_ids: [],
            groups: [],
            tags: [],
            installed_software: [],
            pending_os_updates: [],
            recent_jobs: [],
          }),
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
    await router.push('/devices/dev-ws-7')
    await router.isReady()

    const wrapper = mount(DeviceDetailView, {
      global: { plugins: [router] },
    })
    await flushPromises()

    expect(wrapper.get('[data-testid="device-center-status"]').text()).toContain(
      'Software Center: Aktiv in Benutzersitzung',
    )

    // 1) Immediate Sync
    await wrapper.get('[data-testid="device-sync-btn"]').trigger('click')
    await flushPromises()
    expect(
      fetchMock.mock.calls.some(
        ([u, o]) => String(u) === '/api/v1/admin/devices/dev-ws-7/sync' && o?.method === 'POST',
      ),
    ).toBe(true)

    // 2) Desktop Notification
    await wrapper.get('[data-testid="device-notify-btn"]').trigger('click')
    expect(wrapper.find('[data-testid="device-notify-modal"]').exists()).toBe(true)
    await wrapper.get('[data-testid="notify-title-input"]').setValue('Neustart erforderlich')
    await wrapper
      .get('[data-testid="notify-message-input"]')
      .setValue('Bitte speichern Sie Ihre CAD-Projekte innerhalb der nächsten Stunde.')
    await wrapper.get('[data-testid="notify-urgent-checkbox"]').setValue(true)
    await wrapper.get('[data-testid="device-notify-modal"] form').trigger('submit.prevent')
    await flushPromises()

    const notifyCall = fetchMock.mock.calls.find(
      ([u, o]) => String(u) === '/api/v1/admin/devices/dev-ws-7/notify' && o?.method === 'POST',
    )
    expect(notifyCall).toBeTruthy()
    expect(JSON.parse(String(notifyCall?.[1]?.body))).toEqual({
      title: 'Neustart erforderlich',
      message: 'Bitte speichern Sie Ihre CAD-Projekte innerhalb der nächsten Stunde.',
      urgent: true,
    })

    // 3) Reboot Control (Grace Period 240 min + Immediate warning check + Cancel)
    await wrapper.get('[data-testid="device-reboot-btn"]').trigger('click')
    expect(wrapper.find('[data-testid="device-reboot-modal"]').exists()).toBe(true)

    // Selecting immediate displays safety warning
    await wrapper.get('[data-testid="reboot-mode-immediate"]').setValue('immediate')
    expect(wrapper.find('[data-testid="reboot-immediate-warning"]').exists()).toBe(true)

    // Switch back to grace_period with 240 minutes and submit
    await wrapper.get('[data-testid="reboot-mode-grace_period"]').setValue('grace_period')
    await wrapper.get('[data-testid="reboot-grace-240"]').trigger('click')
    await wrapper.get('[data-testid="device-reboot-modal"] form').trigger('submit.prevent')
    await flushPromises()

    const rebootCalls = fetchMock.mock.calls.filter(
      ([u, o]) => String(u) === '/api/v1/admin/devices/dev-ws-7/reboot' && o?.method === 'POST',
    )
    expect(rebootCalls.length).toBe(1)
    expect(JSON.parse(String(rebootCalls[0][1]?.body))).toEqual({
      mode: 'grace_period',
      grace_minutes: 240,
    })
  })

  it('saves desktop client hardening policies per group in PoliciesView', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input)
      if (url === '/api/v1/admin/groups/grp-kiosk' && init?.method === 'PATCH') {
        const body = JSON.parse(String(init.body))
        return new Response(
          JSON.stringify({
            id: 'grp-kiosk',
            name: 'Empfangs-Terminals',
            ...body,
          }),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        )
      }
      if (url === '/api/v1/admin/groups') {
        return new Response(
          JSON.stringify([
            {
              id: 'grp-kiosk',
              name: 'Empfangs-Terminals',
              description: 'Öffentliche Kiosk-PCs',
              reboot_policy: 'maintenance_window',
              grace_period_minutes: 60,
              enforce_tray: true,
              allow_self_service: true,
              allow_user_os_updates: true,
              silent_notifications: false,
            },
          ]),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        )
      }
      return new Response(JSON.stringify({ server_url: '', version: '1.1.0', artifacts: [] }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    })

    const router = createAppRouter(true)
    await router.push('/policies')
    await router.isReady()

    const wrapper = mount(PoliciesView, {
      global: { plugins: [router] },
    })
    await flushPromises()

    await wrapper.get('[data-testid="policy-allow-self-service-grp-kiosk"]').setValue(false)
    await wrapper.get('[data-testid="policy-allow-os-updates-grp-kiosk"]').setValue(false)
    await wrapper.get('[data-testid="policy-silent-notifications-grp-kiosk"]').setValue(true)

    expect(wrapper.text()).toContain('Managed-Kiosk-Modus')

    await wrapper.get('[data-testid="save-policy-grp-kiosk"]').trigger('click')
    await flushPromises()

    const patchCall = fetchMock.mock.calls.find(
      ([u, o]) => String(u) === '/api/v1/admin/groups/grp-kiosk' && o?.method === 'PATCH',
    )
    expect(patchCall).toBeTruthy()
    expect(JSON.parse(String(patchCall?.[1]?.body))).toEqual({
      reboot_policy: 'maintenance_window',
      grace_period_minutes: 60,
      enforce_tray: true,
      allow_self_service: false,
      allow_user_os_updates: false,
      silent_notifications: true,
    })
  })
})
