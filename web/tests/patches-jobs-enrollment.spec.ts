import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import LoginView from '@/views/LoginView.vue'
import PatchManagementView from '@/views/PatchManagementView.vue'
import EnrollmentView from '@/views/EnrollmentView.vue'
import JobLogModal from '@/components/JobLogModal.vue'
import { createAppRouter } from '@/router'
import { clearAuthSession, saveAuthSession } from '@/api/client'

describe('Login, OS Patching, Job Log Viewer & Enrollment Views', () => {
  beforeEach(() => {
    localStorage.clear()
    vi.restoreAllMocks()
  })

  it('authenticates via NS8 LDAP source on LoginView and redirects to target route', async () => {
    clearAuthSession()
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input)
      if (url === '/api/v1/admin/auth/login') {
        return new Response(
          JSON.stringify({
            access_token: 'ldap-session-token',
            token_type: 'bearer',
            user: { id: 'u-ldap', username: 'sysadmin', role: 'admin', auth_source: 'ldap' },
          }),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        )
      }
      return new Response(JSON.stringify([]), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    })

    const router = createAppRouter(true)
    await router.push('/login?redirect=/catalog')
    await router.isReady()

    const wrapper = mount(LoginView, {
      global: { plugins: [router] },
    })

    await wrapper.get('[data-testid="auth-source-ldap"]').trigger('click')
    await wrapper.get('[data-testid="login-username"]').setValue('sysadmin')
    await wrapper.get('[data-testid="login-password"]').setValue('S3curePass!')
    await wrapper.get('form').trigger('submit.prevent')
    await flushPromises()

    const loginCall = fetchMock.mock.calls.find(([u]) => String(u) === '/api/v1/admin/auth/login')
    expect(loginCall).toBeTruthy()
    expect(JSON.parse(String(loginCall?.[1]?.body))).toEqual({
      username: 'sysadmin',
      password: 'S3curePass!',
      auth_source: 'ldap',
    })
    expect(router.currentRoute.value.path).toBe('/catalog')
  })

  it('excludes a faulty Windows KB update in PatchManagementView', async () => {
    saveAuthSession('admin-token')
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input)
      if (url === '/api/v1/admin/patches/exclude' && init?.method === 'POST') {
        return new Response(JSON.stringify({ update_ids: ['KB5034441'], excluded: true }), {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        })
      }
      if (url.startsWith('/api/v1/admin/patches')) {
        return new Response(
          JSON.stringify([
            {
              update_id: 'KB5034441',
              title: 'Windows Recovery Environment Security Update',
              os_family: 'windows',
              severity: 'critical',
              category: 'security',
              size_bytes: 262144000,
              affected_device_count: 5,
              approved: false,
              excluded: false,
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

    const wrapper = mount(PatchManagementView)
    await flushPromises()

    expect(wrapper.text()).toContain('KB5034441')
    expect(wrapper.text()).toContain('Nur macOS (softwareupdate)')
    const excludeBtn = wrapper.findAll('button').find((b) => b.text().includes('Ausschließen'))
    expect(excludeBtn).toBeTruthy()
    await excludeBtn!.trigger('click')
    await flushPromises()

    const excludeCall = fetchMock.mock.calls.find(
      ([u, opts]) => String(u) === '/api/v1/admin/patches/exclude' && opts?.method === 'POST',
    )
    expect(excludeCall).toBeTruthy()
    expect(JSON.parse(String(excludeCall?.[1]?.body))).toEqual({
      update_ids: ['KB5034441'],
      excluded: true,
    })
    expect(wrapper.text()).toContain('Gesperrt (Ausgeschlossen)')
  })

  it('renders stdout, stderr, exit code 3010, and reboot badge in JobLogModal', async () => {
    saveAuthSession('admin-token')
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          id: 'job-msi-3010',
          device_id: 'dev-1',
          device_hostname: 'ws-vertrieb-02',
          job_type: 'software',
          action: 'install',
          target_id: 'pkg-office',
          version: '24.2',
          status: 'succeeded',
          exit_code: 3010,
          reboot_required: true,
          stdout: 'MSI installation completed cleanly (3010).',
          stderr: 'WARN: Pending file rename scheduled on reboot.',
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      ),
    )

    const wrapper = mount(JobLogModal, {
      props: { open: true, jobId: 'job-msi-3010' },
    })
    await flushPromises()

    expect(wrapper.get('[data-testid="job-stdout"]').text()).toContain(
      'MSI installation completed cleanly (3010).',
    )
    expect(wrapper.get('[data-testid="job-stderr"]').text()).toContain(
      'WARN: Pending file rename scheduled on reboot.',
    )
    expect(wrapper.text()).toContain('3010')
    expect(wrapper.text()).toContain('Neustart erforderlich')
  })

  it('displays enrollment tokens and previews headless=1 toggle on EnrollmentView', async () => {
    saveAuthSession('admin-token')
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input)
      if (url === '/api/v1/admin/enrollment-tokens') {
        return new Response(
          JSON.stringify([
            {
              id: 'tok-1',
              name: 'Linux Server Fleet',
              token: 'swm_tok_abc',
              expires_at: '2026-12-01T00:00:00Z',
              max_uses: 25,
              uses_count: 3,
              auto_approve: false,
              group_id: null,
              revoked: false,
              install_ps1_cmd: "irm 'https://swm.lan/dl/install.ps1?token=swm_tok_abc&headless=0' | iex",
              install_sh_cmd: "curl -fsSL 'https://swm.lan/dl/install.sh?token=swm_tok_abc&headless=0' | sudo bash",
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

    const wrapper = mount(EnrollmentView)
    await flushPromises()

    expect(wrapper.text()).toContain('Linux Server Fleet')
    expect(wrapper.text()).toContain('headless=0')

    const headlessBtn = wrapper.findAll('button').find((b) => b.text().includes('Headless-Server'))
    await headlessBtn!.trigger('click')
    expect(wrapper.text()).toContain('headless=1')

    // Verify GPO & Ansible mass-rollout tabs render idempotent scripts
    await wrapper.get('[data-testid="tab-gpo-tok-1"]').trigger('click')
    expect(wrapper.text()).toContain('install-swm-gpo.ps1')
    expect(wrapper.text()).toContain('skip_if_enrolled=1')

    await wrapper.get('[data-testid="tab-ansible-tok-1"]').trigger('click')
    expect(wrapper.text()).toContain('deploy-swm-agent.yml')
    expect(wrapper.text()).toContain('/etc/ns8-swm/config.json')

    // Verify 5th rollout tab: macOS MDM (Jamf / Intune) idempotent pure-LF script
    await wrapper.get('[data-testid="rollout-tab-mdm"]').trigger('click')
    const mdmScript = wrapper.get('[data-testid="mdm-macos-script-preview"]').text()
    expect(wrapper.text()).toContain('install-swm-mdm-macos.sh')
    expect(mdmScript).toContain('/Library/Application Support/NS8-SoftwareManager/config.json')
    expect(mdmScript).toContain('launchctl print system/org.nethserver.swm-agent')
    expect(mdmScript).toContain(
      "https://swm.lan/dl/install.sh?token=swm_tok_abc&headless=1&skip_if_enrolled=1",
    )
    expect(mdmScript.includes('\r\n')).toBe(false)

    // Verify manual tab includes macOS Universal CLI commands
    await wrapper.get('[data-testid="tab-manual-tok-1"]').trigger('click')
    expect(wrapper.text()).toContain('macOS Universal CLI (swm-agent)')
    expect(wrapper.text()).toContain('/Library/Application Support/NS8-SoftwareManager')
  })
})
