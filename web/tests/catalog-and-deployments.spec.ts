import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import PackageUploadModal from '@/components/PackageUploadModal.vue'
import DeploymentCreateModal from '@/components/DeploymentCreateModal.vue'
import { saveAuthSession } from '@/api/client'
import type { CatalogItem, Device, DeviceGroup } from '@/api/types'

describe('Software Catalog Upload Wizard & Deployment Mode Selection', () => {
  beforeEach(() => {
    localStorage.clear()
    saveAuthSession('admin-test-token')
    vi.restoreAllMocks()
  })

  it('validates required fields before creating a catalog item and submits valid WinGet package', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          id: 'pkg-7zip',
          name: '7-Zip',
          version: '24.08',
          vendor: 'Igor Pavlov',
          description: '',
          category: 'Produktivität',
          icon_url: '',
          os_family: 'windows',
          source_type: 'winget',
          package_ref: '7zip.7zip',
          install_args: '',
          uninstall_args: '',
          blocked_processes: [],
          allow_uninstall: true,
          return_codes: { '0': 'success', '3010': 'reboot_required', '1641': 'reboot_required' },
          detection_rule: {},
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      ),
    )

    const wrapper = mount(PackageUploadModal, {
      props: { open: true, editItem: null },
    })

    // 1. Trigger submit while empty -> must fail validation without calling fetch
    await wrapper.get('form').trigger('submit.prevent')
    expect(wrapper.get('[data-testid="catalog-validation-error"]').text()).toContain(
      'Anzeigenamen',
    )
    expect(fetchMock).not.toHaveBeenCalled()

    // 2. Fill name + version, leave packageRef blank -> must require package_ref for winget
    await wrapper.get('[data-testid="catalog-name"]').setValue('7-Zip')
    await wrapper.get('[data-testid="catalog-version"]').setValue('24.08')
    await wrapper.get('form').trigger('submit.prevent')
    expect(wrapper.get('[data-testid="catalog-validation-error"]').text()).toContain('Paket-ID')
    expect(fetchMock).not.toHaveBeenCalled()

    // 3. Fill package_ref and submit -> calls POST /api/v1/admin/catalog
    await wrapper.get('[data-testid="catalog-package-ref"]').setValue('7zip.7zip')
    await wrapper.get('form').trigger('submit.prevent')
    await flushPromises()

    expect(fetchMock).toHaveBeenCalledTimes(1)
    const [url, init] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/admin/catalog')
    expect(init?.method).toBe('POST')
    const sentBody = JSON.parse(String(init?.body))
    expect(sentBody.name).toBe('7-Zip')
    expect(sentBody.version).toBe('24.08')
    expect(sentBody.source_type).toBe('winget')
    expect(sentBody.package_ref).toBe('7zip.7zip')
    expect(wrapper.emitted('saved')).toBeTruthy()
  })

  it('validates detection rule path when detection rule is enabled', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch')
    const wrapper = mount(PackageUploadModal, {
      props: { open: true, editItem: null },
    })

    await wrapper.get('[data-testid="catalog-name"]').setValue('Custom ERP Client')
    await wrapper.get('[data-testid="catalog-version"]').setValue('3.2.1')
    await wrapper.get('[data-testid="catalog-source-type"]').setValue('custom')
    await wrapper.get('[data-testid="catalog-package-ref"]').setValue('erp-setup.msi')

    // Enable file detection rule without setting path
    await wrapper.get('[data-testid="detection-rule-type"]').setValue('file')
    await wrapper.get('form').trigger('submit.prevent')

    expect(wrapper.get('[data-testid="catalog-validation-error"]').text()).toContain(
      'Erkennungsregel',
    )
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('allows selecting deployment modes (required, available, uninstall) and validates targets', async () => {
    const sampleCatalog: CatalogItem[] = [
      {
        id: 'pkg-ff',
        name: 'Mozilla Firefox',
        version: '132.0',
        os_family: 'windows',
        source_type: 'winget',
        package_ref: 'Mozilla.Firefox',
        blocked_processes: ['firefox.exe'],
        allow_uninstall: true,
        return_codes: { '0': 'success' },
        detection_rule: {},
      },
    ]
    const sampleGroups: DeviceGroup[] = [
      {
        id: 'grp-workstations',
        name: 'Workstations',
        reboot_policy: 'grace_period',
        grace_period_minutes: 240,
      },
    ]
    const sampleDevices: Device[] = []

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          id: 'dep-1',
          package_id: 'pkg-ff',
          package_name: 'Mozilla Firefox',
          package_version: '132.0',
          os_family: 'windows',
          target_group_ids: ['grp-workstations'],
          target_device_ids: [],
          mode: 'available',
          respect_maintenance_window: true,
          stats: { total: 5, installed: 0, pending: 5, failed: 0 },
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      ),
    )

    const wrapper = mount(DeploymentCreateModal, {
      props: {
        open: true,
        catalog: sampleCatalog,
        groups: sampleGroups,
        devices: sampleDevices,
      },
    })

    // Submitting without any target group/device must fail validation
    await wrapper.get('form').trigger('submit.prevent')
    expect(wrapper.get('[data-testid="deployment-error"]').text()).toContain(
      'mindestens eine Gerätegruppe',
    )
    expect(fetchMock).not.toHaveBeenCalled()

    // Select Self-Service 'available' mode and check target group
    await wrapper.get('[data-testid="deployment-mode-available"]').trigger('click')
    await wrapper.get('[data-testid="deployment-group-grp-workstations"]').setValue(true)

    await wrapper.get('form').trigger('submit.prevent')
    await flushPromises()

    expect(fetchMock).toHaveBeenCalledTimes(1)
    const [url, init] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/admin/deployments')
    expect(init?.method).toBe('POST')
    const payload = JSON.parse(String(init?.body))
    expect(payload).toEqual({
      package_id: 'pkg-ff',
      target_group_ids: ['grp-workstations'],
      target_device_ids: [],
      mode: 'available',
      respect_maintenance_window: true,
    })
    expect(wrapper.emitted('created')).toBeTruthy()
  })

  it('creates macOS Homebrew Cask and custom .dmg packages with guided app_bundle detection rules', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input)
      if (url === '/api/v1/admin/catalog' && init?.method === 'POST') {
        const body = JSON.parse(String(init.body))
        return new Response(
          JSON.stringify({
            id: 'pkg-mac-1',
            ...body,
          }),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        )
      }
      if (url.endsWith('/upload-chunk') && init?.method === 'POST') {
        return new Response(
          JSON.stringify({
            package_id: 'pkg-mac-1',
            chunk_index: 0,
            total_chunks: 1,
            completed: true,
            sha256: 'b'.repeat(64),
            file_size: 1024,
          }),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        )
      }
      return new Response(JSON.stringify({}), { status: 200 })
    })

    // 1) Homebrew Cask package
    const caskWrapper = mount(PackageUploadModal, {
      props: { open: true, editItem: null },
    })

    await caskWrapper.get('[data-testid="catalog-os-family"]').setValue('macos')
    expect(
      (caskWrapper.get('[data-testid="catalog-source-type"]').element as HTMLSelectElement).value,
    ).toBe('brew_cask')

    await caskWrapper.get('[data-testid="catalog-name"]').setValue('Mozilla Firefox')
    await caskWrapper.get('[data-testid="catalog-version"]').setValue('133.0')
    await caskWrapper.get('[data-testid="catalog-package-ref"]').setValue('firefox')
    await caskWrapper.get('form').trigger('submit.prevent')
    await flushPromises()

    const createCalls = fetchMock.mock.calls.filter(
      ([u, o]) => String(u) === '/api/v1/admin/catalog' && o?.method === 'POST',
    )
    expect(createCalls.length).toBe(1)
    const caskBody = JSON.parse(String(createCalls[0][1]?.body))
    expect(caskBody.os_family).toBe('macos')
    expect(caskBody.source_type).toBe('brew_cask')
    expect(caskBody.package_ref).toBe('firefox')
    expect(caskBody.detection_rule).toEqual({
      type: 'brew_cask',
      package: 'firefox',
    })

    // 2) Custom .dmg upload with guided app_bundle detection rule
    const dmgWrapper = mount(PackageUploadModal, {
      props: { open: true, editItem: null },
    })

    await dmgWrapper.get('[data-testid="catalog-os-family"]').setValue('macos')
    await dmgWrapper.get('[data-testid="catalog-source-type"]').setValue('custom')
    await dmgWrapper.get('[data-testid="catalog-name"]').setValue('Slack')
    await dmgWrapper.get('[data-testid="catalog-version"]').setValue('4.41.97')

    const fileInput = dmgWrapper.get('[data-testid="catalog-file-input"]')
    expect(fileInput.attributes('accept')).toBe('.pkg,.dmg,.zip,.sh')

    const dmgFile = new File([new Uint8Array(1024).fill(77)], 'Slack-4.41.97-macOS.dmg', {
      type: 'application/x-apple-diskimage',
    })
    Object.defineProperty(fileInput.element, 'files', {
      value: [dmgFile],
      configurable: true,
    })
    await fileInput.trigger('change')

    expect(
      (dmgWrapper.get('[data-testid="catalog-package-ref"]').element as HTMLInputElement).value,
    ).toBe('Slack-4.41.97-macOS.dmg')

    await dmgWrapper.get('[data-testid="preset-macos-app-bundle"]').trigger('click')
    expect(
      (dmgWrapper.get('[data-testid="detection-rule-path"]').element as HTMLInputElement).value,
    ).toBe('/Applications/Slack.app')
    await dmgWrapper.get('[data-testid="detection-rule-bundle-id"]').setValue('com.tinyspeck.slackmacgap')

    await dmgWrapper.get('form').trigger('submit.prevent')
    await flushPromises()

    const allCreates = fetchMock.mock.calls.filter(
      ([u, o]) => String(u) === '/api/v1/admin/catalog' && o?.method === 'POST',
    )
    expect(allCreates.length).toBe(2)
    const dmgBody = JSON.parse(String(allCreates[1][1]?.body))
    expect(dmgBody.os_family).toBe('macos')
    expect(dmgBody.source_type).toBe('custom')
    expect(dmgBody.package_ref).toBe('Slack-4.41.97-macOS.dmg')
    expect(dmgBody.detection_rule).toEqual({
      type: 'app_bundle',
      path: '/Applications/Slack.app',
      min_version: '4.41.97',
      bundle_id: 'com.tinyspeck.slackmacgap',
    })
    expect(fetchMock.mock.calls.some(([u]) => String(u).endsWith('/upload-chunk'))).toBe(true)
    expect(dmgWrapper.emitted('saved')).toBeTruthy()

    // 3) Custom .pkg with pkgutil receipt detection rule
    const pkgWrapper = mount(PackageUploadModal, {
      props: { open: true, editItem: null },
    })
    await pkgWrapper.get('[data-testid="catalog-os-family"]').setValue('macos')
    await pkgWrapper.get('[data-testid="catalog-source-type"]').setValue('custom')
    await pkgWrapper.get('[data-testid="catalog-name"]').setValue('CorpVPN')
    await pkgWrapper.get('[data-testid="catalog-version"]').setValue('2.5.0')
    await pkgWrapper.get('[data-testid="catalog-package-ref"]').setValue('CorpVPN-2.5.0.pkg')
    await pkgWrapper.get('[data-testid="preset-macos-pkgutil"]').trigger('click')
    await pkgWrapper.get('[data-testid="detection-rule-path"]').setValue('com.corp.vpn.pkg')
    await pkgWrapper.get('form').trigger('submit.prevent')
    await flushPromises()

    const finalCreates = fetchMock.mock.calls.filter(
      ([u, o]) => String(u) === '/api/v1/admin/catalog' && o?.method === 'POST',
    )
    expect(finalCreates.length).toBe(3)
    const pkgBody = JSON.parse(String(finalCreates[2][1]?.body))
    expect(pkgBody.detection_rule).toEqual({
      type: 'pkgutil',
      package_id: 'com.corp.vpn.pkg',
      min_version: '2.5.0',
    })
  })
})
