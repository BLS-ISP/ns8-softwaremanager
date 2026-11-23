import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import App from '../src/App.vue';
import StoreView from '../src/views/StoreView.vue';
import UpdatesView from '../src/views/UpdatesView.vue';
import RebootModal from '../src/components/RebootModal.vue';
import type { CatalogViewItem, PendingOsUpdate, RebootState } from '../src/api';

describe('NS8 Software Center UI', () => {
  const sampleCatalog: CatalogViewItem[] = [
    {
      package_id: 'pkg-firefox',
      name: 'Mozilla Firefox',
      vendor: 'Mozilla',
      version: '132.0',
      description: 'Schneller Webbrowser',
      category: 'Browser',
      icon_url: '',
      os_family: 'windows',
      source_type: 'winget',
      allow_uninstall: true,
      installed_version: '132.0',
      status: 'installed',
      blocked_processes: ['firefox.exe'],
      last_message: null,
    },
    {
      package_id: 'pkg-edr',
      name: 'Corporate Security Agent',
      vendor: 'SecOps',
      version: '5.1.0',
      description: 'Pflichtnahes Sicherheitspaket',
      category: 'Sicherheit',
      icon_url: '',
      os_family: 'windows',
      source_type: 'custom',
      allow_uninstall: false,
      installed_version: '5.1.0',
      status: 'installed',
      blocked_processes: [],
      last_message: null,
    },
    {
      package_id: 'pkg-7zip',
      name: '7-Zip',
      vendor: 'Igor Pavlov',
      version: '24.08',
      description: 'Datei-Archivierer',
      category: 'Werkzeuge',
      icon_url: '',
      os_family: 'windows',
      source_type: 'winget',
      allow_uninstall: true,
      installed_version: null,
      status: 'available',
      blocked_processes: [],
      last_message: null,
    },
  ];

  const storageBacking = new Map<string, string>();
  const memoryStorage: Storage = {
    get length() {
      return storageBacking.size;
    },
    clear() {
      storageBacking.clear();
    },
    getItem(key: string) {
      return storageBacking.has(key) ? storageBacking.get(key)! : null;
    },
    key(index: number) {
      return Array.from(storageBacking.keys())[index] ?? null;
    },
    removeItem(key: string) {
      storageBacking.delete(key);
    },
    setItem(key: string, value: string) {
      storageBacking.set(key, String(value));
    },
  };

  beforeEach(() => {
    storageBacking.clear();
    Object.defineProperty(window, 'localStorage', {
      value: memoryStorage,
      configurable: true,
      writable: true,
    });
  });

  it('hides Deinstallieren when allow_uninstall is false and triggers install on click', async () => {
    const wrapper = mount(StoreView, {
      props: { items: sampleCatalog, busyId: null },
    });

    // Uninstall button present for Firefox (allow_uninstall: true), absent for Security Agent (allow_uninstall: false)
    expect(wrapper.find('[data-testid="uninstall-btn-pkg-firefox"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="uninstall-btn-pkg-edr"]').exists()).toBe(false);

    // Generated category/monogram avatars render when icon_url is empty
    const ffAvatar = wrapper.find('[data-testid="app-avatar-pkg-firefox"]');
    expect(ffAvatar.exists()).toBe(true);
    expect(ffAvatar.text()).toBe('MF');
    expect(ffAvatar.classes()).toContain('avatar-browser');

    // Clicking Installieren on 7-Zip emits trigger('pkg-7zip', 'install')
    const installBtn = wrapper.find('[data-testid="install-btn-pkg-7zip"]');
    expect(installBtn.exists()).toBe(true);
    await installBtn.trigger('click');
    expect(wrapper.emitted('trigger')?.[0]).toEqual(['pkg-7zip', 'install']);

    // Category chip filters down cards
    await wrapper.find('[data-testid="category-chip-Sicherheit"]').trigger('click');
    expect(wrapper.find('[data-testid="app-card-pkg-edr"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="app-card-pkg-firefox"]').exists()).toBe(false);
    await wrapper.find('[data-testid="category-chip-Alle"]').trigger('click');

    // Search filter narrows down cards
    const search = wrapper.find('[data-testid="catalog-search"]');
    await search.setValue('Firefox');
    expect(wrapper.find('[data-testid="app-card-pkg-firefox"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="app-card-pkg-7zip"]').exists()).toBe(false);
  });

  it('requires confirmation before emitting uninstall and supports canceling', async () => {
    const wrapper = mount(StoreView, {
      props: { items: sampleCatalog, busyId: null },
    });

    await wrapper.find('[data-testid="uninstall-btn-pkg-firefox"]').trigger('click');
    expect(wrapper.find('[data-testid="confirm-uninstall-modal"]').exists()).toBe(true);
    expect(wrapper.emitted('trigger')).toBeUndefined();

    // Cancel keeps app untouched
    await wrapper.find('[data-testid="cancel-uninstall-btn"]').trigger('click');
    expect(wrapper.find('[data-testid="confirm-uninstall-modal"]').exists()).toBe(false);
    expect(wrapper.emitted('trigger')).toBeUndefined();

    // Confirm emits uninstall
    await wrapper.find('[data-testid="uninstall-btn-pkg-firefox"]').trigger('click');
    await wrapper.find('[data-testid="confirm-uninstall-btn"]').trigger('click');
    expect(wrapper.emitted('trigger')?.[0]).toEqual(['pkg-firefox', 'uninstall']);
  });

  it('renders status pills and active progress indicators cleanly', () => {
    const statesCatalog: CatalogViewItem[] = [
      { ...sampleCatalog[0], package_id: 'p-inst', status: 'installed' },
      { ...sampleCatalog[0], package_id: 'p-upd', status: 'update_available' },
      { ...sampleCatalog[0], package_id: 'p-busy', status: 'installing' },
      {
        ...sampleCatalog[0],
        package_id: 'p-wait',
        status: 'waiting_for_process',
        blocked_processes: ['firefox.exe'],
      },
    ];

    const wrapper = mount(StoreView, {
      props: { items: statesCatalog, busyId: null },
    });

    expect(wrapper.find('[data-testid="status-pill-p-inst"]').text()).toBe('Installiert');
    expect(wrapper.find('[data-testid="status-pill-p-upd"]').text()).toBe('Update verfügbar');
    expect(wrapper.find('[data-testid="status-pill-p-busy"]').text()).toBe('Wird installiert…');
    expect(wrapper.find('[data-testid="status-pill-p-wait"]').text()).toBe(
      'Wartet auf Schließen der App'
    );
    expect(wrapper.find('[data-testid="progress-bar-p-busy"]').exists()).toBe(true);
    expect(wrapper.text()).toContain('firefox.exe');
  });

  it('renders pending OS updates and emits trigger-all on Jetzt installieren', async () => {
    const osUpdates: PendingOsUpdate[] = [
      {
        update_id: 'KB5044285',
        title: '2024-10 Kumulatives Sicherheitsupdate',
        severity: 'critical',
        category: 'security',
        size_bytes: 524288000,
        reboot_behavior: 'always_requires_reboot',
      },
    ];

    const wrapper = mount(UpdatesView, {
      props: { osUpdates, appUpdates: [], busy: false },
    });

    expect(wrapper.text()).toContain('KB5044285');
    await wrapper.find('[data-testid="install-all-updates-btn"]').trigger('click');
    expect(wrapper.emitted('trigger-all')).toBeTruthy();
  });

  it('shows grace-period countdown and disables snooze after deadline expires', async () => {
    const rbBefore: RebootState = {
      reboot_required: true,
      policy: 'grace_period',
      grace_period_minutes: 120,
      detected_at_unix: 1000,
      deadline_unix: 4600,
      snoozed_until_unix: null,
      triggered_by_job_id: 'job-1',
    };

    const wrapper = mount(RebootModal, {
      props: {
        visible: true,
        rebootState: rbBefore,
        nowUnixOverride: 1000,
      },
    });

    expect(wrapper.find('[data-testid="countdown-value"]').text()).toBe('01:00:00');
    const snooze1h = wrapper.find('[data-testid="snooze-1h-btn"]');
    expect((snooze1h.element as HTMLButtonElement).disabled).toBe(false);
    await snooze1h.trigger('click');
    expect(wrapper.emitted('snooze')?.[0]).toEqual([60]);

    // Advance past deadline -> snooze buttons become disabled
    await wrapper.setProps({ nowUnixOverride: 5000 });
    expect((wrapper.find('[data-testid="snooze-1h-btn"]').element as HTMLButtonElement).disabled).toBe(true);
    expect((wrapper.find('[data-testid="snooze-4h-btn"]').element as HTMLButtonElement).disabled).toBe(true);

    await wrapper.find('[data-testid="confirm-reboot-btn"]').trigger('click');
    expect(wrapper.emitted('confirm')).toBeTruthy();
  });

  it('renders custom brand SVG logo and persists Dark/Light theme toggle in localStorage', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        json: async () => ({
          ok: true,
          data: {
            items: [],
            device_id: 'dev-local',
            server_url: 'https://swm.local',
            agent_version: '1.0.0',
            connected: true,
            device_status: 'approved',
            pending_os_updates: [],
            active_jobs: [],
            recent_history: [],
            reboot_state: {
              reboot_required: false,
              policy: 'never',
              grace_period_minutes: 240,
              detected_at_unix: null,
              deadline_unix: null,
              snoozed_until_unix: null,
              triggered_by_job_id: null,
            },
          },
        }),
      })
    );

    const wrapper = mount(App);
    expect(wrapper.find('[data-testid="brand-logo-svg"]').exists()).toBe(true);
    expect(wrapper.attributes('data-theme')).toBe('light');

    await wrapper.find('[data-testid="theme-toggle-btn"]').trigger('click');
    expect(wrapper.attributes('data-theme')).toBe('dark');
    expect(window.localStorage.getItem('swm-center-theme')).toBe('dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');

    wrapper.unmount();
    vi.unstubAllGlobals();
  });

  it('enforces IT managed-mode banners and disables self-service & OS update buttons when disallowed', () => {
    const storeManaged = mount(StoreView, {
      props: { items: sampleCatalog, busyId: null, allowSelfService: false },
    });
    expect(storeManaged.find('[data-testid="self-service-policy-banner"]').exists()).toBe(true);
    expect(storeManaged.text()).toContain(
      'Durch IT-Richtlinie verwalteter Modus — Installationen werden zentral gesteuert'
    );
    expect(
      (storeManaged.find('[data-testid="install-btn-pkg-7zip"]').element as HTMLButtonElement)
        .disabled
    ).toBe(true);
    expect(
      (storeManaged.find('[data-testid="uninstall-btn-pkg-firefox"]').element as HTMLButtonElement)
        .disabled
    ).toBe(true);

    const updatesManaged = mount(UpdatesView, {
      props: {
        osUpdates: [
          {
            update_id: 'KB5044285',
            title: 'Kumulatives Sicherheitsupdate',
            severity: 'critical',
            category: 'security',
            size_bytes: 102400,
            reboot_behavior: 'always_requires_reboot',
          },
        ],
        appUpdates: [],
        busy: false,
        allowUserOsUpdates: false,
      },
    });
    expect(updatesManaged.find('[data-testid="os-updates-policy-banner"]').exists()).toBe(true);
    expect(
      (updatesManaged.find('[data-testid="install-all-updates-btn"]').element as HTMLButtonElement)
        .disabled
    ).toBe(true);
  });

  it('renders dismissible admin message banners and System & Diagnose tab with sync and clipboard copy', async () => {
    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: writeTextMock },
      configurable: true,
    });

    const fetchMock = vi.fn().mockImplementation(async (url: string) => {
      if (url === '/api/center/catalog') {
        return {
          ok: true,
          status: 200,
          json: async () => ({ ok: true, data: { kind: 'catalog', items: sampleCatalog } }),
        };
      }
      if (url === '/api/messages/dismiss') {
        return {
          ok: true,
          status: 200,
          json: async () => ({
            ok: true,
            data: {
              device_id: 'dev-wks-07',
              hostname: 'WKS-BUCHHALTUNG-07',
              server_url: 'https://swm.ns8.example',
              agent_version: '1.0.0',
              connected: true,
              device_status: 'approved',
              pending_os_updates: [],
              active_jobs: [],
              recent_history: [],
              reboot_state: {
                reboot_required: false,
                policy: 'grace_period',
                grace_period_minutes: 120,
                detected_at_unix: null,
                deadline_unix: null,
                snoozed_until_unix: null,
                triggered_by_job_id: null,
              },
              desktop_policy: {
                enforce_tray: true,
                allow_self_service: false,
                allow_user_os_updates: true,
                silent_notifications: false,
              },
              admin_messages: [],
            },
          }),
        };
      }
      if (url === '/api/sync') {
        return {
          ok: true,
          status: 200,
          json: async () => ({
            ok: true,
            data: {
              job_id: 'sync-now',
              status: 'running',
              message: 'Synchronisation mit NS8-Server gestartet.',
            },
          }),
        };
      }
      return {
        ok: true,
        status: 200,
        json: async () => ({
          ok: true,
          data: {
            device_id: 'dev-wks-07',
            hostname: 'WKS-BUCHHALTUNG-07',
            server_url: 'https://swm.ns8.example',
            agent_version: '1.0.0',
            connected: true,
            device_status: 'approved',
            pending_os_updates: [],
            active_jobs: [],
            recent_history: [],
            reboot_state: {
              reboot_required: false,
              policy: 'grace_period',
              grace_period_minutes: 120,
              detected_at_unix: null,
              deadline_unix: null,
              snoozed_until_unix: null,
              triggered_by_job_id: null,
            },
            desktop_policy: {
              enforce_tray: true,
              allow_self_service: false,
              allow_user_os_updates: true,
              silent_notifications: false,
            },
            admin_messages: [
              {
                id: 'msg-99',
                title: 'Geplante Serverwartung',
                message: 'Bitte speichern Sie alle Dokumente bis 19:00 Uhr.',
                urgent: true,
                created_at: '2026-11-23T18:00:00Z',
              },
            ],
          },
        }),
      };
    });

    vi.stubGlobal('fetch', fetchMock);
    const wrapper = mount(App);
    await new Promise((r) => setTimeout(r, 20));
    await wrapper.vm.$nextTick();

    // 1. Admin message banner is visible and dismissible via POST /api/messages/dismiss
    const banner = wrapper.find('[data-testid="admin-message-banner"]');
    expect(banner.exists()).toBe(true);
    expect(banner.text()).toContain('Geplante Serverwartung');
    await wrapper.find('[data-testid="dismiss-message-msg-99"]').trigger('click');
    await new Promise((r) => setTimeout(r, 15));
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-testid="admin-message-banner"]').exists()).toBe(false);

    // 2. Navigate to 4th tab "System & Diagnose"
    const navDiag = wrapper.find('[data-testid="nav-diagnostics"]');
    expect(navDiag.exists()).toBe(true);
    await navDiag.trigger('click');

    expect(wrapper.find('[data-testid="diag-hostname"]').text()).toBe('WKS-BUCHHALTUNG-07');
    expect(wrapper.find('[data-testid="diag-device-id"]').text()).toBe('dev-wks-07');
    expect(wrapper.find('[data-testid="diag-server-url"]').text()).toBe('https://swm.ns8.example');
    expect(wrapper.find('[data-testid="policy-enforce-tray"]').text()).toContain('Erzwungen');
    expect(wrapper.find('[data-testid="policy-self-service"]').text()).toContain('Zentral verwaltet');

    // 3. Copy diagnostics report to clipboard
    await wrapper.find('[data-testid="diagnostics-copy-btn"]').trigger('click');
    expect(writeTextMock).toHaveBeenCalledTimes(1);
    expect(writeTextMock.mock.calls[0][0]).toContain('WKS-BUCHHALTUNG-07');

    // 4. Trigger manual server sync via POST /api/sync
    await wrapper.find('[data-testid="diagnostics-sync-btn"]').trigger('click');
    await new Promise((r) => setTimeout(r, 15));
    expect(fetchMock.mock.calls.some((c) => c[0] === '/api/sync')).toBe(true);

    wrapper.unmount();
    vi.unstubAllGlobals();
  });
});
