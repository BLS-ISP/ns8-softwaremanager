from __future__ import annotations

import json
from typing import Optional

from fastapi import APIRouter, Depends, HTTPException, Query, Request, status
from sqlalchemy import desc, func, select
from sqlalchemy.ext.asyncio import AsyncSession
from sqlalchemy.orm import selectinload

from app.api.admin_auth import job_to_out
from app.auth import get_current_admin, get_db, get_settings_dep
from app.config import Settings
from app.models import (
    AdminUser,
    AuditLog,
    Device,
    Group,
    Job,
    MaintenanceWindow,
    PatchPolicy,
    device_group_links,
)
from app.schemas import (
    BulkDeviceActionRequest,
    BulkDeviceActionResponse,
    DeviceDetailOut,
    DeviceNotifyRequest,
    DevicePatchRequest,
    DeviceRemoteRebootRequest,
    DeviceSummaryOut,
    GroupBriefOut,
    GroupCreate,
    GroupOut,
    GroupUpdate,
    JobOut,
    MaintenanceWindowCreate,
    MaintenanceWindowOut,
    PendingOsUpdateOut,
    SoftwareEntryOut,
)
from app.services import (
    can_dispatch_job_now,
    create_signed_job,
    current_utc_time,
    evaluate_deployments_for_device,
    get_device_maintenance_windows,
)

router = APIRouter(prefix="/api/v1/admin", tags=["admin-devices-groups"])


def _device_to_summary(device: Device, ws_online: bool = False) -> DeviceSummaryOut:
    is_online = device.status == "approved" and (device.is_online or ws_online)
    groups = [GroupBriefOut(id=g.id, name=g.name) for g in device.groups]
    return DeviceSummaryOut(
        id=device.id,
        hostname=device.hostname,
        domain=device.domain,
        os_family=device.os_family,
        os_name=device.os_name,
        os_version=device.os_version,
        kernel_or_build=device.kernel_or_build,
        arch=device.arch,
        agent_version=device.agent_version,
        machine_uid=device.machine_uid,
        status=device.status,
        is_online=is_online,
        reboot_required=device.reboot_required,
        center_running=bool(device.center_running),
        ip_addresses=list(device.ip_addresses or []),
        logged_in_users=list(device.logged_in_users or []),
        group_ids=[g.id for g in device.groups],
        groups=groups,
        tags=list(device.tags or []),
        pending_updates_count=len(device.pending_updates or []),
        installed_software_count=len(device.installed_software or []),
        last_seen_at=device.last_seen_at,
        created_at=device.created_at,
    )


@router.get("/devices", response_model=list[DeviceSummaryOut])
async def list_devices(
    request: Request,
    status_filter: Optional[str] = Query(default=None, alias="status"),
    group_id: Optional[str] = Query(default=None),
    os_family: Optional[str] = Query(default=None),
    search: Optional[str] = Query(default=None),
    software: Optional[str] = Query(default=None),
    _admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> list[DeviceSummaryOut]:
    ws_hub = request.app.state.ws_hub
    stmt = select(Device).order_by(desc(Device.created_at))
    if status_filter:
        stmt = stmt.where(Device.status == status_filter)
    if os_family:
        stmt = stmt.where(Device.os_family == os_family.lower())

    res = await session.execute(stmt)
    devices = list(res.scalars().all())

    if group_id:
        devices = [d for d in devices if any(g.id == group_id for g in d.groups)]

    if search:
        q = search.strip().lower()
        devices = [
            d
            for d in devices
            if q in d.hostname.lower()
            or q in d.os_name.lower()
            or q in d.os_version.lower()
            or any(q in ip.lower() for ip in (d.ip_addresses or []))
            or any(q in t.lower() for t in (d.tags or []))
            or any(
                q in sw.name.lower() or q in (sw.version or "").lower()
                for sw in (d.installed_software or [])
            )
        ]

    if software:
        sq = software.strip().lower()
        devices = [
            d
            for d in devices
            if any(
                sq in sw.name.lower() or sq in (sw.package_ref or "").lower()
                for sw in (d.installed_software or [])
            )
        ]

    return [_device_to_summary(d, ws_hub.is_online(d.id)) for d in devices]


@router.post("/devices/bulk-action", response_model=BulkDeviceActionResponse)
async def bulk_device_action(
    payload: BulkDeviceActionRequest,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> BulkDeviceActionResponse:
    unique_ids = list(dict.fromkeys(payload.device_ids))
    if not unique_ids:
        return BulkDeviceActionResponse(affected=0, job_ids=[])

    target_group: Optional[Group] = None
    if payload.action == "assign_group":
        if not payload.group_id:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail="group_id is required for assign_group action",
            )
        target_group = await session.get(Group, payload.group_id)
        if target_group is None:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail="Group not found",
            )

    notify_command = ""
    if payload.action == "notify":
        if not payload.title or not payload.title.strip() or not payload.message or not payload.message.strip():
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail="title and message are required for notify action",
            )
        notify_command = json.dumps(
            {
                "title": payload.title,
                "message": payload.message,
                "urgent": payload.urgent,
            },
            ensure_ascii=False,
        )

    dev_res = await session.execute(
        select(Device)
        .options(
            selectinload(Device.groups),
            selectinload(Device.installed_software),
            selectinload(Device.pending_updates),
        )
        .where(Device.id.in_(unique_ids))
    )
    devices_by_id = {d.id: d for d in dev_res.scalars().all()}
    devices = [devices_by_id[did] for did in unique_ids if did in devices_by_id]

    signing = request.app.state.signing
    ws_hub = request.app.state.ws_hub
    now = current_utc_time()

    affected = 0
    job_ids: list[str] = []
    ws_job_pushes: list[tuple[str, str]] = []
    ws_catalog_pushes: list[str] = []

    for dev in devices:
        if payload.action == "approve":
            dev.status = "approved"
            created_jobs = await evaluate_deployments_for_device(
                session, dev, signing, settings, ws_hub=None, now=now
            )
            windows = await get_device_maintenance_windows(session, dev)
            for j in created_jobs:
                job_ids.append(j.id)
                if can_dispatch_job_now(j, windows, now):
                    ws_job_pushes.append((dev.id, j.id))
            ws_catalog_pushes.append(dev.id)
            affected += 1

        elif payload.action == "assign_group":
            assert target_group is not None
            if all(g.id != target_group.id for g in dev.groups):
                dev.groups.append(target_group)
                await session.flush()
            if dev.status == "approved":
                created_jobs = await evaluate_deployments_for_device(
                    session, dev, signing, settings, ws_hub=None, now=now
                )
                windows = await get_device_maintenance_windows(session, dev)
                for j in created_jobs:
                    job_ids.append(j.id)
                    if can_dispatch_job_now(j, windows, now):
                        ws_job_pushes.append((dev.id, j.id))
                ws_catalog_pushes.append(dev.id)
            affected += 1

        elif payload.action == "sync":
            if dev.status != "approved":
                continue
            job = create_signed_job(
                device_id=dev.id,
                job_type="remote_control",
                action="force_sync",
                target_id="agent",
                version="",
                sha256="",
                command="",
                respect_maintenance_window=False,
                signing=signing,
                settings=settings,
                now=now,
            )
            session.add(job)
            job_ids.append(job.id)
            ws_job_pushes.append((dev.id, job.id))
            affected += 1

        elif payload.action == "scan_updates":
            if dev.status != "approved":
                continue
            job = create_signed_job(
                device_id=dev.id,
                job_type="scan_updates",
                action="scan",
                target_id="os_updates",
                version="",
                sha256="",
                command="",
                respect_maintenance_window=False,
                signing=signing,
                settings=settings,
                now=now,
            )
            session.add(job)
            job_ids.append(job.id)
            ws_job_pushes.append((dev.id, job.id))
            affected += 1

        elif payload.action == "notify":
            if dev.status != "approved":
                continue
            job = create_signed_job(
                device_id=dev.id,
                job_type="remote_control",
                action="admin_notify",
                target_id="desktop",
                version="",
                sha256="",
                command=notify_command,
                respect_maintenance_window=False,
                signing=signing,
                settings=settings,
                now=now,
            )
            session.add(job)
            job_ids.append(job.id)
            ws_job_pushes.append((dev.id, job.id))
            affected += 1

    session.add(
        AuditLog(
            actor=admin.username,
            action=f"device.bulk.{payload.action}",
            target_type="device_bulk",
            target_id=",".join(unique_ids[:5]),
            details={
                "action": payload.action,
                "requested_count": len(unique_ids),
                "affected": affected,
                "job_ids": job_ids,
                "group_id": payload.group_id,
            },
        )
    )
    await session.commit()

    for dev_id in dict.fromkeys(ws_catalog_pushes):
        await ws_hub.send_to_device(dev_id, {"type": "catalog_updated"})
    for dev_id, job_id in ws_job_pushes:
        await ws_hub.send_to_device(dev_id, {"type": "job_available", "job_id": job_id})

    return BulkDeviceActionResponse(affected=affected, job_ids=job_ids)


@router.get("/devices/{device_id}", response_model=DeviceDetailOut)
async def get_device_detail(
    device_id: str,
    request: Request,
    _admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> DeviceDetailOut:
    device = await session.get(Device, device_id)
    if device is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Device not found"
        )
    ws_hub = request.app.state.ws_hub
    summary = _device_to_summary(device, ws_hub.is_online(device.id))

    pol_res = await session.execute(select(PatchPolicy))
    pol_map = {p.update_id: p for p in pol_res.scalars().all()}

    pending_out = [
        PendingOsUpdateOut(
            update_id=u.update_id,
            title=u.title,
            severity=u.severity,
            category=u.category,
            size_bytes=u.size_bytes,
            reboot_behavior=u.reboot_behavior,
            approved=pol_map[u.update_id].approved if u.update_id in pol_map else False,
            excluded=pol_map[u.update_id].excluded if u.update_id in pol_map else False,
            reported_at=u.reported_at,
        )
        for u in (device.pending_updates or [])
    ]

    software_out = [
        SoftwareEntryOut(
            name=sw.name,
            version=sw.version,
            publisher=sw.publisher,
            source=sw.source,
            scope=sw.scope,
            package_ref=sw.package_ref,
        )
        for sw in sorted(device.installed_software or [], key=lambda s: s.name.lower())
    ]

    jobs_res = await session.execute(
        select(Job)
        .where(Job.device_id == device.id)
        .order_by(desc(Job.created_at))
        .limit(25)
    )
    recent_jobs = [job_to_out(j, device.hostname) for j in jobs_res.scalars().all()]

    return DeviceDetailOut(
        **summary.model_dump(),
        hardware=dict(device.hardware or {}),
        installed_software=software_out,
        pending_os_updates=pending_out,
        recent_jobs=recent_jobs,
    )


@router.post("/devices/{device_id}/approve", response_model=DeviceSummaryOut)
async def approve_device(
    device_id: str,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> DeviceSummaryOut:
    device = await session.get(Device, device_id)
    if device is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Device not found"
        )
    device.status = "approved"
    signing = request.app.state.signing
    ws_hub = request.app.state.ws_hub
    await evaluate_deployments_for_device(
        session, device, signing, settings, ws_hub=ws_hub
    )
    session.add(
        AuditLog(
            actor=admin.username,
            action="device.approve",
            target_type="device",
            target_id=device.id,
            details={"hostname": device.hostname},
        )
    )
    await session.commit()
    reloaded = (
        await session.execute(
            select(Device)
            .options(
                selectinload(Device.groups),
                selectinload(Device.installed_software),
                selectinload(Device.pending_updates),
            )
            .where(Device.id == device.id)
        )
    ).scalar_one()
    return _device_to_summary(reloaded, ws_hub.is_online(reloaded.id))


@router.post("/devices/{device_id}/revoke", response_model=DeviceSummaryOut)
async def revoke_device(
    device_id: str,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> DeviceSummaryOut:
    device = await session.get(Device, device_id)
    if device is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Device not found"
        )
    device.status = "revoked"
    device.is_online = False
    ws_hub = request.app.state.ws_hub
    session.add(
        AuditLog(
            actor=admin.username,
            action="device.revoke",
            target_type="device",
            target_id=device.id,
            details={"hostname": device.hostname},
        )
    )
    await session.commit()
    reloaded = (
        await session.execute(
            select(Device)
            .options(
                selectinload(Device.groups),
                selectinload(Device.installed_software),
                selectinload(Device.pending_updates),
            )
            .where(Device.id == device.id)
        )
    ).scalar_one()
    await ws_hub.revoke_device(reloaded.id)
    return _device_to_summary(reloaded, False)


@router.patch("/devices/{device_id}", response_model=DeviceSummaryOut)
async def patch_device(
    device_id: str,
    payload: DevicePatchRequest,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> DeviceSummaryOut:
    device = await session.get(Device, device_id)
    if device is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Device not found"
        )
    ws_hub = request.app.state.ws_hub
    signing = request.app.state.signing

    if payload.group_ids is not None:
        if payload.group_ids:
            grp_res = await session.execute(
                select(Group).where(Group.id.in_(payload.group_ids))
            )
            groups = list(grp_res.scalars().all())
            if len(groups) != len(set(payload.group_ids)):
                raise HTTPException(
                    status_code=status.HTTP_404_NOT_FOUND,
                    detail="One or more group_ids not found",
                )
            device.groups = groups
        else:
            device.groups = []

    if payload.tags is not None:
        device.tags = [t.strip() for t in payload.tags if t.strip()]

    await session.flush()
    if device.status == "approved":
        await evaluate_deployments_for_device(
            session, device, signing, settings, ws_hub=ws_hub
        )
        await ws_hub.send_to_device(device.id, {"type": "catalog_updated"})

    session.add(
        AuditLog(
            actor=admin.username,
            action="device.update",
            target_type="device",
            target_id=device.id,
            details=payload.model_dump(exclude_none=True),
        )
    )
    await session.commit()
    reloaded = (
        await session.execute(
            select(Device)
            .options(
                selectinload(Device.groups),
                selectinload(Device.installed_software),
                selectinload(Device.pending_updates),
            )
            .where(Device.id == device.id)
        )
    ).scalar_one()
    return _device_to_summary(reloaded, ws_hub.is_online(reloaded.id))


@router.delete("/devices/{device_id}")
async def delete_device(
    device_id: str,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> dict[str, str]:
    device = await session.get(Device, device_id)
    if device is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Device not found"
        )
    hostname = device.hostname
    ws_hub = request.app.state.ws_hub
    await ws_hub.revoke_device(device.id)
    await session.delete(device)
    session.add(
        AuditLog(
            actor=admin.username,
            action="device.delete",
            target_type="device",
            target_id=device_id,
            details={"hostname": hostname},
        )
    )
    await session.commit()
    return {"status": "deleted", "id": device_id}


@router.post("/devices/{device_id}/scan-updates", response_model=JobOut)
async def trigger_device_scan_updates(
    device_id: str,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> JobOut:
    device = await session.get(Device, device_id)
    if device is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Device not found"
        )
    if device.status != "approved":
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Cannot scan updates on an unapproved or revoked device",
        )
    signing = request.app.state.signing
    ws_hub = request.app.state.ws_hub
    job = create_signed_job(
        device_id=device.id,
        job_type="scan_updates",
        action="scan",
        target_id="os_updates",
        version="",
        sha256="",
        command="",
        respect_maintenance_window=False,
        signing=signing,
        settings=settings,
    )
    session.add(job)
    session.add(
        AuditLog(
            actor=admin.username,
            action="device.scan_updates",
            target_type="device",
            target_id=device.id,
            details={"job_id": job.id},
        )
    )
    await session.commit()
    await session.refresh(job)
    await ws_hub.send_to_device(device.id, {"type": "job_available", "job_id": job.id})
    return job_to_out(job, device.hostname)


@router.post("/devices/{device_id}/sync", response_model=JobOut)
async def trigger_device_sync(
    device_id: str,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> JobOut:
    device = await session.get(Device, device_id)
    if device is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Device not found"
        )
    if device.status != "approved":
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Cannot sync an unapproved or revoked device",
        )
    signing = request.app.state.signing
    ws_hub = request.app.state.ws_hub
    job = create_signed_job(
        device_id=device.id,
        job_type="remote_control",
        action="force_sync",
        target_id="agent",
        version="",
        sha256="",
        command="",
        respect_maintenance_window=False,
        signing=signing,
        settings=settings,
    )
    session.add(job)
    session.add(
        AuditLog(
            actor=admin.username,
            action="device.sync",
            target_type="device",
            target_id=device.id,
            details={"job_id": job.id, "hostname": device.hostname},
        )
    )
    await session.commit()
    await session.refresh(job)
    await ws_hub.send_to_device(device.id, {"type": "job_available", "job_id": job.id})
    return job_to_out(job, device.hostname)


@router.post("/devices/{device_id}/notify", response_model=JobOut)
async def trigger_device_notify(
    device_id: str,
    payload: DeviceNotifyRequest,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> JobOut:
    device = await session.get(Device, device_id)
    if device is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Device not found"
        )
    if device.status != "approved":
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Cannot notify an unapproved or revoked device",
        )
    signing = request.app.state.signing
    ws_hub = request.app.state.ws_hub
    command_json = json.dumps(
        {
            "title": payload.title,
            "message": payload.message,
            "urgent": payload.urgent,
        },
        ensure_ascii=False,
    )
    job = create_signed_job(
        device_id=device.id,
        job_type="remote_control",
        action="admin_notify",
        target_id="desktop",
        version="",
        sha256="",
        command=command_json,
        respect_maintenance_window=False,
        signing=signing,
        settings=settings,
    )
    session.add(job)
    session.add(
        AuditLog(
            actor=admin.username,
            action="device.notify",
            target_type="device",
            target_id=device.id,
            details={
                "job_id": job.id,
                "hostname": device.hostname,
                "title": payload.title,
                "urgent": payload.urgent,
            },
        )
    )
    await session.commit()
    await session.refresh(job)
    await ws_hub.send_to_device(device.id, {"type": "job_available", "job_id": job.id})
    return job_to_out(job, device.hostname)


@router.post("/devices/{device_id}/reboot", response_model=JobOut)
async def trigger_device_reboot(
    device_id: str,
    payload: DeviceRemoteRebootRequest,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> JobOut:
    device = await session.get(Device, device_id)
    if device is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Device not found"
        )
    if device.status != "approved":
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Cannot reboot an unapproved or revoked device",
        )
    signing = request.app.state.signing
    ws_hub = request.app.state.ws_hub
    command_json = json.dumps(
        {
            "mode": payload.mode,
            "grace_minutes": payload.grace_minutes,
        }
    )
    job = create_signed_job(
        device_id=device.id,
        job_type="remote_control",
        action="remote_reboot",
        target_id=payload.mode,
        version="",
        sha256="",
        command=command_json,
        respect_maintenance_window=False,
        signing=signing,
        settings=settings,
    )
    session.add(job)
    session.add(
        AuditLog(
            actor=admin.username,
            action="device.reboot",
            target_type="device",
            target_id=device.id,
            details={
                "job_id": job.id,
                "hostname": device.hostname,
                "mode": payload.mode,
                "grace_minutes": payload.grace_minutes,
            },
        )
    )
    await session.commit()
    await session.refresh(job)
    await ws_hub.send_to_device(device.id, {"type": "job_available", "job_id": job.id})
    return job_to_out(job, device.hostname)


# --- Groups ---


def _group_to_out(group: Group, device_count: int = 0) -> GroupOut:
    return GroupOut(
        id=group.id,
        name=group.name,
        description=group.description,
        reboot_policy=group.reboot_policy,
        grace_period_minutes=group.grace_period_minutes,
        enforce_tray=bool(group.enforce_tray),
        allow_self_service=bool(group.allow_self_service),
        allow_user_os_updates=bool(group.allow_user_os_updates),
        silent_notifications=bool(group.silent_notifications),
        device_count=device_count,
        created_at=group.created_at,
    )


async def _group_device_counts(session: AsyncSession) -> dict[str, int]:
    stmt = select(
        device_group_links.c.group_id, func.count(device_group_links.c.device_id)
    ).group_by(device_group_links.c.group_id)
    res = await session.execute(stmt)
    return {gid: int(cnt) for gid, cnt in res.all()}


@router.get("/groups", response_model=list[GroupOut])
async def list_groups(
    _admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> list[GroupOut]:
    counts = await _group_device_counts(session)
    res = await session.execute(select(Group).order_by(Group.name))
    return [_group_to_out(g, counts.get(g.id, 0)) for g in res.scalars().all()]


@router.post("/groups", response_model=GroupOut)
async def create_group(
    payload: GroupCreate,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> GroupOut:
    existing = await session.execute(select(Group).where(Group.name == payload.name))
    if existing.scalar_one_or_none() is not None:
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail=f"Group '{payload.name}' already exists",
        )
    group = Group(
        name=payload.name,
        description=payload.description,
        reboot_policy=payload.reboot_policy,
        grace_period_minutes=payload.grace_period_minutes,
        enforce_tray=payload.enforce_tray,
        allow_self_service=payload.allow_self_service,
        allow_user_os_updates=payload.allow_user_os_updates,
        silent_notifications=payload.silent_notifications,
    )
    session.add(group)
    session.add(
        AuditLog(
            actor=admin.username,
            action="group.create",
            target_type="group",
            target_id=group.id,
            details=payload.model_dump(),
        )
    )
    await session.commit()
    await session.refresh(group)
    return _group_to_out(group, 0)


@router.patch("/groups/{group_id}", response_model=GroupOut)
async def update_group(
    group_id: str,
    payload: GroupUpdate,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> GroupOut:
    group = await session.get(Group, group_id)
    if group is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Group not found"
        )
    if payload.name is not None and payload.name != group.name:
        dup = await session.execute(select(Group).where(Group.name == payload.name))
        if dup.scalar_one_or_none() is not None:
            raise HTTPException(
                status_code=status.HTTP_409_CONFLICT,
                detail=f"Group '{payload.name}' already exists",
            )
        group.name = payload.name
    if payload.description is not None:
        group.description = payload.description
    if payload.reboot_policy is not None:
        group.reboot_policy = payload.reboot_policy
    if payload.grace_period_minutes is not None:
        group.grace_period_minutes = payload.grace_period_minutes
    if payload.enforce_tray is not None:
        group.enforce_tray = payload.enforce_tray
    if payload.allow_self_service is not None:
        group.allow_self_service = payload.allow_self_service
    if payload.allow_user_os_updates is not None:
        group.allow_user_os_updates = payload.allow_user_os_updates
    if payload.silent_notifications is not None:
        group.silent_notifications = payload.silent_notifications

    member_res = await session.execute(
        select(device_group_links.c.device_id).where(
            device_group_links.c.group_id == group.id
        )
    )
    member_device_ids = [row[0] for row in member_res.all()]

    session.add(
        AuditLog(
            actor=admin.username,
            action="group.update",
            target_type="group",
            target_id=group.id,
            details=payload.model_dump(exclude_none=True),
        )
    )
    await session.commit()
    await session.refresh(group)
    counts = await _group_device_counts(session)

    ws_hub = request.app.state.ws_hub
    for dev_id in member_device_ids:
        await ws_hub.send_to_device(dev_id, {"type": "catalog_updated"})

    return _group_to_out(group, counts.get(group.id, 0))


@router.delete("/groups/{group_id}")
async def delete_group(
    group_id: str,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> dict[str, str]:
    group = await session.get(Group, group_id)
    if group is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Group not found"
        )
    gname = group.name
    await session.delete(group)
    session.add(
        AuditLog(
            actor=admin.username,
            action="group.delete",
            target_type="group",
            target_id=group_id,
            details={"name": gname},
        )
    )
    await session.commit()
    return {"status": "deleted", "id": group_id}


# --- Maintenance Windows ---


@router.get("/maintenance-windows", response_model=list[MaintenanceWindowOut])
async def list_maintenance_windows(
    group_id: Optional[str] = Query(default=None),
    _admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> list[MaintenanceWindowOut]:
    grp_res = await session.execute(select(Group))
    grp_names = {g.id: g.name for g in grp_res.scalars().all()}

    stmt = select(MaintenanceWindow).order_by(desc(MaintenanceWindow.created_at))
    if group_id:
        stmt = stmt.where(MaintenanceWindow.group_id == group_id)
    res = await session.execute(stmt)
    return [
        MaintenanceWindowOut(
            id=mw.id,
            group_id=mw.group_id,
            group_name=grp_names.get(mw.group_id, ""),
            name=mw.name,
            days_of_week=list(mw.days_of_week or []),
            start_time=mw.start_time,
            end_time=mw.end_time,
            timezone=mw.timezone,
            catch_up=mw.catch_up,
            created_at=mw.created_at,
        )
        for mw in res.scalars().all()
    ]


@router.post("/maintenance-windows", response_model=MaintenanceWindowOut)
async def create_maintenance_window(
    payload: MaintenanceWindowCreate,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> MaintenanceWindowOut:
    grp = await session.get(Group, payload.group_id)
    if grp is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Group not found"
        )
    mw = MaintenanceWindow(
        group_id=payload.group_id,
        name=payload.name,
        days_of_week=payload.days_of_week,
        start_time=payload.start_time,
        end_time=payload.end_time,
        timezone=payload.timezone,
        catch_up=payload.catch_up,
    )
    session.add(mw)
    session.add(
        AuditLog(
            actor=admin.username,
            action="maintenance_window.create",
            target_type="maintenance_window",
            target_id=mw.id,
            details=payload.model_dump(),
        )
    )
    await session.commit()
    await session.refresh(mw)
    return MaintenanceWindowOut(
        id=mw.id,
        group_id=mw.group_id,
        group_name=grp.name,
        name=mw.name,
        days_of_week=list(mw.days_of_week or []),
        start_time=mw.start_time,
        end_time=mw.end_time,
        timezone=mw.timezone,
        catch_up=mw.catch_up,
        created_at=mw.created_at,
    )


@router.delete("/maintenance-windows/{window_id}")
async def delete_maintenance_window(
    window_id: str,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> dict[str, str]:
    mw = await session.get(MaintenanceWindow, window_id)
    if mw is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Maintenance window not found",
        )
    await session.delete(mw)
    session.add(
        AuditLog(
            actor=admin.username,
            action="maintenance_window.delete",
            target_type="maintenance_window",
            target_id=window_id,
            details={"name": mw.name},
        )
    )
    await session.commit()
    return {"status": "deleted", "id": window_id}
