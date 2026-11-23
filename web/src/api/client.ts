import type {
  AdminUser,
  AgentArtifactsResponse,
  BulkDeviceActionPayload,
  BulkDeviceActionResponse,
  CatalogCreatePayload,
  CatalogItem,
  ChunkUploadResponse,
  DashboardSummary,
  Deployment,
  DeploymentCreatePayload,
  Device,
  DeviceFilters,
  DeviceGroup,
  DeviceNotifyPayload,
  DeviceRebootPayload,
  DeviceUpdatePayload,
  EnrollmentToken,
  EnrollmentTokenCreatePayload,
  GroupCreatePayload,
  HealthResponse,
  JobItem,
  LoginPayload,
  LoginResponse,
  MaintenanceWindow,
  MaintenanceWindowCreatePayload,
  OsPatch,
  PatchApprovePayload,
  PatchApproveResponse,
  PatchExcludePayload,
  SelfUpdatePayload,
  SelfUpdateResponse,
} from './types'

export const TOKEN_STORAGE_KEY = 'swm_access_token'
export const USER_STORAGE_KEY = 'swm_admin_user'

export class ApiError extends Error {
  status: number
  detail: unknown

  constructor(message: string, status: number, detail?: unknown) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.detail = detail
  }
}

const fallbackMemoryStore = new Map<string, string>()

function getSafeStorage(): Pick<Storage, 'getItem' | 'setItem' | 'removeItem'> {
  try {
    const ls = typeof window !== 'undefined' ? window.localStorage : globalThis.localStorage
    if (ls && typeof ls.getItem === 'function' && typeof ls.setItem === 'function') {
      return ls
    }
  } catch {
    // ignore Node experimental localStorage warning
  }
  return {
    getItem: (k: string) => (fallbackMemoryStore.has(k) ? (fallbackMemoryStore.get(k) ?? null) : null),
    setItem: (k: string, v: string) => {
      fallbackMemoryStore.set(k, String(v))
    },
    removeItem: (k: string) => {
      fallbackMemoryStore.delete(k)
    },
  }
}

export function getStoredToken(): string | null {
  try {
    return getSafeStorage().getItem(TOKEN_STORAGE_KEY)
  } catch {
    return null
  }
}

export function getStoredUser(): AdminUser | null {
  try {
    const raw = getSafeStorage().getItem(USER_STORAGE_KEY)
    return raw ? (JSON.parse(raw) as AdminUser) : null
  } catch {
    return null
  }
}

export function saveAuthSession(token: string, user?: AdminUser | null): void {
  const storage = getSafeStorage()
  storage.setItem(TOKEN_STORAGE_KEY, token)
  if (user) {
    storage.setItem(USER_STORAGE_KEY, JSON.stringify(user))
  }
}

export function clearAuthSession(): void {
  const storage = getSafeStorage()
  storage.removeItem(TOKEN_STORAGE_KEY)
  storage.removeItem(USER_STORAGE_KEY)
}

function buildQuery(params?: Record<string, unknown>): string {
  if (!params) return ''
  const searchParams = new URLSearchParams()
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== '') {
      searchParams.set(key, String(value))
    }
  }
  const str = searchParams.toString()
  return str ? `?${str}` : ''
}

function unwrapList<T>(payload: unknown, candidateKeys: string[] = []): T[] {
  if (Array.isArray(payload)) {
    return payload as T[]
  }
  if (payload && typeof payload === 'object') {
    const obj = payload as Record<string, unknown>
    for (const key of ['items', ...candidateKeys]) {
      if (Array.isArray(obj[key])) {
        return obj[key] as T[]
      }
    }
  }
  return []
}

export async function apiRequest<T>(
  path: string,
  options: RequestInit & { skipAuth?: boolean } = {},
): Promise<T> {
  const { skipAuth = false, headers: customHeaders, ...rest } = options
  const headers = new Headers(customHeaders || {})

  if (!skipAuth) {
    const token = getStoredToken()
    if (token) {
      headers.set('Authorization', `Bearer ${token}`)
    }
  }

  const isFormData = typeof FormData !== 'undefined' && rest.body instanceof FormData
  if (!isFormData && rest.body !== undefined && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json')
  }
  if (!headers.has('Accept')) {
    headers.set('Accept', 'application/json')
  }

  const response = await fetch(path, {
    ...rest,
    headers,
  })

  if (response.status === 401 && !skipAuth) {
    clearAuthSession()
    if (typeof window !== 'undefined') {
      window.dispatchEvent(new CustomEvent('swm:unauthorized'))
    }
  }

  if (response.status === 204) {
    return undefined as T
  }

  const contentType = response.headers.get('content-type') || ''
  let data: unknown = null
  if (contentType.includes('application/json')) {
    data = await response.json()
  } else {
    const text = await response.text()
    try {
      data = text ? JSON.parse(text) : null
    } catch {
      data = text
    }
  }

  if (!response.ok) {
    let message = `API-Fehler (${response.status})`
    if (data && typeof data === 'object' && 'detail' in data) {
      const detail = (data as { detail: unknown }).detail
      if (typeof detail === 'string') {
        message = detail
      } else if (Array.isArray(detail)) {
        message = detail
          .map((d) => (typeof d === 'object' && d && 'msg' in d ? d.msg : String(d)))
          .join(', ')
      }
    }
    throw new ApiError(message, response.status, data)
  }

  return data as T
}

export const adminApi = {
  // Health & Auth
  getHealth(): Promise<HealthResponse> {
    return apiRequest<HealthResponse>('/api/v1/health', { skipAuth: true })
  },

  async login(payload: LoginPayload): Promise<LoginResponse> {
    const res = await apiRequest<LoginResponse>('/api/v1/admin/auth/login', {
      method: 'POST',
      body: JSON.stringify(payload),
      skipAuth: true,
    })
    saveAuthSession(res.access_token, res.user)
    return res
  },

  getMe(): Promise<AdminUser> {
    return apiRequest<AdminUser>('/api/v1/admin/auth/me')
  },

  // Dashboard
  getDashboardSummary(): Promise<DashboardSummary> {
    return apiRequest<DashboardSummary>('/api/v1/admin/dashboard/summary')
  },

  // Devices
  async listDevices(filters?: DeviceFilters): Promise<Device[]> {
    const query = buildQuery(filters as Record<string, unknown>)
    const data = await apiRequest<unknown>(`/api/v1/admin/devices${query}`)
    return unwrapList<Device>(data, ['devices'])
  },

  getDevice(id: string): Promise<Device> {
    return apiRequest<Device>(`/api/v1/admin/devices/${encodeURIComponent(id)}`)
  },

  approveDevice(id: string): Promise<Device> {
    return apiRequest<Device>(`/api/v1/admin/devices/${encodeURIComponent(id)}/approve`, {
      method: 'POST',
    })
  },

  revokeDevice(id: string): Promise<Device> {
    return apiRequest<Device>(`/api/v1/admin/devices/${encodeURIComponent(id)}/revoke`, {
      method: 'POST',
    })
  },

  updateDevice(id: string, payload: DeviceUpdatePayload): Promise<Device> {
    return apiRequest<Device>(`/api/v1/admin/devices/${encodeURIComponent(id)}`, {
      method: 'PATCH',
      body: JSON.stringify(payload),
    })
  },

  deleteDevice(id: string): Promise<{ status: string; id: string }> {
    return apiRequest<{ status: string; id: string }>(`/api/v1/admin/devices/${encodeURIComponent(id)}`, {
      method: 'DELETE',
    })
  },

  scanDeviceUpdates(id: string): Promise<JobItem> {
    return apiRequest<JobItem>(`/api/v1/admin/devices/${encodeURIComponent(id)}/scan-updates`, {
      method: 'POST',
    })
  },

  syncDevice(id: string): Promise<JobItem> {
    return apiRequest<JobItem>(`/api/v1/admin/devices/${encodeURIComponent(id)}/sync`, {
      method: 'POST',
    })
  },

  notifyDevice(id: string, payload: DeviceNotifyPayload): Promise<JobItem> {
    return apiRequest<JobItem>(`/api/v1/admin/devices/${encodeURIComponent(id)}/notify`, {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },

  rebootDevice(id: string, payload: DeviceRebootPayload): Promise<JobItem> {
    return apiRequest<JobItem>(`/api/v1/admin/devices/${encodeURIComponent(id)}/reboot`, {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },

  bulkDeviceAction(payload: BulkDeviceActionPayload): Promise<BulkDeviceActionResponse> {
    return apiRequest<BulkDeviceActionResponse>('/api/v1/admin/devices/bulk-action', {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },

  // Groups & Policies
  async listGroups(): Promise<DeviceGroup[]> {
    const data = await apiRequest<unknown>('/api/v1/admin/groups')
    return unwrapList<DeviceGroup>(data, ['groups'])
  },

  createGroup(payload: GroupCreatePayload): Promise<DeviceGroup> {
    return apiRequest<DeviceGroup>('/api/v1/admin/groups', {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },

  updateGroup(id: string, payload: Partial<GroupCreatePayload>): Promise<DeviceGroup> {
    return apiRequest<DeviceGroup>(`/api/v1/admin/groups/${encodeURIComponent(id)}`, {
      method: 'PATCH',
      body: JSON.stringify(payload),
    })
  },

  deleteGroup(id: string): Promise<{ status: string; id: string }> {
    return apiRequest<{ status: string; id: string }>(`/api/v1/admin/groups/${encodeURIComponent(id)}`, {
      method: 'DELETE',
    })
  },

  triggerSelfUpdate(payload: SelfUpdatePayload): Promise<SelfUpdateResponse> {
    return apiRequest<SelfUpdateResponse>('/api/v1/admin/self-update', {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },

  // Maintenance Windows
  async listMaintenanceWindows(params?: { group_id?: string }): Promise<MaintenanceWindow[]> {
    const query = buildQuery(params as Record<string, unknown>)
    const data = await apiRequest<unknown>(`/api/v1/admin/maintenance-windows${query}`)
    return unwrapList<MaintenanceWindow>(data, ['maintenance_windows', 'windows'])
  },

  createMaintenanceWindow(payload: MaintenanceWindowCreatePayload): Promise<MaintenanceWindow> {
    return apiRequest<MaintenanceWindow>('/api/v1/admin/maintenance-windows', {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },

  deleteMaintenanceWindow(id: string): Promise<{ status: string; id: string }> {
    return apiRequest<{ status: string; id: string }>(
      `/api/v1/admin/maintenance-windows/${encodeURIComponent(id)}`,
      { method: 'DELETE' },
    )
  },

  // OS Patching
  async listPatches(params?: {
    os_family?: string
    severity?: string
    excluded?: boolean
    search?: string
  }): Promise<OsPatch[]> {
    const query = buildQuery(params as Record<string, unknown>)
    const data = await apiRequest<unknown>(`/api/v1/admin/patches${query}`)
    return unwrapList<OsPatch>(data, ['patches', 'updates'])
  },

  approvePatches(payload: PatchApprovePayload): Promise<PatchApproveResponse> {
    return apiRequest<PatchApproveResponse>('/api/v1/admin/patches/approve', {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },

  excludePatches(payload: PatchExcludePayload): Promise<{ update_ids: string[]; excluded: boolean }> {
    return apiRequest<{ update_ids: string[]; excluded: boolean }>('/api/v1/admin/patches/exclude', {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },

  // Software Catalog & Chunked Upload
  async listCatalog(params?: {
    os_family?: string
    source_type?: string
    category?: string
    search?: string
  }): Promise<CatalogItem[]> {
    const query = buildQuery(params as Record<string, unknown>)
    const data = await apiRequest<unknown>(`/api/v1/admin/catalog${query}`)
    return unwrapList<CatalogItem>(data, ['catalog', 'packages'])
  },

  createCatalogItem(payload: CatalogCreatePayload): Promise<CatalogItem> {
    return apiRequest<CatalogItem>('/api/v1/admin/catalog', {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },

  updateCatalogItem(id: string, payload: Partial<CatalogCreatePayload>): Promise<CatalogItem> {
    return apiRequest<CatalogItem>(`/api/v1/admin/catalog/${encodeURIComponent(id)}`, {
      method: 'PATCH',
      body: JSON.stringify(payload),
    })
  },

  deleteCatalogItem(id: string): Promise<{ status: string; id: string }> {
    return apiRequest<{ status: string; id: string }>(`/api/v1/admin/catalog/${encodeURIComponent(id)}`, {
      method: 'DELETE',
    })
  },

  uploadCatalogFile(id: string, file: File): Promise<CatalogItem> {
    const formData = new FormData()
    formData.append('file', file, file.name)
    return apiRequest<CatalogItem>(`/api/v1/admin/catalog/${encodeURIComponent(id)}/upload`, {
      method: 'POST',
      body: formData,
    })
  },

  uploadCatalogChunk(
    id: string,
    chunkIndex: number,
    totalChunks: number,
    chunkBlob: Blob,
    filename: string,
  ): Promise<ChunkUploadResponse> {
    const formData = new FormData()
    formData.append('chunk_index', String(chunkIndex))
    formData.append('total_chunks', String(totalChunks))
    formData.append('filename', filename)
    formData.append('file', chunkBlob, filename)

    return apiRequest<ChunkUploadResponse>(
      `/api/v1/admin/catalog/${encodeURIComponent(id)}/upload-chunk`,
      {
        method: 'POST',
        body: formData,
      },
    )
  },

  async uploadCatalogFileChunked(
    id: string,
    file: File,
    onProgress?: (percent: number, uploadedChunks: number, totalChunks: number) => void,
    chunkSizeBytes = 2 * 1024 * 1024,
  ): Promise<ChunkUploadResponse> {
    const totalChunks = Math.max(1, Math.ceil(file.size / chunkSizeBytes))
    let lastResponse: ChunkUploadResponse = {
      package_id: id,
      chunk_index: 0,
      total_chunks: totalChunks,
      completed: false,
    }

    for (let chunkIndex = 0; chunkIndex < totalChunks; chunkIndex++) {
      const start = chunkIndex * chunkSizeBytes
      const end = Math.min(file.size, start + chunkSizeBytes)
      const slice = file.slice(start, end)
      lastResponse = await this.uploadCatalogChunk(id, chunkIndex, totalChunks, slice, file.name)
      if (onProgress) {
        const percent = Math.round(((chunkIndex + 1) / totalChunks) * 100)
        onProgress(percent, chunkIndex + 1, totalChunks)
      }
    }

    return lastResponse
  },

  // Deployments
  async listDeployments(): Promise<Deployment[]> {
    const data = await apiRequest<unknown>('/api/v1/admin/deployments')
    return unwrapList<Deployment>(data, ['deployments'])
  },

  createDeployment(payload: DeploymentCreatePayload): Promise<Deployment> {
    return apiRequest<Deployment>('/api/v1/admin/deployments', {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },

  deleteDeployment(id: string): Promise<{ status: string; id: string }> {
    return apiRequest<{ status: string; id: string }>(
      `/api/v1/admin/deployments/${encodeURIComponent(id)}`,
      { method: 'DELETE' },
    )
  },

  // Jobs & Logs
  async listJobs(params?: {
    status?: string
    device_id?: string
    job_type?: string
  }): Promise<JobItem[]> {
    const query = buildQuery(params as Record<string, unknown>)
    const data = await apiRequest<unknown>(`/api/v1/admin/jobs${query}`)
    return unwrapList<JobItem>(data, ['jobs'])
  },

  getJob(id: string): Promise<JobItem> {
    return apiRequest<JobItem>(`/api/v1/admin/jobs/${encodeURIComponent(id)}`)
  },

  // Enrollment Tokens
  async listEnrollmentTokens(): Promise<EnrollmentToken[]> {
    const data = await apiRequest<unknown>('/api/v1/admin/enrollment-tokens')
    return unwrapList<EnrollmentToken>(data, ['tokens', 'enrollment_tokens'])
  },

  createEnrollmentToken(payload: EnrollmentTokenCreatePayload): Promise<EnrollmentToken> {
    return apiRequest<EnrollmentToken>('/api/v1/admin/enrollment-tokens', {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },

  deleteEnrollmentToken(id: string): Promise<{ status: string; id: string }> {
    return apiRequest<{ status: string; id: string }>(
      `/api/v1/admin/enrollment-tokens/${encodeURIComponent(id)}`,
      { method: 'DELETE' },
    )
  },

  async getAgentArtifacts(): Promise<AgentArtifactsResponse> {
    const raw = await apiRequest<unknown>('/api/v1/admin/agent-artifacts')
    if (raw && typeof raw === 'object' && !Array.isArray(raw)) {
      const obj = raw as Partial<AgentArtifactsResponse>
      return {
        server_url: typeof obj.server_url === 'string' ? obj.server_url : '',
        version: typeof obj.version === 'string' ? obj.version : '1.0.0',
        artifacts: Array.isArray(obj.artifacts) ? obj.artifacts : [],
      }
    }
    return {
      server_url: typeof window !== 'undefined' ? window.location.origin : '',
      version: '1.0.0',
      artifacts: [],
    }
  },
}
