from __future__ import annotations

import re
from datetime import datetime, timedelta, timezone
from typing import Any, Optional
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.config import Settings
from app.crypto.signing import SigningService
from app.models import (
    CatalogItem,
    Deployment,
    Device,
    InstalledSoftware,
    Job,
    MaintenanceWindow,
    ensure_utc,
    new_uuid,
    utc_now,
)
from app.schemas import DesktopPolicyOut, SignedCatalogItemOut, SignedJobOut
from app.services.ws_hub import WebSocketHub

ACTIVE_JOB_STATUSES = ("queued", "dispatched", "running", "waiting_for_process")


def current_utc_time() -> datetime:
    return utc_now()


def _get_tz(tz_name: str) -> timezone | ZoneInfo:
    if not tz_name or tz_name.upper() == "UTC":
        return timezone.utc
    try:
        return ZoneInfo(tz_name)
    except ZoneInfoNotFoundError:
        return timezone.utc


def _parse_hhmm(hhmm: str) -> tuple[int, int]:
    parts = hhmm.strip().split(":")
    return int(parts[0]), int(parts[1])


def is_window_active_at(mw: MaintenanceWindow, now: datetime) -> bool:
    if not mw.days_of_week:
        return False
    tz = _get_tz(mw.timezone)
    local_now = ensure_utc(now).astimezone(tz)
    sh, sm = _parse_hhmm(mw.start_time)
    eh, em = _parse_hhmm(mw.end_time)
    start_min = sh * 60 + sm
    end_min = eh * 60 + em
    cur_min = local_now.hour * 60 + local_now.minute
    weekday = local_now.weekday()

    if start_min < end_min:
        return weekday in mw.days_of_week and start_min <= cur_min < end_min
    if start_min > end_min:
        prev_day = (weekday - 1) % 7
        return (weekday in mw.days_of_week and cur_min >= start_min) or (
            prev_day in mw.days_of_week and cur_min < end_min
        )
    return weekday in mw.days_of_week


def most_recent_window_end_before(
    mw: MaintenanceWindow, now: datetime
) -> Optional[datetime]:
    """Finds the latest completed occurrence end timestamp (UTC) of `mw` at or before `now`."""
    if not mw.days_of_week:
        return None
    tz = _get_tz(mw.timezone)
    local_now = ensure_utc(now).astimezone(tz)
    sh, sm = _parse_hhmm(mw.start_time)
    eh, em = _parse_hhmm(mw.end_time)
    start_min = sh * 60 + sm
    end_min = eh * 60 + em

    candidates: list[datetime] = []
    base_date = local_now.date()
    for days_ago in range(0, 9):
        start_date = base_date - timedelta(days=days_ago)
        if start_date.weekday() not in mw.days_of_week:
            continue
        win_start = datetime(
            start_date.year, start_date.month, start_date.day, sh, sm, tzinfo=tz
        )
        if start_min < end_min:
            win_end = datetime(
                start_date.year, start_date.month, start_date.day, eh, em, tzinfo=tz
            )
        else:
            next_date = start_date + timedelta(days=1)
            win_end = datetime(
                next_date.year, next_date.month, next_date.day, eh, em, tzinfo=tz
            )
        if win_end <= local_now and win_start < win_end:
            candidates.append(win_end.astimezone(timezone.utc))

    return max(candidates) if candidates else None


def is_device_in_maintenance_window(
    windows: list[MaintenanceWindow], now: datetime
) -> bool:
    if not windows:
        return True
    return any(is_window_active_at(mw, now) for mw in windows)


def can_dispatch_job_now(
    job: Job,
    windows: list[MaintenanceWindow],
    now: datetime,
) -> bool:
    if not job.respect_maintenance_window:
        return True
    if not windows:
        return True
    if any(is_window_active_at(mw, now) for mw in windows):
        return True

    job_created = ensure_utc(job.created_at) or ensure_utc(now)
    for mw in windows:
        if not mw.catch_up:
            continue
        last_end = most_recent_window_end_before(mw, now)
        if last_end is not None and job_created <= last_end:
            return True
    return False


async def get_device_maintenance_windows(
    session: AsyncSession, device: Device
) -> list[MaintenanceWindow]:
    group_ids = [g.id for g in device.groups]
    if not group_ids:
        return []
    stmt = select(MaintenanceWindow).where(MaintenanceWindow.group_id.in_(group_ids))
    res = await session.execute(stmt)
    return list(res.scalars().all())


async def get_dispatchable_jobs_for_device(
    session: AsyncSession,
    device: Device,
    *,
    include_dispatched: bool = False,
    now: Optional[datetime] = None,
) -> list[Job]:
    """Returns pending jobs for `device` whose maintenance window allows immediate dispatch."""
    now_dt = ensure_utc(now) or current_utc_time()
    windows = await get_device_maintenance_windows(session, device)
    statuses = ("queued", "dispatched") if include_dispatched else ("queued",)
    jobs_res = await session.execute(
        select(Job)
        .where(
            Job.device_id == device.id,
            Job.status.in_(statuses),
        )
        .order_by(Job.created_at.asc())
    )
    candidate_jobs = list(jobs_res.scalars().all())
    return [j for j in candidate_jobs if can_dispatch_job_now(j, windows, now_dt)]


def resolve_device_reboot_policy(device: Device) -> tuple[str, int]:
    if not device.groups:
        return "never", 240
    if len(device.groups) == 1:
        g = device.groups[0]
        return g.reboot_policy, g.grace_period_minutes

    # Conservative precedence across multiple groups: never > maintenance_window > grace_period
    policies = {g.reboot_policy for g in device.groups}
    if "never" in policies:
        return "never", min(g.grace_period_minutes for g in device.groups)
    if "maintenance_window" in policies:
        return "maintenance_window", min(g.grace_period_minutes for g in device.groups)
    return "grace_period", min(g.grace_period_minutes for g in device.groups)


def resolve_device_desktop_policy(device: Device) -> DesktopPolicyOut:
    if not device.groups:
        return DesktopPolicyOut(
            enforce_tray=True,
            allow_self_service=True,
            allow_user_os_updates=True,
            silent_notifications=False,
        )
    return DesktopPolicyOut(
        enforce_tray=any(bool(g.enforce_tray) for g in device.groups),
        allow_self_service=all(bool(g.allow_self_service) for g in device.groups),
        allow_user_os_updates=all(bool(g.allow_user_os_updates) for g in device.groups),
        silent_notifications=any(bool(g.silent_notifications) for g in device.groups),
    )


_VER_TOKEN_RE = re.compile(r"\d+|[A-Za-z]+")


def _tokenize_version(ver: str) -> list[tuple[int, int | str]]:
    tokens: list[tuple[int, int | str]] = []
    for part in _VER_TOKEN_RE.findall(ver):
        if part.isdigit():
            tokens.append((1, int(part)))
        else:
            tokens.append((0, part.lower()))
    # Strip trailing numeric zeros so 24.08.0 == 24.8
    while len(tokens) > 1 and tokens[-1] == (1, 0):
        tokens.pop()
    return tokens


def compare_versions(installed_ver: str, target_ver: str) -> int:
    """Returns -1 if installed < target, 0 if equal/wildcard, 1 if installed > target."""
    if not target_ver or target_ver.lower() == "latest":
        return 0
    if not installed_ver:
        return -1
    t1 = _tokenize_version(installed_ver)
    t2 = _tokenize_version(target_ver)
    if t1 == t2:
        return 0
    return -1 if t1 < t2 else 1


def find_installed_package_version(
    installed_list: list[InstalledSoftware], pkg: CatalogItem
) -> Optional[str]:
    pkg_ref_low = (pkg.package_ref or "").strip().lower()
    pkg_name_low = pkg.name.strip().lower()

    for sw in installed_list:
        sw_ref_low = (sw.package_ref or "").strip().lower()
        sw_name_low = sw.name.strip().lower()
        if pkg_ref_low and sw_ref_low and sw_ref_low == pkg_ref_low:
            return sw.version or "0"
        if sw_name_low == pkg_name_low:
            return sw.version or "0"
        if sw_name_low.startswith(pkg_name_low + " ") or sw_name_low.startswith(
            pkg_name_low + "-"
        ):
            return sw.version or "0"
    return None


def create_signed_job(
    *,
    device_id: str,
    job_type: str,
    action: str,
    target_id: str,
    version: str = "",
    sha256: str = "",
    command: str = "",
    download_url: Optional[str] = None,
    update_ids: Optional[list[str]] = None,
    security_only: bool = False,
    deployment_id: Optional[str] = None,
    respect_maintenance_window: bool = True,
    initial_status: str = "queued",
    signing: SigningService,
    settings: Settings,
    now: Optional[datetime] = None,
) -> Job:
    now_dt = ensure_utc(now) or current_utc_time()
    job_id = new_uuid()
    expires_at = int(now_dt.timestamp()) + settings.JOB_SIGNATURE_TTL_SECONDS
    sig = signing.sign_job(
        job_id=job_id,
        device_id=device_id,
        job_type=job_type,
        action=action,
        target_id=target_id,
        version=version,
        sha256=sha256,
        command=command,
        expires_at_unix=expires_at,
    )
    return Job(
        id=job_id,
        device_id=device_id,
        deployment_id=deployment_id,
        job_type=job_type,
        action=action,
        target_id=target_id,
        version=version or "",
        sha256=sha256 or "",
        command=command or "",
        download_url=download_url,
        update_ids=list(update_ids or []),
        security_only=security_only,
        respect_maintenance_window=respect_maintenance_window,
        status=initial_status,
        expires_at_unix=expires_at,
        signature_b64=sig,
        created_at=now_dt,
        dispatched_at=now_dt if initial_status == "dispatched" else None,
    )


def _normalize_detection_rule_for_agent(raw: Optional[dict]) -> dict:
    if not raw or not isinstance(raw, dict):
        return {}
    cleaned = {k: v for k, v in raw.items() if v is not None}
    rtype = str(cleaned.get("type") or cleaned.get("rule_type") or "").strip().lower()
    if not rtype or rtype == "none":
        return {}
    if "key_path" in cleaned and "registry_key" not in cleaned:
        cleaned["registry_key"] = cleaned["key_path"]
    return cleaned


def build_signed_job_dto(
    job: Job,
    catalog_map: dict[str, CatalogItem],
    signing: SigningService,
    settings: Settings,
    now: Optional[datetime] = None,
) -> SignedJobOut:
    now_dt = ensure_utc(now) or current_utc_time()
    now_ts = int(now_dt.timestamp())
    if job.expires_at_unix < now_ts + 300:
        job.expires_at_unix = now_ts + settings.JOB_SIGNATURE_TTL_SECONDS
        job.signature_b64 = signing.sign_job(
            job_id=job.id,
            device_id=job.device_id,
            job_type=job.job_type,
            action=job.action,
            target_id=job.target_id,
            version=job.version,
            sha256=job.sha256,
            command=job.command,
            expires_at_unix=job.expires_at_unix,
        )

    pkg = catalog_map.get(job.target_id) if job.job_type == "software" else None
    pkg_meta = None
    detection_rule: dict = {}
    blocked_processes: list[str] = []
    return_codes: dict[str, str] = {
        "0": "success",
        "3010": "reboot_required",
        "1641": "reboot_required",
    }
    download_url = job.download_url

    if pkg is not None:
        pkg_meta = {
            "package_id": pkg.id,
            "name": pkg.name or "",
            "vendor": pkg.vendor or "",
            "version": pkg.version or "",
            "os_family": pkg.os_family or "",
            "source_type": pkg.source_type or "",
            "package_ref": pkg.package_ref or "",
            "filename": pkg.filename or "",
            "file_size": pkg.file_size or 0,
            "install_args": pkg.install_args or "",
            "uninstall_args": pkg.uninstall_args or "",
        }
        detection_rule = _normalize_detection_rule_for_agent(pkg.detection_rule)
        blocked_processes = list(pkg.blocked_processes or [])
        if pkg.return_codes:
            return_codes = {str(k): str(v) for k, v in pkg.return_codes.items()}
        if not download_url and (pkg.source_type == "custom" or pkg.storage_path):
            download_url = f"/api/v1/agent/packages/{pkg.id}/download"

    return SignedJobOut(
        job_id=job.id,
        device_id=job.device_id,
        job_type=job.job_type,
        action=job.action,
        target_id=job.target_id,
        version=job.version or "",
        sha256=job.sha256 or "",
        command=job.command or "",
        expires_at_unix=job.expires_at_unix,
        signature_b64=job.signature_b64,
        download_url=download_url,
        update_ids=list(job.update_ids or []),
        security_only=job.security_only,
        package_meta=pkg_meta,
        detection_rule=detection_rule,
        blocked_processes=blocked_processes,
        return_codes=return_codes,
    )


def device_matches_deployment(device: Device, dep: Deployment) -> bool:
    if device.status != "approved":
        return False
    if device.id in (dep.target_device_ids or []):
        return True
    dev_group_ids = {g.id for g in device.groups}
    return bool(dev_group_ids.intersection(dep.target_group_ids or []))


async def evaluate_deployments_for_device(
    session: AsyncSession,
    device: Device,
    signing: SigningService,
    settings: Settings,
    ws_hub: Optional[WebSocketHub] = None,
    from_deployment_creation: bool = False,
    now: Optional[datetime] = None,
) -> list[Job]:
    """Evaluates active required/uninstall deployments for a device and enqueues needed jobs."""
    if device.status != "approved":
        return []

    dep_res = await session.execute(
        select(Deployment).where(Deployment.mode.in_(("required", "uninstall")))
    )
    deployments = [
        d for d in dep_res.scalars().all() if device_matches_deployment(device, d)
    ]
    if not deployments:
        return []

    pkg_ids = list({d.package_id for d in deployments})
    pkg_res = await session.execute(
        select(CatalogItem).where(CatalogItem.id.in_(pkg_ids))
    )
    pkg_map = {p.id: p for p in pkg_res.scalars().all()}

    jobs_res = await session.execute(
        select(Job).where(Job.device_id == device.id, Job.job_type == "software")
    )
    existing_jobs = list(jobs_res.scalars().all())
    windows = await get_device_maintenance_windows(session, device)
    now_dt = ensure_utc(now) or current_utc_time()

    created_jobs: list[Job] = []

    for dep in deployments:
        pkg = pkg_map.get(dep.package_id)
        if pkg is None or pkg.os_family != device.os_family:
            continue

        installed_ver = find_installed_package_version(device.installed_software, pkg)
        has_active_job = any(
            j.target_id == pkg.id and j.status in ACTIVE_JOB_STATUSES
            for j in existing_jobs
        )
        if has_active_job:
            continue

        dl_url = (
            f"/api/v1/agent/packages/{pkg.id}/download"
            if (pkg.source_type == "custom" or pkg.storage_path)
            else None
        )

        if dep.mode == "required":
            action: Optional[str] = None
            completed_pkg_jobs = sorted(
                (
                    j
                    for j in existing_jobs
                    if j.target_id == pkg.id and j.status in ("succeeded", "failed")
                ),
                key=lambda j: (
                    ensure_utc(j.completed_at or j.created_at) or now_dt,
                    ensure_utc(j.created_at) or now_dt,
                ),
                reverse=True,
            )
            latest_completed = completed_pkg_jobs[0] if completed_pkg_jobs else None

            if installed_ver is None:
                already_succeeded = (
                    (pkg.source_type == "custom" or not device.installed_software)
                    and latest_completed is not None
                    and latest_completed.action in ("install", "update")
                    and latest_completed.version == pkg.version
                    and latest_completed.status == "succeeded"
                )
                if not already_succeeded:
                    action = "install"
            elif compare_versions(installed_ver, pkg.version) < 0:
                action = "update"

            if action:
                job = create_signed_job(
                    device_id=device.id,
                    deployment_id=dep.id,
                    job_type="software",
                    action=action,
                    target_id=pkg.id,
                    version=pkg.version,
                    sha256=pkg.sha256 or "",
                    command=pkg.install_args or "",
                    download_url=dl_url,
                    respect_maintenance_window=dep.respect_maintenance_window,
                    signing=signing,
                    settings=settings,
                    now=now_dt,
                )
                session.add(job)
                existing_jobs.append(job)
                created_jobs.append(job)

        elif dep.mode == "uninstall":
            should_uninstall = installed_ver is not None or (
                from_deployment_creation
                and not device.installed_software
                and not any(
                    j.target_id == pkg.id and j.action == "uninstall"
                    for j in existing_jobs
                )
            )
            if should_uninstall:
                job = create_signed_job(
                    device_id=device.id,
                    deployment_id=dep.id,
                    job_type="software",
                    action="uninstall",
                    target_id=pkg.id,
                    version=pkg.version,
                    sha256=pkg.sha256 or "",
                    command=pkg.uninstall_args or "",
                    download_url=None,
                    respect_maintenance_window=dep.respect_maintenance_window,
                    signing=signing,
                    settings=settings,
                    now=now_dt,
                )
                session.add(job)
                existing_jobs.append(job)
                created_jobs.append(job)

    if created_jobs:
        await session.flush()
        if ws_hub is not None:
            for job in created_jobs:
                if can_dispatch_job_now(job, windows, now_dt):
                    await ws_hub.send_to_device(
                        device.id, {"type": "job_available", "job_id": job.id}
                    )

    return created_jobs


async def run_scheduler_tick(
    session: AsyncSession,
    settings: Settings,
    signing: SigningService,
    ws_hub: WebSocketHub,
    *,
    now: Optional[datetime] = None,
) -> dict[str, int]:
    """Runs one background scheduler tick:
    a) Sweeps stale `is_online == True` devices (not actively on `ws_hub`) to `False`.
    b) Evaluates active deployments and proactively dispatches newly unlocked `queued` jobs
       over WebSocket (`{"type": "job_available", "job_id": ..., "job": ...}`) when a
       maintenance window opens.
    """
    now_dt = ensure_utc(now) or current_utc_time()
    stale_cutoff = now_dt - timedelta(seconds=settings.ONLINE_STALE_SECONDS)

    swept_offline = 0
    online_res = await session.execute(
        select(Device).where(Device.is_online.is_(True))
    )
    for dev in online_res.scalars().all():
        if ws_hub.is_online(dev.id):
            continue
        last_seen = ensure_utc(dev.last_seen_at)
        if last_seen is None or last_seen <= stale_cutoff:
            dev.is_online = False
            swept_offline += 1

    enqueued_count = 0
    dispatched_count = 0
    pending_ws_pushes: list[tuple[str, dict[str, Any]]] = []

    connected_ids = ws_hub.connected_device_ids()
    if connected_ids:
        conn_res = await session.execute(
            select(Device).where(
                Device.id.in_(connected_ids),
                Device.status == "approved",
            )
        )
        connected_devices = list(conn_res.scalars().all())

        for dev in connected_devices:
            enqueued = await evaluate_deployments_for_device(
                session,
                dev,
                signing,
                settings,
                ws_hub=None,
                now=now_dt,
            )
            enqueued_count += len(enqueued)

            ready_jobs = await get_dispatchable_jobs_for_device(
                session,
                dev,
                include_dispatched=False,
                now=now_dt,
            )
            if not ready_jobs:
                continue

            pkg_ids = list(
                {j.target_id for j in ready_jobs if j.job_type == "software"}
            )
            catalog_map: dict[str, CatalogItem] = {}
            if pkg_ids:
                pkg_res = await session.execute(
                    select(CatalogItem).where(CatalogItem.id.in_(pkg_ids))
                )
                catalog_map = {p.id: p for p in pkg_res.scalars().all()}

            for job in ready_jobs:
                job.status = "dispatched"
                job.dispatched_at = now_dt
                signed_dto = build_signed_job_dto(
                    job, catalog_map, signing, settings, now=now_dt
                )
                pending_ws_pushes.append(
                    (
                        dev.id,
                        {
                            "type": "job_available",
                            "job_id": job.id,
                            "job": signed_dto.model_dump(mode="json"),
                        },
                    )
                )
                dispatched_count += 1

    await session.commit()

    for dev_id, ws_payload in pending_ws_pushes:
        await ws_hub.send_to_device(dev_id, ws_payload)

    return {
        "swept_offline": swept_offline,
        "enqueued_jobs": enqueued_count,
        "dispatched_jobs": dispatched_count,
    }


async def build_available_catalog_for_device(
    session: AsyncSession,
    device: Device,
    signing: SigningService,
) -> list[SignedCatalogItemOut]:
    dep_res = await session.execute(
        select(Deployment).where(Deployment.mode == "available")
    )
    deployments = [
        d for d in dep_res.scalars().all() if device_matches_deployment(device, d)
    ]
    if not deployments:
        return []

    pkg_ids = list({d.package_id for d in deployments})
    pkg_res = await session.execute(
        select(CatalogItem).where(CatalogItem.id.in_(pkg_ids))
    )
    pkgs = [p for p in pkg_res.scalars().all() if p.os_family == device.os_family]
    if not pkgs:
        return []

    jobs_res = await session.execute(
        select(Job).where(
            Job.device_id == device.id,
            Job.job_type == "software",
            Job.status.in_(ACTIVE_JOB_STATUSES),
        )
    )
    active_pkg_ids = {j.target_id for j in jobs_res.scalars().all()}

    items: list[SignedCatalogItemOut] = []
    for pkg in sorted(pkgs, key=lambda x: x.name.lower()):
        installed_ver = find_installed_package_version(device.installed_software, pkg)
        if pkg.id in active_pkg_ids:
            pkg_status = "installing"
        elif installed_ver is not None:
            if compare_versions(installed_ver, pkg.version) < 0:
                pkg_status = "update_available"
            else:
                pkg_status = "installed"
        else:
            pkg_status = "available"

        dl_url = (
            f"/api/v1/agent/packages/{pkg.id}/download"
            if (pkg.source_type == "custom" or pkg.storage_path)
            else None
        )
        sig = signing.sign_catalog_item(
            package_id=pkg.id,
            device_id=device.id,
            source_type=pkg.source_type,
            package_ref=pkg.package_ref or "",
            version=pkg.version or "",
            sha256=pkg.sha256 or "",
            install_cmd=pkg.install_args or "",
            uninstall_cmd=pkg.uninstall_args or "",
            allow_uninstall=pkg.allow_uninstall,
        )
        items.append(
            SignedCatalogItemOut(
                package_id=pkg.id,
                device_id=device.id,
                name=pkg.name,
                vendor=pkg.vendor,
                description=pkg.description,
                category=pkg.category,
                icon_url=pkg.icon_url,
                os_family=pkg.os_family,
                source_type=pkg.source_type,
                package_ref=pkg.package_ref or "",
                version=pkg.version or "",
                sha256=pkg.sha256 or "",
                file_size=pkg.file_size,
                download_url=dl_url,
                install_cmd=pkg.install_args or "",
                uninstall_cmd=pkg.uninstall_args or "",
                allow_uninstall=pkg.allow_uninstall,
                installed_version=installed_ver,
                status=pkg_status,
                blocked_processes=list(pkg.blocked_processes or []),
                detection_rule=_normalize_detection_rule_for_agent(pkg.detection_rule),
                return_codes={str(k): str(v) for k, v in (pkg.return_codes or {}).items()},
                signature_b64=sig,
            )
        )
    return items
