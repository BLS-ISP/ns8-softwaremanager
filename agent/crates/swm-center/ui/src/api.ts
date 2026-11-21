export interface CatalogViewItem {
  package_id: string;
  name: string;
  vendor: string;
  version: string;
  description: string;
  category: string;
  icon_url: string;
  os_family: string;
  source_type: string;
  allow_uninstall: boolean;
  installed_version: string | null;
  status: string;
  blocked_processes: string[];
  last_message: string | null;
}

export interface PendingOsUpdate {
  update_id: string;
  title: string;
  severity: string;
  category: string;
  size_bytes: number;
  reboot_behavior: string;
}

export interface JobActivityEntry {
  job_id: string;
  job_type: string;
  action: string;
  target_name: string;
  version: string;
  status: string;
  exit_code: number | null;
  reboot_required: boolean;
  timestamp: string;
  message: string;
}

export interface RebootState {
  reboot_required: boolean;
  policy: 'never' | 'maintenance_window' | 'grace_period';
  grace_period_minutes: number;
  detected_at_unix: number | null;
  deadline_unix: number | null;
  snoozed_until_unix: number | null;
  triggered_by_job_id: string | null;
}

export interface CenterStatusSnapshot {
  device_id: string;
  server_url: string;
  agent_version: string;
  connected: boolean;
  device_status: string;
  pending_os_updates: PendingOsUpdate[];
  active_jobs: JobActivityEntry[];
  recent_history: JobActivityEntry[];
  reboot_state: RebootState;
}

interface IpcEnvelope<T> {
  ok: boolean;
  error_code?: string;
  message?: string;
  data?: T;
}

async function callBridge<T>(url: string, init?: RequestInit): Promise<T> {
  const resp = await fetch(url, {
    headers: { 'Content-Type': 'application/json' },
    ...init,
  });
  const body = (await resp.json()) as IpcEnvelope<T>;
  if (!body.ok || !resp.ok) {
    throw new Error(body.message || `IPC-Fehler (${resp.status})`);
  }
  return body.data as T;
}

export async function fetchCatalog(): Promise<CatalogViewItem[]> {
  const data = await callBridge<{ kind: 'catalog'; items: CatalogViewItem[] }>(
    '/api/center/catalog'
  );
  return data?.items ?? [];
}

export async function fetchCenterStatus(): Promise<CenterStatusSnapshot> {
  return callBridge<CenterStatusSnapshot>('/api/center/status');
}

export async function triggerCatalogAction(
  packageId: string,
  action: 'install' | 'update' | 'uninstall'
): Promise<{ job_id: string; status: string; message: string }> {
  return callBridge('/api/center/trigger', {
    method: 'POST',
    body: JSON.stringify({ package_id: packageId, action }),
  });
}

export async function triggerPendingUpdates(): Promise<{
  job_id: string;
  status: string;
  message: string;
}> {
  return callBridge('/api/center/trigger-updates', { method: 'POST' });
}

export async function snoozeReboot(minutes: number): Promise<RebootState> {
  return callBridge<RebootState>('/api/center/snooze-reboot', {
    method: 'POST',
    body: JSON.stringify({ minutes }),
  });
}

export async function confirmRebootNow(): Promise<RebootState> {
  return callBridge<RebootState>('/api/center/confirm-reboot', { method: 'POST' });
}
