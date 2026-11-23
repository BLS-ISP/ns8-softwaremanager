from __future__ import annotations

import re
from datetime import datetime
from typing import Any, Literal, Optional

from pydantic import BaseModel, ConfigDict, Field, field_validator, model_validator

TIME_HHMM_RE = re.compile(r"^([01]\d|2[0-3]):[0-5]\d$")


class HealthResponse(BaseModel):
    status: str = "ok"
    version: str = "1.0.0"
    database: str = "connected"


# --- Auth & Dashboard ---


class LoginRequest(BaseModel):
    username: str = Field(..., min_length=1, max_length=128)
    password: str = Field(..., min_length=1, max_length=256)
    auth_source: Literal["local", "ldap"] = "local"


class AdminUserOut(BaseModel):
    model_config = ConfigDict(from_attributes=True)

    id: str
    username: str
    role: str
    auth_source: str


class LoginResponse(BaseModel):
    access_token: str
    token_type: str = "bearer"
    user: AdminUserOut


class OsBreakdownOut(BaseModel):
    windows: int = 0
    linux: int = 0


class JobOut(BaseModel):
    id: str
    device_id: str
    device_hostname: str = ""
    deployment_id: Optional[str] = None
    job_type: str
    action: str
    target_id: str
    version: str = ""
    sha256: str = ""
    command: str = ""
    download_url: Optional[str] = None
    update_ids: list[str] = Field(default_factory=list)
    security_only: bool = False
    respect_maintenance_window: bool = True
    status: str
    exit_code: Optional[int] = None
    reboot_required: bool = False
    stdout: str = ""
    stderr: str = ""
    expires_at_unix: int
    signature_b64: str
    created_at: datetime
    dispatched_at: Optional[datetime] = None
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None


class DashboardSummaryOut(BaseModel):
    total_devices: int
    online_devices: int
    pending_approval_devices: int
    reboot_required_devices: int
    patch_compliance_percent: float
    pending_critical_patches: int
    recent_jobs: list[JobOut]
    os_breakdown: OsBreakdownOut


# --- Enrollment Tokens ---


class EnrollmentTokenCreate(BaseModel):
    name: str = Field(..., min_length=1, max_length=128)
    expires_in_hours: int = Field(default=24, ge=1, le=8760)
    max_uses: Optional[int] = Field(default=None, ge=1)
    auto_approve: bool = False
    group_id: Optional[str] = None


class EnrollmentTokenOut(BaseModel):
    id: str
    name: str
    token: str
    expires_at: datetime
    max_uses: Optional[int] = None
    uses_count: int
    auto_approve: bool
    group_id: Optional[str] = None
    revoked: bool
    created_at: datetime
    install_ps1_cmd: str
    install_sh_cmd: str


# --- Groups & Maintenance Windows ---


class DesktopPolicyOut(BaseModel):
    enforce_tray: bool = True
    allow_self_service: bool = True
    allow_user_os_updates: bool = True
    silent_notifications: bool = False


class GroupCreate(BaseModel):
    name: str = Field(..., min_length=1, max_length=128)
    description: str = Field(default="", max_length=512)
    reboot_policy: Literal["never", "maintenance_window", "grace_period"] = "never"
    grace_period_minutes: int = Field(default=240, ge=5, le=10080)
    enforce_tray: bool = True
    allow_self_service: bool = True
    allow_user_os_updates: bool = True
    silent_notifications: bool = False


class GroupUpdate(BaseModel):
    name: Optional[str] = Field(default=None, min_length=1, max_length=128)
    description: Optional[str] = Field(default=None, max_length=512)
    reboot_policy: Optional[Literal["never", "maintenance_window", "grace_period"]] = None
    grace_period_minutes: Optional[int] = Field(default=None, ge=5, le=10080)
    enforce_tray: Optional[bool] = None
    allow_self_service: Optional[bool] = None
    allow_user_os_updates: Optional[bool] = None
    silent_notifications: Optional[bool] = None


class GroupOut(BaseModel):
    id: str
    name: str
    description: str
    reboot_policy: str
    grace_period_minutes: int
    enforce_tray: bool = True
    allow_self_service: bool = True
    allow_user_os_updates: bool = True
    silent_notifications: bool = False
    device_count: int = 0
    created_at: datetime


class GroupBriefOut(BaseModel):
    id: str
    name: str


class MaintenanceWindowCreate(BaseModel):
    group_id: str
    name: str = Field(..., min_length=1, max_length=128)
    days_of_week: list[int] = Field(..., min_length=1)
    start_time: str
    end_time: str
    timezone: str = "UTC"
    catch_up: bool = True

    @field_validator("days_of_week")
    @classmethod
    def validate_days(cls, v: list[int]) -> list[int]:
        for d in v:
            if d < 0 or d > 6:
                raise ValueError("days_of_week entries must be between 0 (Mon) and 6 (Sun)")
        return sorted(set(v))

    @field_validator("start_time", "end_time")
    @classmethod
    def validate_hhmm(cls, v: str) -> str:
        if not TIME_HHMM_RE.match(v):
            raise ValueError("Time must be formatted as HH:MM (24-hour)")
        return v


class MaintenanceWindowOut(BaseModel):
    id: str
    group_id: str
    group_name: str = ""
    name: str
    days_of_week: list[int]
    start_time: str
    end_time: str
    timezone: str
    catch_up: bool
    created_at: datetime


# --- Devices & Inventory ---


class SoftwareEntryIn(BaseModel):
    model_config = ConfigDict(extra="ignore")

    name: str
    version: str = ""
    publisher: str = ""
    vendor: Optional[str] = None
    source: str = "system"
    scope: str = "machine"
    package_ref: str = ""

    @model_validator(mode="after")
    def sync_publisher_vendor(self) -> "SoftwareEntryIn":
        if not self.publisher and self.vendor:
            self.publisher = self.vendor
        return self


class SoftwareEntryOut(BaseModel):
    name: str
    version: str
    publisher: str
    source: str
    scope: str
    package_ref: str = ""


class PendingOsUpdateIn(BaseModel):
    model_config = ConfigDict(extra="ignore")

    update_id: str
    title: str = ""
    severity: str = "important"
    category: str = "security"
    size_bytes: int = 0
    reboot_behavior: str = "may_require_reboot"


class PendingOsUpdateOut(BaseModel):
    update_id: str
    title: str
    severity: str
    category: str
    size_bytes: int
    reboot_behavior: str
    approved: bool = False
    excluded: bool = False
    reported_at: Optional[datetime] = None


class DevicePatchRequest(BaseModel):
    group_ids: Optional[list[str]] = None
    tags: Optional[list[str]] = None


class DeviceNotifyRequest(BaseModel):
    title: str = Field(..., min_length=1, max_length=255)
    message: str = Field(..., min_length=1, max_length=4000)
    urgent: bool = False


class DeviceRemoteRebootRequest(BaseModel):
    mode: Literal["grace_period", "immediate", "cancel"]
    grace_minutes: int = Field(default=60, ge=1, le=10080)

    @field_validator("grace_minutes", mode="before")
    @classmethod
    def default_none_grace_minutes(cls, v: Any) -> Any:
        return 60 if v is None else v


class BulkDeviceActionRequest(BaseModel):
    device_ids: list[str]
    action: Literal["approve", "sync", "scan_updates", "assign_group", "notify"]
    group_id: Optional[str] = None
    title: Optional[str] = None
    message: Optional[str] = None
    urgent: bool = False


class BulkDeviceActionResponse(BaseModel):
    affected: int
    job_ids: list[str] = Field(default_factory=list)


class DeviceSummaryOut(BaseModel):
    id: str
    hostname: str
    domain: str = ""
    os_family: str
    os_name: str
    os_version: str
    kernel_or_build: str = ""
    arch: str
    agent_version: str
    machine_uid: str
    status: str
    is_online: bool
    reboot_required: bool
    center_running: bool = False
    ip_addresses: list[str] = Field(default_factory=list)
    logged_in_users: list[str] = Field(default_factory=list)
    group_ids: list[str] = Field(default_factory=list)
    groups: list[GroupBriefOut] = Field(default_factory=list)
    tags: list[str] = Field(default_factory=list)
    pending_updates_count: int = 0
    installed_software_count: int = 0
    last_seen_at: Optional[datetime] = None
    created_at: datetime


DeviceOut = DeviceSummaryOut


class DeviceDetailOut(DeviceSummaryOut):
    hardware: dict[str, Any] = Field(default_factory=dict)
    installed_software: list[SoftwareEntryOut] = Field(default_factory=list)
    pending_os_updates: list[PendingOsUpdateOut] = Field(default_factory=list)
    recent_jobs: list[JobOut] = Field(default_factory=list)


# --- OS Patching ---


class PatchSummaryOut(BaseModel):
    update_id: str
    title: str
    os_family: str
    severity: str
    category: str
    size_bytes: int = 0
    affected_device_count: int = 0
    affected_device_ids: list[str] = Field(default_factory=list)
    approved: bool = False
    excluded: bool = False
    last_seen_at: Optional[datetime] = None


class PatchApproveRequest(BaseModel):
    update_ids: list[str] = Field(..., min_length=1)
    group_ids: list[str] = Field(default_factory=list)
    device_ids: list[str] = Field(default_factory=list)
    respect_maintenance_window: bool = True
    security_only: bool = False


class PatchExcludeRequest(BaseModel):
    update_ids: list[str] = Field(..., min_length=1)
    excluded: bool = True


# --- Software Catalog & Deployments ---


def _normalize_return_codes(v: Any) -> dict[str, str]:
    if not v or not isinstance(v, dict):
        return {"0": "success", "3010": "reboot_required", "1641": "reboot_required"}
    return {str(k): str(val) for k, val in v.items()}


class CatalogItemCreate(BaseModel):
    name: str = Field(..., min_length=1, max_length=255)
    version: str = Field(default="latest", max_length=128)
    vendor: str = Field(default="", max_length=255)
    description: str = ""
    category: str = Field(default="General", max_length=128)
    icon_url: str = ""
    os_family: Literal["windows", "linux"]
    source_type: Literal["winget", "apt", "dnf", "flatpak", "custom"]
    package_ref: str = Field(default="", max_length=255)
    install_args: str = ""
    uninstall_args: str = ""
    blocked_processes: list[str] = Field(default_factory=list)
    allow_uninstall: bool = True
    return_codes: dict[str, str] = Field(
        default_factory=lambda: {
            "0": "success",
            "3010": "reboot_required",
            "1641": "reboot_required",
        }
    )
    detection_rule: dict[str, Any] = Field(default_factory=dict)

    @field_validator("return_codes", mode="before")
    @classmethod
    def coerce_return_codes(cls, v: Any) -> dict[str, str]:
        return _normalize_return_codes(v)


class CatalogItemUpdate(BaseModel):
    name: Optional[str] = Field(default=None, min_length=1, max_length=255)
    version: Optional[str] = Field(default=None, max_length=128)
    vendor: Optional[str] = Field(default=None, max_length=255)
    description: Optional[str] = None
    category: Optional[str] = Field(default=None, max_length=128)
    icon_url: Optional[str] = None
    os_family: Optional[Literal["windows", "linux"]] = None
    source_type: Optional[Literal["winget", "apt", "dnf", "flatpak", "custom"]] = None
    package_ref: Optional[str] = Field(default=None, max_length=255)
    install_args: Optional[str] = None
    uninstall_args: Optional[str] = None
    blocked_processes: Optional[list[str]] = None
    allow_uninstall: Optional[bool] = None
    return_codes: Optional[dict[str, str]] = None
    detection_rule: Optional[dict[str, Any]] = None

    @field_validator("return_codes", mode="before")
    @classmethod
    def coerce_optional_return_codes(cls, v: Any) -> Optional[dict[str, str]]:
        if v is None:
            return None
        return _normalize_return_codes(v)


class CatalogItemOut(BaseModel):
    id: str
    name: str
    version: str
    vendor: str
    description: str
    category: str
    icon_url: str
    os_family: str
    source_type: str
    package_ref: str
    install_args: str
    uninstall_args: str
    blocked_processes: list[str]
    allow_uninstall: bool
    return_codes: dict[str, str]
    detection_rule: dict[str, Any]
    filename: Optional[str] = None
    file_size: int = 0
    sha256: str = ""
    created_at: datetime
    updated_at: datetime


class ChunkUploadResponse(BaseModel):
    package_id: str
    chunk_index: int
    total_chunks: int
    completed: bool
    sha256: Optional[str] = None
    file_size: Optional[int] = None
    catalog_item: Optional[CatalogItemOut] = None


class DeploymentCreate(BaseModel):
    package_id: str
    target_group_ids: list[str] = Field(default_factory=list)
    target_device_ids: list[str] = Field(default_factory=list)
    mode: Literal["required", "available", "uninstall"]
    respect_maintenance_window: bool = True


class DeploymentStatsOut(BaseModel):
    total: int = 0
    installed: int = 0
    pending: int = 0
    failed: int = 0


class DeploymentOut(BaseModel):
    id: str
    package_id: str
    package_name: str = ""
    package_version: str = ""
    os_family: str = ""
    target_group_ids: list[str]
    target_device_ids: list[str]
    mode: str
    respect_maintenance_window: bool
    stats: DeploymentStatsOut = Field(default_factory=DeploymentStatsOut)
    created_at: datetime


class SelfUpdateRequest(BaseModel):
    version: str = Field(..., min_length=1, max_length=64)
    os_family: Optional[Literal["windows", "linux"]] = None
    sha256: str = ""
    download_url: Optional[str] = None
    target_group_ids: list[str] = Field(default_factory=list)
    target_device_ids: list[str] = Field(default_factory=list)
    respect_maintenance_window: bool = True


# --- Agent Wire DTOs ---


class AgentEnrollRequest(BaseModel):
    model_config = ConfigDict(extra="ignore")

    enrollment_token: str = Field(..., min_length=1)
    hostname: str = Field(..., min_length=1, max_length=255)
    os_family: str = Field(..., min_length=1, max_length=32)
    os_name: str = ""
    os_version: str = ""
    arch: str = "amd64"
    agent_version: str = "1.0.0"
    machine_uid: str = Field(..., min_length=1, max_length=255)

    @field_validator("os_family")
    @classmethod
    def normalize_os_family(cls, v: str) -> str:
        norm = v.strip().lower()
        if norm in ("win", "windows", "win32", "win64"):
            return "windows"
        if norm in ("linux", "gnu/linux", "debian", "ubuntu", "rhel", "fedora"):
            return "linux"
        if norm not in ("windows", "linux"):
            raise ValueError("os_family must be 'windows' or 'linux'")
        return norm


class AgentEnrollResponse(BaseModel):
    device_id: str
    device_token: str
    status: str
    server_public_key_b64: str


class AgentStatusResponse(BaseModel):
    device_id: str
    status: str
    reboot_policy: str
    grace_period_minutes: int
    in_maintenance_window: bool = True
    excluded_updates: list[str] = Field(default_factory=list)
    server_public_key_b64: str
    desktop_policy: DesktopPolicyOut = Field(default_factory=DesktopPolicyOut)


class AgentInventoryRequest(BaseModel):
    model_config = ConfigDict(extra="ignore")

    hostname: Optional[str] = None
    domain: str = ""
    os_family: Optional[str] = None
    os_name: Optional[str] = None
    os_version: Optional[str] = None
    kernel_or_build: str = ""
    arch: Optional[str] = None
    agent_version: Optional[str] = None
    hardware: dict[str, Any] = Field(default_factory=dict)
    ip_addresses: list[str] = Field(default_factory=list)
    logged_in_users: list[str] = Field(default_factory=list)
    reboot_required: bool = False
    center_running: bool = False
    software_hash: Optional[str] = None
    installed_software: Optional[list[SoftwareEntryIn]] = None
    pending_os_updates: Optional[list[PendingOsUpdateIn]] = None


class SignedJobOut(BaseModel):
    job_id: str
    device_id: str
    job_type: str
    action: str
    target_id: str
    version: str = ""
    sha256: str = ""
    command: str = ""
    expires_at_unix: int
    signature_b64: str
    download_url: Optional[str] = None
    update_ids: list[str] = Field(default_factory=list)
    security_only: bool = False
    package_meta: Optional[dict[str, Any]] = None
    detection_rule: dict[str, Any] = Field(default_factory=dict)
    blocked_processes: list[str] = Field(default_factory=list)
    return_codes: dict[str, str] = Field(
        default_factory=lambda: {"0": "success", "3010": "reboot_required"}
    )


class SignedCatalogItemOut(BaseModel):
    package_id: str
    device_id: str
    name: str
    vendor: str = ""
    description: str = ""
    category: str = "General"
    icon_url: str = ""
    os_family: str
    source_type: str
    package_ref: str
    version: str
    sha256: str = ""
    file_size: int = 0
    download_url: Optional[str] = None
    install_cmd: str = ""
    uninstall_cmd: str = ""
    allow_uninstall: bool = True
    installed_version: Optional[str] = None
    status: str = "available"
    blocked_processes: list[str] = Field(default_factory=list)
    detection_rule: dict[str, Any] = Field(default_factory=dict)
    return_codes: dict[str, str] = Field(
        default_factory=lambda: {"0": "success", "3010": "reboot_required"}
    )
    signature_b64: str


class AgentPendingJobsResponse(BaseModel):
    jobs: list[SignedJobOut]
    available_catalog: list[SignedCatalogItemOut]


class JobResultIn(BaseModel):
    model_config = ConfigDict(extra="ignore")

    status: str
    exit_code: Optional[int] = None
    reboot_required: bool = False
    stdout: str = ""
    stderr: str = ""


class SelfServiceTriggerRequest(BaseModel):
    package_id: str
    action: Literal["install", "update", "uninstall"] = "install"
