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
})
