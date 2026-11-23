from __future__ import annotations

from typing import Any, Optional

from fastapi import (
    APIRouter,
    Depends,
    File,
    Form,
    HTTPException,
    Query,
    Request,
    UploadFile,
    status,
)
from sqlalchemy import desc, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.admin_auth import job_to_out
from app.auth import get_current_admin, get_db, get_settings_dep
from app.config import Settings
from app.models import AdminUser, AuditLog, CatalogItem, Deployment, Device, Job, utc_now
from app.schemas import (
    CatalogItemCreate,
    CatalogItemOut,
    CatalogItemUpdate,
    ChunkUploadResponse,
    DeploymentCreate,
    DeploymentOut,
    DeploymentStatsOut,
    JobOut,
    SelfUpdateRequest,
)
from app.services import (
    ACTIVE_JOB_STATUSES,
    PackageStorageService,
    can_dispatch_job_now,
    compare_versions,
    create_signed_job,
    current_utc_time,
    device_matches_deployment,
    evaluate_deployments_for_device,
    find_installed_package_version,
    get_device_maintenance_windows,
)

router = APIRouter(prefix="/api/v1/admin", tags=["admin-catalog-deployments"])


def _catalog_to_out(item: CatalogItem) -> CatalogItemOut:
    return CatalogItemOut(
        id=item.id,
        name=item.name,
        version=item.version,
        vendor=item.vendor,
        description=item.description,
        category=item.category,
        icon_url=item.icon_url,
        os_family=item.os_family,
        source_type=item.source_type,
        package_ref=item.package_ref,
        install_args=item.install_args,
        uninstall_args=item.uninstall_args,
        blocked_processes=list(item.blocked_processes or []),
        allow_uninstall=item.allow_uninstall,
        return_codes={str(k): str(v) for k, v in (item.return_codes or {}).items()},
        detection_rule=dict(item.detection_rule or {}),
        filename=item.filename,
        file_size=item.file_size,
        sha256=item.sha256,
        created_at=item.created_at,
        updated_at=item.updated_at,
    )


@router.get("/catalog", response_model=list[CatalogItemOut])
async def list_catalog(
    os_family: Optional[str] = Query(default=None),
    source_type: Optional[str] = Query(default=None),
    category: Optional[str] = Query(default=None),
    search: Optional[str] = Query(default=None),
    _admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> list[CatalogItemOut]:
    stmt = select(CatalogItem).order_by(CatalogItem.name)
    if os_family:
        stmt = stmt.where(CatalogItem.os_family == os_family.lower())
    if source_type:
        stmt = stmt.where(CatalogItem.source_type == source_type.lower())
    if category:
        stmt = stmt.where(CatalogItem.category == category)

    res = await session.execute(stmt)
    items = list(res.scalars().all())
    if search:
        q = search.strip().lower()
        items = [
            i
            for i in items
            if q in i.name.lower()
            or q in i.vendor.lower()
            or q in i.package_ref.lower()
            or q in i.description.lower()
        ]
    return [_catalog_to_out(i) for i in items]


@router.post("/catalog", response_model=CatalogItemOut)
async def create_catalog_item(
    payload: CatalogItemCreate,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> CatalogItemOut:
    now = utc_now()
    item = CatalogItem(
        name=payload.name,
        version=payload.version,
        vendor=payload.vendor,
        description=payload.description,
        category=payload.category,
        icon_url=payload.icon_url,
        os_family=payload.os_family,
        source_type=payload.source_type,
        package_ref=payload.package_ref,
        install_args=payload.install_args,
        uninstall_args=payload.uninstall_args,
        blocked_processes=payload.blocked_processes,
        allow_uninstall=payload.allow_uninstall,
        return_codes=payload.return_codes,
        detection_rule=payload.detection_rule,
        created_at=now,
        updated_at=now,
    )
    session.add(item)
    session.add(
        AuditLog(
            actor=admin.username,
            action="catalog.create",
            target_type="catalog_item",
            target_id=item.id,
            details={"name": item.name, "source_type": item.source_type},
        )
    )
    await session.commit()
    await session.refresh(item)
    return _catalog_to_out(item)


@router.patch("/catalog/{package_id}", response_model=CatalogItemOut)
async def update_catalog_item(
    package_id: str,
    payload: CatalogItemUpdate,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> CatalogItemOut:
    item = await session.get(CatalogItem, package_id)
    if item is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Catalog item not found"
        )
    updates = payload.model_dump(exclude_none=True)
    for field_name, val in updates.items():
        setattr(item, field_name, val)
    item.updated_at = utc_now()
    await session.flush()

    # Re-evaluate deployments if version or install commands changed
    signing = request.app.state.signing
    ws_hub = request.app.state.ws_hub
    dev_res = await session.execute(
        select(Device).where(
            Device.status == "approved", Device.os_family == item.os_family
        )
    )
    for dev in dev_res.scalars().all():
        await evaluate_deployments_for_device(
            session, dev, signing, settings, ws_hub=ws_hub
        )
        await ws_hub.send_to_device(dev.id, {"type": "catalog_updated"})

    session.add(
        AuditLog(
            actor=admin.username,
            action="catalog.update",
            target_type="catalog_item",
            target_id=item.id,
            details=updates,
        )
    )
    await session.commit()
    await session.refresh(item)
    return _catalog_to_out(item)


@router.delete("/catalog/{package_id}")
async def delete_catalog_item(
    package_id: str,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> dict[str, str]:
    item = await session.get(CatalogItem, package_id)
    if item is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Catalog item not found"
        )
    storage: PackageStorageService = request.app.state.storage
    storage.delete_package_files(package_id)
    name = item.name
    await session.delete(item)
    session.add(
        AuditLog(
            actor=admin.username,
            action="catalog.delete",
            target_type="catalog_item",
            target_id=package_id,
            details={"name": name},
        )
    )
    await session.commit()
    return {"status": "deleted", "id": package_id}


@router.post("/catalog/{package_id}/upload", response_model=CatalogItemOut)
async def upload_catalog_binary(
    package_id: str,
    request: Request,
    file: UploadFile = File(...),
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> CatalogItemOut:
    item = await session.get(CatalogItem, package_id)
    if item is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Catalog item not found"
        )
    storage: PackageStorageService = request.app.state.storage
    saved_path, filename, file_size, sha256 = await storage.save_full_upload(
        package_id, file
    )
    item.filename = filename
    item.file_size = file_size
    item.sha256 = sha256
    item.storage_path = str(saved_path)
    if not item.package_ref:
        item.package_ref = filename
    item.updated_at = utc_now()

    session.add(
        AuditLog(
            actor=admin.username,
            action="catalog.upload",
            target_type="catalog_item",
            target_id=package_id,
            details={"filename": filename, "file_size": file_size, "sha256": sha256},
        )
    )
    await session.commit()
    await session.refresh(item)
    return _catalog_to_out(item)


@router.post("/catalog/{package_id}/upload-chunk", response_model=ChunkUploadResponse)
async def upload_catalog_chunk(
    package_id: str,
    request: Request,
    chunk_index: int = Form(...),
    total_chunks: int = Form(...),
    filename: Optional[str] = Form(default=None),
    file: UploadFile = File(...),
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> ChunkUploadResponse:
    item = await session.get(CatalogItem, package_id)
    if item is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Catalog item not found"
        )
    storage: PackageStorageService = request.app.state.storage
    completed, final_path, final_name, file_size, sha256 = await storage.save_chunk(
        package_id=package_id,
        chunk_index=chunk_index,
        total_chunks=total_chunks,
        upload=file,
        explicit_filename=filename,
    )
    if not completed:
        return ChunkUploadResponse(
            package_id=package_id,
            chunk_index=chunk_index,
            total_chunks=total_chunks,
            completed=False,
        )

    assert final_path is not None and final_name is not None and sha256 is not None
    item.filename = final_name
    item.file_size = file_size or 0
    item.sha256 = sha256
    item.storage_path = str(final_path)
    if not item.package_ref:
        item.package_ref = final_name
    item.updated_at = utc_now()

    session.add(
        AuditLog(
            actor=admin.username,
            action="catalog.upload_chunked",
            target_type="catalog_item",
            target_id=package_id,
            details={
                "filename": final_name,
                "file_size": file_size,
                "sha256": sha256,
                "total_chunks": total_chunks,
            },
        )
    )
    await session.commit()
    await session.refresh(item)
    return ChunkUploadResponse(
        package_id=package_id,
        chunk_index=chunk_index,
        total_chunks=total_chunks,
        completed=True,
        sha256=sha256,
        file_size=file_size,
        catalog_item=_catalog_to_out(item),
    )


# --- Deployments ---


def _compute_deployment_stats(
    dep: Deployment,
    pkg: Optional[CatalogItem],
    devices: list[Device],
    jobs_by_device: dict[str, list[Job]],
) -> DeploymentStatsOut:
    if pkg is None:
        return DeploymentStatsOut()
    targeted = [
        d
        for d in devices
        if d.os_family == pkg.os_family and device_matches_deployment(d, dep)
    ]
    total = len(targeted)
    installed = 0
    failed = 0
    pending = 0

    for dev in targeted:
        inst_ver = find_installed_package_version(dev.installed_software, pkg)
        dev_jobs = [
            j
            for j in jobs_by_device.get(dev.id, [])
            if j.job_type == "software" and j.target_id == pkg.id
        ]
        latest_job = dev_jobs[0] if dev_jobs else None

        if dep.mode == "uninstall":
            if inst_ver is None and (
                latest_job is None or latest_job.status == "succeeded"
            ):
                installed += 1
            elif latest_job and latest_job.status == "failed":
                failed += 1
            else:
                pending += 1
        elif dep.mode == "required":
            if inst_ver is not None and compare_versions(inst_ver, pkg.version) >= 0:
                installed += 1
            elif latest_job and latest_job.status == "failed":
                failed += 1
            else:
                pending += 1
        else:
            # available mode
            if inst_ver is not None and compare_versions(inst_ver, pkg.version) >= 0:
                installed += 1
            elif latest_job and latest_job.status == "failed":
                failed += 1
            elif latest_job and latest_job.status in ACTIVE_JOB_STATUSES:
                pending += 1

    return DeploymentStatsOut(
        total=total,
        installed=installed,
        pending=pending,
        failed=failed,
    )


@router.get("/deployments", response_model=list[DeploymentOut])
async def list_deployments(
    _admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> list[DeploymentOut]:
    dep_res = await session.execute(
        select(Deployment).order_by(desc(Deployment.created_at))
    )
    deployments = list(dep_res.scalars().all())
    pkg_res = await session.execute(select(CatalogItem))
    pkg_map = {p.id: p for p in pkg_res.scalars().all()}

    dev_res = await session.execute(select(Device).where(Device.status == "approved"))
    devices = list(dev_res.scalars().all())

    jobs_res = await session.execute(
        select(Job).where(Job.job_type == "software").order_by(desc(Job.created_at))
    )
    jobs_by_device: dict[str, list[Job]] = {}
    for j in jobs_res.scalars().all():
        jobs_by_device.setdefault(j.device_id, []).append(j)

    return [
        DeploymentOut(
            id=d.id,
            package_id=d.package_id,
            package_name=pkg_map[d.package_id].name if d.package_id in pkg_map else "",
            package_version=pkg_map[d.package_id].version
            if d.package_id in pkg_map
            else "",
            os_family=pkg_map[d.package_id].os_family if d.package_id in pkg_map else "",
            target_group_ids=list(d.target_group_ids or []),
            target_device_ids=list(d.target_device_ids or []),
            mode=d.mode,
            respect_maintenance_window=d.respect_maintenance_window,
            stats=_compute_deployment_stats(
                d, pkg_map.get(d.package_id), devices, jobs_by_device
            ),
            created_at=d.created_at,
        )
        for d in deployments
    ]


@router.post("/deployments", response_model=DeploymentOut)
async def create_deployment(
    payload: DeploymentCreate,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> DeploymentOut:
    pkg = await session.get(CatalogItem, payload.package_id)
    if pkg is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Catalog package not found"
        )
    if not payload.target_group_ids and not payload.target_device_ids:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="At least one target_group_id or target_device_id must be specified",
        )

    dep = Deployment(
        package_id=payload.package_id,
        target_group_ids=payload.target_group_ids,
        target_device_ids=payload.target_device_ids,
        mode=payload.mode,
        respect_maintenance_window=payload.respect_maintenance_window,
    )
    session.add(dep)
    await session.flush()

    signing = request.app.state.signing
    ws_hub = request.app.state.ws_hub
    dev_res = await session.execute(select(Device).where(Device.status == "approved"))
    devices = list(dev_res.scalars().all())

    for dev in devices:
        if dev.os_family != pkg.os_family or not device_matches_deployment(dev, dep):
            continue
        if dep.mode in ("required", "uninstall"):
            await evaluate_deployments_for_device(
                session,
                dev,
                signing,
                settings,
                ws_hub=ws_hub,
                from_deployment_creation=True,
            )
        else:
            await ws_hub.send_to_device(dev.id, {"type": "catalog_updated"})

    session.add(
        AuditLog(
            actor=admin.username,
            action="deployment.create",
            target_type="deployment",
            target_id=dep.id,
            details=payload.model_dump(),
        )
    )
    await session.commit()
    await session.refresh(dep)

    jobs_res = await session.execute(
        select(Job).where(Job.job_type == "software").order_by(desc(Job.created_at))
    )
    jobs_by_device: dict[str, list[Job]] = {}
    for j in jobs_res.scalars().all():
        jobs_by_device.setdefault(j.device_id, []).append(j)

    return DeploymentOut(
        id=dep.id,
        package_id=dep.package_id,
        package_name=pkg.name,
        package_version=pkg.version,
        os_family=pkg.os_family,
        target_group_ids=list(dep.target_group_ids or []),
        target_device_ids=list(dep.target_device_ids or []),
        mode=dep.mode,
        respect_maintenance_window=dep.respect_maintenance_window,
        stats=_compute_deployment_stats(dep, pkg, devices, jobs_by_device),
        created_at=dep.created_at,
    )


@router.delete("/deployments/{deployment_id}")
async def delete_deployment(
    deployment_id: str,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> dict[str, str]:
    dep = await session.get(Deployment, deployment_id)
    if dep is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Deployment not found"
        )
    ws_hub = request.app.state.ws_hub
    dev_res = await session.execute(select(Device).where(Device.status == "approved"))
    affected = [
        d.id for d in dev_res.scalars().all() if device_matches_deployment(d, dep)
    ]

    await session.delete(dep)
    session.add(
        AuditLog(
            actor=admin.username,
            action="deployment.delete",
            target_type="deployment",
            target_id=deployment_id,
            details={"package_id": dep.package_id, "mode": dep.mode},
        )
    )
    await session.commit()

    for dev_id in affected:
        await ws_hub.send_to_device(dev_id, {"type": "catalog_updated"})

    return {"status": "deleted", "id": deployment_id}


# --- Jobs & Self-Update ---


@router.get("/jobs", response_model=list[JobOut])
async def list_jobs(
    device_id: Optional[str] = Query(default=None),
    status_filter: Optional[str] = Query(default=None, alias="status"),
    job_type: Optional[str] = Query(default=None),
    limit: int = Query(default=100, ge=1, le=500),
    _admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> list[JobOut]:
    dev_res = await session.execute(select(Device.id, Device.hostname))
    host_map = {did: h for did, h in dev_res.all()}

    stmt = select(Job).order_by(desc(Job.created_at)).limit(limit)
    if device_id:
        stmt = stmt.where(Job.device_id == device_id)
    if status_filter:
        stmt = stmt.where(Job.status == status_filter)
    if job_type:
        stmt = stmt.where(Job.job_type == job_type)

    res = await session.execute(stmt)
    return [job_to_out(j, host_map.get(j.device_id, "")) for j in res.scalars().all()]


@router.get("/jobs/{job_id}", response_model=JobOut)
async def get_job_detail(
    job_id: str,
    _admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> JobOut:
    job = await session.get(Job, job_id)
    if job is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Job not found"
        )
    dev = await session.get(Device, job.device_id)
    return job_to_out(job, dev.hostname if dev else "")


@router.post("/self-update")
async def trigger_agent_self_update(
    payload: SelfUpdateRequest,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> dict[str, Any]:
    signing = request.app.state.signing
    ws_hub = request.app.state.ws_hub
    now = current_utc_time()

    stmt = select(Device).where(Device.status == "approved")
    if payload.os_family:
        stmt = stmt.where(Device.os_family == payload.os_family)
    dev_res = await session.execute(stmt)
    devices = list(dev_res.scalars().all())

    target_group_set = set(payload.target_group_ids)
    target_device_set = set(payload.target_device_ids)

    job_ids: list[str] = []
    for dev in devices:
        if target_group_set or target_device_set:
            dev_groups = {g.id for g in dev.groups}
            if dev.id not in target_device_set and not dev_groups.intersection(
                target_group_set
            ):
                continue

        if dev.os_family == "windows":
            default_dl = "/dl/agents/windows/swm-agent.exe"
        elif dev.os_family == "macos":
            default_dl = "/dl/agents/macos-universal/swm-agent"
        else:
            default_dl = f"/dl/agents/linux-{dev.arch}/swm-agent"
        dl_url = payload.download_url or default_dl
        job = create_signed_job(
            device_id=dev.id,
            job_type="self_update",
            action="update",
            target_id="agent",
            version=payload.version,
            sha256=payload.sha256,
            command=dl_url,
            download_url=dl_url,
            respect_maintenance_window=payload.respect_maintenance_window,
            signing=signing,
            settings=settings,
            now=now,
        )
        session.add(job)
        await session.flush()
        job_ids.append(job.id)

        windows = await get_device_maintenance_windows(session, dev)
        if can_dispatch_job_now(job, windows, now):
            await ws_hub.send_to_device(
                dev.id, {"type": "job_available", "job_id": job.id}
            )

    session.add(
        AuditLog(
            actor=admin.username,
            action="agent.self_update",
            target_type="self_update",
            target_id=payload.version,
            details={"version": payload.version, "jobs_created": len(job_ids)},
        )
    )
    await session.commit()
    return {"jobs_created": len(job_ids), "job_ids": job_ids}
