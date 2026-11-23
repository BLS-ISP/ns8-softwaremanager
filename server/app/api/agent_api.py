from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Optional

from fastapi import (
    APIRouter,
    Depends,
    HTTPException,
    Query,
    Request,
    WebSocket,
    WebSocketDisconnect,
    status,
)
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.auth import (
    generate_device_token,
    get_approved_device,
    get_db,
    get_non_revoked_device,
    get_settings_dep,
    hash_token,
)
from app.config import Settings
from app.models import (
    CatalogItem,
    Deployment,
    Device,
    DevicePendingUpdate,
    EnrollmentToken,
    Group,
    InstalledSoftware,
    Job,
    PatchPolicy,
    ensure_utc,
    utc_now,
)
from app.schemas import (
    AgentEnrollRequest,
    AgentEnrollResponse,
    AgentInventoryRequest,
    AgentPendingJobsResponse,
    AgentStatusResponse,
    JobResultIn,
    SelfServiceTriggerRequest,
    SignedJobOut,
)
from app.services import (
    ACTIVE_JOB_STATUSES,
    WebSocketHub,
    build_available_catalog_for_device,
    build_range_file_response,
    build_signed_job_dto,
    create_signed_job,
    current_utc_time,
    device_matches_deployment,
    evaluate_deployments_for_device,
    get_device_maintenance_windows,
    get_dispatchable_jobs_for_device,
    is_device_in_maintenance_window,
    resolve_device_desktop_policy,
    resolve_device_reboot_policy,
)

router = APIRouter(prefix="/api/v1/agent", tags=["agent"])


@router.post("/enroll", response_model=AgentEnrollResponse)
async def agent_enroll(
    payload: AgentEnrollRequest,
    request: Request,
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> AgentEnrollResponse:
    tok_hash = hash_token(payload.enrollment_token)
    res = await session.execute(
        select(EnrollmentToken).where(EnrollmentToken.token_hash == tok_hash)
    )
    tok = res.scalar_one_or_none()
    now = utc_now()

    if tok is None or tok.revoked:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid or revoked enrollment token",
        )
    expires_at = ensure_utc(tok.expires_at)
    if expires_at is not None and expires_at <= now:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Enrollment token has expired",
        )
    if tok.max_uses is not None and tok.uses_count >= tok.max_uses:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Enrollment token usage limit reached",
        )

    tok.uses_count += 1
    raw_device_token = generate_device_token()
    dev_token_hash = hash_token(raw_device_token)
    initial_status = "approved" if tok.auto_approve else "pending"

    existing_res = await session.execute(
        select(Device).where(Device.machine_uid == payload.machine_uid)
    )
    device = existing_res.scalar_one_or_none()

    if device is None:
        initial_groups: list[Group] = []
        if tok.group_id:
            grp = await session.get(Group, tok.group_id)
            if grp is not None:
                initial_groups.append(grp)
        device = Device(
            hostname=payload.hostname,
            os_family=payload.os_family,
            os_name=payload.os_name,
            os_version=payload.os_version,
            arch=payload.arch,
            agent_version=payload.agent_version,
            machine_uid=payload.machine_uid,
            token_hash=dev_token_hash,
            status=initial_status,
            last_seen_at=now,
            created_at=now,
            groups=initial_groups,
            installed_software=[],
            pending_updates=[],
        )
        session.add(device)
        await session.flush()
    else:
        device.hostname = payload.hostname
        device.os_family = payload.os_family
        device.os_name = payload.os_name
        device.os_version = payload.os_version
        device.arch = payload.arch
        device.agent_version = payload.agent_version
        device.token_hash = dev_token_hash
        device.status = initial_status
        device.last_seen_at = now
        if tok.group_id:
            grp = await session.get(Group, tok.group_id)
            if grp is not None and all(g.id != grp.id for g in device.groups):
                device.groups.append(grp)
        await session.flush()

    signing = request.app.state.signing
    ws_hub = request.app.state.ws_hub
    if device.status == "approved":
        await evaluate_deployments_for_device(
            session, device, signing, settings, ws_hub=ws_hub
        )

    await session.commit()

    return AgentEnrollResponse(
        device_id=device.id,
        device_token=raw_device_token,
        status=device.status,
        server_public_key_b64=signing.public_key_b64(),
    )


@router.get("/status", response_model=AgentStatusResponse)
async def agent_status(
    request: Request,
    device: Device = Depends(get_non_revoked_device),
    session: AsyncSession = Depends(get_db),
) -> AgentStatusResponse:
    device.last_seen_at = utc_now()
    reboot_policy, grace_minutes = resolve_device_reboot_policy(device)
    desktop_policy = resolve_device_desktop_policy(device)
    windows = await get_device_maintenance_windows(session, device)
    in_mw = is_device_in_maintenance_window(windows, current_utc_time())

    excl_res = await session.execute(
        select(PatchPolicy.update_id).where(PatchPolicy.excluded.is_(True))
    )
    excluded_updates = sorted(excl_res.scalars().all())
    await session.commit()

    signing = request.app.state.signing
    return AgentStatusResponse(
        device_id=device.id,
        status=device.status,
        reboot_policy=reboot_policy,
        grace_period_minutes=grace_minutes,
        in_maintenance_window=in_mw,
        excluded_updates=excluded_updates,
        server_public_key_b64=signing.public_key_b64(),
        desktop_policy=desktop_policy,
    )


@router.post("/inventory")
async def submit_agent_inventory(
    payload: AgentInventoryRequest,
    request: Request,
    device: Device = Depends(get_approved_device),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> dict[str, Any]:
    now = utc_now()
    device.last_seen_at = now
    if payload.hostname:
        device.hostname = payload.hostname
    if payload.domain is not None:
        device.domain = payload.domain
    if payload.os_family:
        norm_os = payload.os_family.strip().lower()
        if norm_os in ("windows", "linux"):
            device.os_family = norm_os
    if payload.os_name is not None:
        device.os_name = payload.os_name
    if payload.os_version is not None:
        device.os_version = payload.os_version
    if payload.kernel_or_build is not None:
        device.kernel_or_build = payload.kernel_or_build
    if payload.arch:
        device.arch = payload.arch
    if payload.agent_version:
        device.agent_version = payload.agent_version
    if payload.hardware:
        device.hardware = payload.hardware
    device.ip_addresses = list(payload.ip_addresses or [])
    device.logged_in_users = list(payload.logged_in_users or [])
    device.reboot_required = payload.reboot_required
    device.center_running = bool(payload.center_running)

    software_updated = False
    if payload.installed_software is not None:
        computed_hash = payload.software_hash or hashlib.sha256(
            json.dumps(
                sorted(
                    (s.name, s.version, s.scope, s.package_ref)
                    for s in payload.installed_software
                )
            ).encode("utf-8")
        ).hexdigest()

        os_reported_count = sum(
            1 for s in device.installed_software if s.source != "custom"
        )
        if (
            device.software_hash != computed_hash
            or os_reported_count != len(payload.installed_software)
        ):
            incoming_names = {
                s.name.strip().lower() for s in payload.installed_software
            }
            incoming_refs = {
                s.package_ref.strip().lower()
                for s in payload.installed_software
                if s.package_ref.strip()
            }
            preserved_custom = [
                (
                    s.name,
                    s.version,
                    s.publisher,
                    s.source,
                    s.scope,
                    s.package_ref,
                )
                for s in device.installed_software
                if s.source == "custom"
                and s.name.strip().lower() not in incoming_names
                and (
                    not s.package_ref.strip()
                    or s.package_ref.strip().lower() not in incoming_refs
                )
            ]

            device.installed_software.clear()
            await session.flush()
            for sw in payload.installed_software:
                device.installed_software.append(
                    InstalledSoftware(
                        device_id=device.id,
                        name=sw.name,
                        version=sw.version,
                        publisher=sw.publisher,
                        source=sw.source,
                        scope=sw.scope,
                        package_ref=sw.package_ref,
                    )
                )
            for c_name, c_ver, c_pub, c_src, c_scope, c_ref in preserved_custom:
                device.installed_software.append(
                    InstalledSoftware(
                        device_id=device.id,
                        name=c_name,
                        version=c_ver,
                        publisher=c_pub,
                        source=c_src,
                        scope=c_scope,
                        package_ref=c_ref,
                    )
                )
            device.software_hash = computed_hash
            software_updated = True

    signing = request.app.state.signing
    ws_hub = request.app.state.ws_hub
    auto_patch_jobs = 0

    if payload.pending_os_updates is not None:
        device.pending_updates.clear()
        await session.flush()
        for upd in payload.pending_os_updates:
            device.pending_updates.append(
                DevicePendingUpdate(
                    device_id=device.id,
                    update_id=upd.update_id,
                    title=upd.title,
                    severity=upd.severity,
                    category=upd.category,
                    size_bytes=upd.size_bytes,
                    reboot_behavior=upd.reboot_behavior,
                    reported_at=now,
                )
            )
        await session.flush()

        # Auto-enqueue previously approved non-excluded OS patches for this device
        reported_ids = [u.update_id for u in payload.pending_os_updates]
        if reported_ids:
            pol_res = await session.execute(
                select(PatchPolicy).where(
                    PatchPolicy.update_id.in_(reported_ids),
                    PatchPolicy.approved.is_(True),
                    PatchPolicy.excluded.is_(False),
                )
            )
            dev_group_ids = {g.id for g in device.groups}
            eligible_pols = [
                p
                for p in pol_res.scalars().all()
                if not p.approved_group_ids
                or dev_group_ids.intersection(p.approved_group_ids)
            ]
            if eligible_pols:
                active_res = await session.execute(
                    select(Job).where(
                        Job.device_id == device.id,
                        Job.job_type == "os_patch",
                        Job.status.in_(ACTIVE_JOB_STATUSES),
                    )
                )
                already_queued = set()
                for aj in active_res.scalars().all():
                    already_queued.update(aj.update_ids or [])
                new_uids = sorted(
                    {p.update_id for p in eligible_pols} - already_queued
                )
                if new_uids:
                    respect_mw = any(
                        p.respect_maintenance_window for p in eligible_pols
                    )
                    pjob = create_signed_job(
                        device_id=device.id,
                        job_type="os_patch",
                        action="install",
                        target_id=",".join(new_uids),
                        update_ids=new_uids,
                        respect_maintenance_window=respect_mw,
                        signing=signing,
                        settings=settings,
                    )
                    session.add(pjob)
                    auto_patch_jobs += 1

    await session.flush()
    created_sw_jobs = await evaluate_deployments_for_device(
        session, device, signing, settings, ws_hub=ws_hub
    )
    await session.commit()
    return {
        "status": "ok",
        "enqueued_jobs": len(created_sw_jobs) + auto_patch_jobs,
        "software_updated": software_updated,
    }


@router.get("/jobs/pending", response_model=AgentPendingJobsResponse)
async def get_pending_agent_jobs(
    request: Request,
    device: Device = Depends(get_approved_device),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> AgentPendingJobsResponse:
    now = current_utc_time()
    device.last_seen_at = now
    signing = request.app.state.signing

    ready_jobs = await get_dispatchable_jobs_for_device(
        session, device, include_dispatched=True, now=now
    )

    pkg_ids = list({j.target_id for j in ready_jobs if j.job_type == "software"})
    catalog_map: dict[str, CatalogItem] = {}
    if pkg_ids:
        pkg_res = await session.execute(
            select(CatalogItem).where(CatalogItem.id.in_(pkg_ids))
        )
        catalog_map = {p.id: p for p in pkg_res.scalars().all()}

    signed_jobs: list[SignedJobOut] = []
    for job in ready_jobs:
        if job.status == "queued":
            job.status = "dispatched"
            job.dispatched_at = now
        signed_jobs.append(
            build_signed_job_dto(job, catalog_map, signing, settings, now=now)
        )

    available_catalog = await build_available_catalog_for_device(
        session, device, signing
    )
    await session.commit()
    return AgentPendingJobsResponse(
        jobs=signed_jobs,
        available_catalog=available_catalog,
    )


@router.post("/jobs/{job_id}/result")
async def report_job_result(
    job_id: str,
    payload: JobResultIn,
    device: Device = Depends(get_approved_device),
    session: AsyncSession = Depends(get_db),
) -> dict[str, str]:
    job = await session.get(Job, job_id)
    if job is None or job.device_id != device.id:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Job not found for this device",
        )

    now = utc_now()
    device.last_seen_at = now
    raw_status = payload.status.strip().lower()
    if raw_status in ("succeeded", "success", "completed", "ok"):
        norm_status = "succeeded"
    elif raw_status in ("running", "in_progress"):
        norm_status = "running"
    elif raw_status == "waiting_for_process":
        norm_status = "waiting_for_process"
    else:
        norm_status = "failed"

    job.status = norm_status
    job.exit_code = payload.exit_code
    reboot_flag = payload.reboot_required or (
        payload.exit_code in (3010, 1641)
    )
    job.reboot_required = reboot_flag
    if reboot_flag:
        device.reboot_required = True

    if payload.stdout:
        job.stdout = payload.stdout
    if payload.stderr:
        job.stderr = payload.stderr

    if job.started_at is None:
        job.started_at = now
    if norm_status in ("succeeded", "failed"):
        job.completed_at = now

    # Reflect completed OS patches or software changes immediately
    if norm_status == "succeeded":
        if job.job_type == "os_patch" and job.update_ids:
            done_set = set(job.update_ids)
            device.pending_updates = [
                u for u in (device.pending_updates or []) if u.update_id not in done_set
            ]
        elif job.job_type == "software":
            pkg = await session.get(CatalogItem, job.target_id)
            if pkg is not None:
                if job.action in ("install", "update"):
                    matched = None
                    for sw in device.installed_software:
                        if (
                            pkg.package_ref
                            and sw.package_ref.lower() == pkg.package_ref.lower()
                        ) or sw.name.lower() == pkg.name.lower():
                            matched = sw
                            break
                    if matched is not None:
                        matched.version = pkg.version
                    else:
                        device.installed_software.append(
                            InstalledSoftware(
                                device_id=device.id,
                                name=pkg.name,
                                version=pkg.version,
                                publisher=pkg.vendor,
                                source=pkg.source_type,
                                scope="machine",
                                package_ref=pkg.package_ref,
                            )
                        )
                elif job.action == "uninstall":
                    device.installed_software = [
                        sw
                        for sw in device.installed_software
                        if not (
                            (
                                pkg.package_ref
                                and sw.package_ref.lower() == pkg.package_ref.lower()
                            )
                            or sw.name.lower() == pkg.name.lower()
                        )
                    ]

    await session.commit()
    return {"status": "ok", "job_id": job.id, "job_status": job.status}


@router.post("/self-service/trigger", response_model=SignedJobOut)
async def trigger_self_service_package(
    payload: SelfServiceTriggerRequest,
    request: Request,
    device: Device = Depends(get_approved_device),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> SignedJobOut:
    pkg = await session.get(CatalogItem, payload.package_id)
    if pkg is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Package not found",
        )
    if pkg.os_family != device.os_family:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Package OS family does not match device OS family",
        )

    dep_res = await session.execute(
        select(Deployment).where(Deployment.package_id == pkg.id)
    )
    matching_deps = [
        d for d in dep_res.scalars().all() if device_matches_deployment(device, d)
    ]
    allowed_modes = (
        {"available", "required"} if payload.action in ("install", "update") else {"available"}
    )
    if not any(d.mode in allowed_modes for d in matching_deps):
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Package is not assigned as self-service available for this device",
        )

    if payload.action == "uninstall" and not pkg.allow_uninstall:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Uninstallation is not permitted for this package",
        )

    signing = request.app.state.signing
    ws_hub = request.app.state.ws_hub
    now = current_utc_time()
    cmd = pkg.uninstall_args if payload.action == "uninstall" else pkg.install_args
    dl_url = (
        f"/api/v1/agent/packages/{pkg.id}/download"
        if payload.action in ("install", "update")
        and (pkg.source_type == "custom" or pkg.storage_path)
        else None
    )

    job = create_signed_job(
        device_id=device.id,
        job_type="software",
        action=payload.action,
        target_id=pkg.id,
        version=pkg.version,
        sha256=pkg.sha256 or "",
        command=cmd or "",
        download_url=dl_url,
        respect_maintenance_window=False,
        initial_status="dispatched",
        signing=signing,
        settings=settings,
        now=now,
    )
    session.add(job)
    await session.commit()
    await session.refresh(job)
    await ws_hub.send_to_device(device.id, {"type": "job_available", "job_id": job.id})

    return build_signed_job_dto(job, {pkg.id: pkg}, signing, settings, now=now)


@router.get("/packages/{package_id}/download")
async def download_package_binary(
    package_id: str,
    request: Request,
    _device: Device = Depends(get_approved_device),
    session: AsyncSession = Depends(get_db),
):
    pkg = await session.get(CatalogItem, package_id)
    if pkg is None or not pkg.storage_path:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Package binary not found",
        )
    return build_range_file_response(
        path=Path(pkg.storage_path),
        filename=pkg.filename or f"{pkg.id}.bin",
        sha256=pkg.sha256,
        range_header=request.headers.get("range"),
    )


@router.websocket("/ws")
async def agent_websocket(
    websocket: WebSocket,
    token: Optional[str] = Query(default=None),
) -> None:
    raw_token = token
    if not raw_token:
        auth_header = websocket.headers.get("authorization", "")
        if auth_header.lower().startswith("bearer "):
            raw_token = auth_header[7:].strip()

    if not raw_token:
        await websocket.close(code=1008)
        return

    session_factory = websocket.app.state.db.session_factory
    ws_hub: WebSocketHub = websocket.app.state.ws_hub
    th = hash_token(raw_token)

    async with session_factory() as session:
        res = await session.execute(select(Device).where(Device.token_hash == th))
        device = res.scalar_one_or_none()
        if device is None or device.status != "approved":
            await websocket.close(code=1008)
            return
        device_id = device.id
        device.is_online = True
        device.last_seen_at = utc_now()
        await session.commit()

    await websocket.accept()
    ws_hub.register(device_id, websocket)

    try:
        while True:
            msg = await websocket.receive_json()
            if not isinstance(msg, dict):
                continue
            msg_type = msg.get("type")

            if msg_type == "heartbeat":
                async with session_factory() as session:
                    dev = await session.get(Device, device_id)
                    if dev is None or dev.status != "approved":
                        await websocket.send_json({"type": "revoked"})
                        await websocket.close(code=1008)
                        return
                    dev.last_seen_at = utc_now()
                    dev.is_online = True
                    if "agent_version" in msg and msg["agent_version"]:
                        dev.agent_version = str(msg["agent_version"])
                    if "reboot_required" in msg and isinstance(
                        msg["reboot_required"], bool
                    ):
                        dev.reboot_required = msg["reboot_required"]
                    await session.commit()
                await websocket.send_json(
                    {"type": "heartbeat_ack", "server_time": utc_now().isoformat()}
                )

            elif msg_type == "log_chunk":
                job_id = msg.get("job_id")
                chunk = str(msg.get("chunk", ""))
                stream = str(msg.get("stream", "stdout")).lower()
                if job_id and chunk:
                    async with session_factory() as session:
                        job = await session.get(Job, str(job_id))
                        if (
                            job is not None
                            and job.device_id == device_id
                            and job.status not in ("succeeded", "failed")
                        ):
                            if job.status in ("queued", "dispatched"):
                                job.status = "running"
                                job.started_at = job.started_at or utc_now()
                            if stream == "stderr":
                                job.stderr = (job.stderr or "") + chunk
                            else:
                                job.stdout = (job.stdout or "") + chunk
                            await session.commit()

    except WebSocketDisconnect:
        pass
    except Exception:
        pass
    finally:
        still_connected = ws_hub.unregister(device_id, websocket)
        if not still_connected:
            import anyio

            with anyio.CancelScope(shield=True):
                async with session_factory() as session:
                    dev = await session.get(Device, device_id)
                    if dev is not None:
                        dev.is_online = False
                        dev.last_seen_at = utc_now()
                        await session.commit()
