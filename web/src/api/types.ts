export type AuthSource = 'local' | 'ldap'
export type OsFamily = 'windows' | 'linux'
export type DeviceStatus = 'pending' | 'approved' | 'revoked'
export type RebootPolicy = 'never' | 'maintenance_window' | 'grace_period'
export type SourceType = 'winget' | 'apt' | 'dnf' | 'flatpak' | 'custom'
export type DeploymentMode = 'required' | 'available' | 'uninstall'

export interface AdminUser {
  id: string
  username: string
  role: string
  auth_source: AuthSource
}

export interface LoginPayload {
  username: string
  password: string
  auth_source: AuthSource
}

export interface LoginResponse {
  access_token: string
  token_type: string
  user: AdminUser
}

export interface HealthResponse {
  status: string
  version: string
  database: string
}

export interface OsBreakdown {
  windows: number
  linux: number
}

export interface JobItem {
  id: string
  job_id?: string
  device_id: string
  device_hostname?: string
  hostname?: string
  job_type: 'os_patch' | 'software' | 'self_update' | 'scan_updates' | string
  action: 'install' | 'update' | 'uninstall' | 'scan' | string
  target_id?: string
  target_name?: string
  version?: string
  status: string
  exit_code?: number | null
  reboot_required?: boolean
  stdout?: string | null
  stderr?: string | null
  respect_maintenance_window?: boolean
  created_at?: string
  started_at?: string | null
  completed_at?: string | null
  finished_at?: string | null
}

export interface DashboardSummary {
  total_devices: number
  online_devices: number
  pending_approval_devices: number
  reboot_required_devices: number
  patch_compliance_percent: number
  pending_critical_patches: number
  recent_jobs: JobItem[]
  os_breakdown: OsBreakdown
}

export interface DiskInfo {
  mount?: string
  mount_point?: string
  name?: string
  total_gb?: number
  free_gb?: number
  total_bytes?: number
  free_bytes?: number
}

export interface HardwareInfo {
  cpu_model?: string
  cpu_cores?: number
  ram_total_mb?: number
  ram_free_mb?: number
  disks?: DiskInfo[]
  serial_number?: string
  mac_address?: string
}

export interface InstalledSoftwareItem {
  name: string
  version: string
  publisher?: string | null
  vendor?: string | null
  source?: string | null
  scope?: 'machine' | 'user' | string | null
  package_ref?: string | null
  install_date?: string | null
}

export interface PendingOsUpdateItem {
  update_id: string
  kb_id?: string | null
  title: string
  severity?: string | null
  category?: string | null
  size_bytes?: number | null
  reboot_behavior?: string | null
  excluded?: boolean
  approved?: boolean
}

export interface DeviceGroupSummary {
  id: string
  name: string
}

export interface Device {
  id: string
  hostname: string
  domain?: string | null
  os_family: OsFamily
  os_name: string
  os_version: string
  kernel_or_build?: string | null
  arch: string
  agent_version: string
  machine_uid?: string
  status: DeviceStatus
  is_online: boolean
  reboot_required: boolean
  ip_addresses: string[]
  logged_in_users: string[]
  group_ids: string[]
  groups: DeviceGroupSummary[]
  tags: string[]
  pending_updates_count?: number
  installed_software_count?: number
  last_seen_at?: string | null
  created_at?: string | null
  hardware?: HardwareInfo | null
  installed_software?: InstalledSoftwareItem[]
  pending_os_updates?: PendingOsUpdateItem[]
  recent_jobs?: JobItem[]
}

export interface DeviceFilters {
  status?: string
  group_id?: string
  os_family?: string
  search?: string
}

export interface DeviceUpdatePayload {
  group_ids?: string[]
  tags?: string[]
}

export interface DeviceGroup {
  id: string
  name: string
  description?: string | null
  reboot_policy: RebootPolicy
  grace_period_minutes: number
  device_count?: number
  created_at?: string
}

export interface GroupCreatePayload {
  name: string
  description?: string
  reboot_policy: RebootPolicy
  grace_period_minutes: number
}

export interface MaintenanceWindow {
  id: string
  group_id: string
  group_name?: string
  name: string
  days_of_week: number[]
  start_time: string
  end_time: string
  timezone: string
  catch_up: boolean
  created_at?: string
}

export interface MaintenanceWindowCreatePayload {
  group_id: string
  name: string
  days_of_week: number[]
  start_time: string
  end_time: string
  timezone: string
  catch_up: boolean
}

export interface OsPatch {
  update_id: string
  title: string
  os_family: OsFamily
  severity: string
  category?: string | null
  size_bytes?: number | null
  affected_device_count: number
  affected_device_ids?: string[]
  approved: boolean
  excluded: boolean
  last_seen_at?: string | null
}

export interface PatchApprovePayload {
  update_ids: string[]
  group_ids: string[]
  respect_maintenance_window: boolean
}

export interface PatchApproveResponse {
  approved_update_ids: string[]
  jobs_created: number
  job_ids: string[]
}

export interface PatchExcludePayload {
  update_ids: string[]
  excluded: boolean
}

export interface DetectionRule {
  type?: 'none' | 'registry' | 'file' | 'package' | string
  path?: string
  key_path?: string
  value_name?: string
  package_name?: string
  operator?: 'gte' | 'eq' | 'exists' | string
  version?: string
  expected_version?: string
}

export interface CatalogItem {
  id: string
  name: string
  version: string
  vendor?: string | null
  description?: string | null
  category?: string | null
  icon_url?: string | null
  os_family: OsFamily
  source_type: SourceType
  package_ref: string
  install_args?: string | null
  uninstall_args?: string | null
  blocked_processes: string[]
  allow_uninstall: boolean
  return_codes: Record<string, string>
  detection_rule: DetectionRule | Record<string, unknown>
  filename?: string | null
  file_size?: number | null
  sha256?: string | null
  created_at?: string
  updated_at?: string
}

export interface CatalogCreatePayload {
  name: string
  version: string
  vendor?: string
  description?: string
  category?: string
  icon_url?: string
  os_family: OsFamily
  source_type: SourceType
  package_ref: string
  install_args?: string
  uninstall_args?: string
  blocked_processes: string[]
  allow_uninstall: boolean
  return_codes: Record<string, string>
  detection_rule: DetectionRule | Record<string, unknown>
}

export interface ChunkUploadResponse {
  package_id: string
  chunk_index: number
  total_chunks: number
  completed: boolean
  sha256?: string | null
  file_size?: number | null
  catalog_item?: CatalogItem | null
}

export interface DeploymentStats {
  total: number
  installed: number
  pending: number
  failed: number
}

export interface Deployment {
  id: string
  package_id: string
  package_name?: string
  package_version?: string
  os_family?: OsFamily
  target_group_ids: string[]
  target_device_ids: string[]
  mode: DeploymentMode
  respect_maintenance_window: boolean
  stats?: DeploymentStats
  installed_count?: number
  pending_count?: number
  failed_count?: number
  total_targets?: number
  created_at?: string
}

export interface DeploymentCreatePayload {
  package_id: string
  target_group_ids: string[]
  target_device_ids: string[]
  mode: DeploymentMode
  respect_maintenance_window: boolean
}

export interface EnrollmentToken {
  id: string
  name: string
  token: string
  expires_at?: string | null
  max_uses?: number | null
  uses_count: number
  auto_approve: boolean
  group_id?: string | null
  revoked: boolean
  created_at?: string
  install_ps1_cmd: string
  install_sh_cmd: string
}

export interface EnrollmentTokenCreatePayload {
  name: string
  expires_in_hours: number
  max_uses: number | null
  auto_approve: boolean
  group_id: string | null
}

export interface SelfUpdatePayload {
  version: string
  os_family?: OsFamily | null
  sha256: string
  download_url?: string | null
  target_group_ids: string[]
  target_device_ids: string[]
  respect_maintenance_window: boolean
}

export interface SelfUpdateResponse {
  jobs_created: number
  job_ids: string[]
}
