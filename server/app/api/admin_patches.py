from __future__ import annotations

from typing import Any, Optional

from fastapi import APIRouter, Depends, Query, Request
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.auth import get_current_admin, get_db, get_settings_dep
from app.config import Settings
from app.models import (
    AdminUser,
    AuditLog,
    Device,
    Job,
    PatchPolicy,
    ensure_utc,
    utc_now,
)
from app.schemas import (
    PatchApproveRequest,
    PatchExcludeRequest,
    PatchSummaryOut,
    normalize_os_family_str,
)
from app.services import (
    ACTIVE_JOB_STATUSES,
    can_dispatch_job_now,
    create_signed_job,
    current_utc_time,
    get_device_maintenance_windows,
)

router = APIRouter(prefix="/api/v1/admin/patches", tags=["admin-patches"])


def _infer_fallback_patch_os(update_id: str) -> str:
    upper = update_id.upper()
    if upper.startswith("KB"):
        return "windows"
    low = update_id.lower()
    if any(
        marker in low
        for marker in ("macos", "msu_", "osx", "safari", "xcode", "command line tools")
    ):
        return "macos"
    return "linux"


@router.get("", response_model=list[PatchSummaryOut])
async def list_patches(
    os_family: Optional[str] = Query(default=None),
    severity: Optional[str] = Query(default=None),
    excluded: Optional[bool] = Query(default=None),
    approved: Optional[bool] = Query(default=None),
    search: Optional[str] = Query(default=None),
    _admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> list[PatchSummaryOut]:
    pol_res = await session.execute(select(PatchPolicy))
    pol_map = {p.update_id: p for p in pol_res.scalars().all()}

    dev_res = await session.execute(select(Device).where(Device.status != "revoked"))
    devices = list(dev_res.scalars().all())

    agg: dict[str, dict[str, Any]] = {}
    for dev in devices:
        dev_os = normalize_os_family_str(dev.os_family or "")
        if dev_os not in ("windows", "linux", "macos"):
            dev_os = dev.os_family
        for u in dev.pending_updates or []:
            pol = pol_map.get(u.update_id)
            rec = agg.get(u.update_id)
            reported = ensure_utc(u.reported_at)
            if rec is None:
                agg[u.update_id] = {
                    "update_id": u.update_id,
                    "title": u.title or (pol.title if pol else u.update_id),
                    "os_family": dev_os,
                    "severity": u.severity or (pol.severity if pol else "important"),
                    "category": u.category or (pol.category if pol else "security"),
                    "size_bytes": u.size_bytes,
                    "affected_device_ids": [dev.id],
                    "approved": pol.approved if pol else False,
                    "excluded": pol.excluded if pol else False,
                    "last_seen_at": reported,
                }
            else:
                if dev.id not in rec["affected_device_ids"]:
                    rec["affected_device_ids"].append(dev.id)
                if reported and (
                    rec["last_seen_at"] is None or reported > rec["last_seen_at"]
                ):
                    rec["last_seen_at"] = reported

    # Include explicitly excluded/approved policies even if 0 devices currently report them
    for uid, pol in pol_map.items():
        if uid not in agg:
            agg[uid] = {
                "update_id": uid,
                "title": pol.title or uid,
                "os_family": pol.os_family,
                "severity": pol.severity,
                "category": pol.category,
                "size_bytes": 0,
                "affected_device_ids": [],
                "approved": pol.approved,
                "excluded": pol.excluded,
                "last_seen_at": ensure_utc(pol.updated_at),
            }

    results = [
        PatchSummaryOut(
            update_id=r["update_id"],
            title=r["title"],
            os_family=r["os_family"],
            severity=r["severity"],
            category=r["category"],
            size_bytes=r["size_bytes"],
            affected_device_count=len(r["affected_device_ids"]),
            affected_device_ids=r["affected_device_ids"],
            approved=r["approved"],
            excluded=r["excluded"],
            last_seen_at=r["last_seen_at"],
        )
        for r in agg.values()
    ]

    if os_family:
        results = [r for r in results if r.os_family == os_family.lower()]
    if severity:
        results = [r for r in results if r.severity.lower() == severity.lower()]
    if excluded is not None:
        results = [r for r in results if r.excluded is excluded]
    if approved is not None:
        results = [r for r in results if r.approved is approved]
    if search:
        q = search.strip().lower()
        results = [
            r for r in results if q in r.update_id.lower() or q in r.title.lower()
        ]

    results.sort(key=lambda r: (-r.affected_device_count, r.update_id))
    return results


@router.post("/approve")
async def approve_patches(
    payload: PatchApproveRequest,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> dict[str, Any]:
    signing = request.app.state.signing
    ws_hub = request.app.state.ws_hub
    now = current_utc_time()

    dev_res = await session.execute(select(Device).where(Device.status == "approved"))
    devices = list(dev_res.scalars().all())

    # Upsert PatchPolicy records
    pol_res = await session.execute(
        select(PatchPolicy).where(PatchPolicy.update_id.in_(payload.update_ids))
    )
    pol_map = {p.update_id: p for p in pol_res.scalars().all()}

    # Gather metadata from pending updates if policy doesn't exist yet
    all_dev_res = await session.execute(
        select(Device).where(Device.status != "revoked")
    )
    meta_by_uid: dict[str, tuple[str, str, str, str]] = {}
    for d in all_dev_res.scalars().all():
        d_os = normalize_os_family_str(d.os_family or "")
        if d_os not in ("windows", "linux", "macos"):
            d_os = d.os_family
        for u in d.pending_updates or []:
            if u.update_id not in meta_by_uid:
                meta_by_uid[u.update_id] = (
                    d_os,
                    u.title,
                    u.severity,
                    u.category,
                )

    for uid in payload.update_ids:
        pol = pol_map.get(uid)
        if pol is None:
            os_fam, title, sev, cat = meta_by_uid.get(
                uid, (_infer_fallback_patch_os(uid), uid, "important", "security")
            )
            pol = PatchPolicy(
                update_id=uid,
                os_family=os_fam,
                title=title,
                severity=sev,
                category=cat,
                approved=True,
                excluded=False,
                approved_group_ids=list(payload.group_ids),
                respect_maintenance_window=payload.respect_maintenance_window,
                updated_at=utc_now(),
            )
            session.add(pol)
            pol_map[uid] = pol
        else:
            if uid in meta_by_uid:
                pol.os_family = meta_by_uid[uid][0]
            pol.approved = True
            pol.excluded = False
            if payload.group_ids:
                merged = sorted(set(pol.approved_group_ids or []).union(payload.group_ids))
                pol.approved_group_ids = merged
            pol.respect_maintenance_window = payload.respect_maintenance_window
            pol.updated_at = utc_now()

    # Check all excluded updates so we never schedule an excluded KB
    excl_res = await session.execute(
        select(PatchPolicy.update_id).where(PatchPolicy.excluded.is_(True))
    )
    excluded_set = set(excl_res.scalars().all()) - set(payload.update_ids)

    target_uid_set = set(payload.update_ids) - excluded_set
    target_group_set = set(payload.group_ids)
    target_device_set = set(payload.device_ids)

    created_job_ids: list[str] = []
    for dev in devices:
        if target_group_set or target_device_set:
            dev_groups = {g.id for g in dev.groups}
            if dev.id not in target_device_set and not dev_groups.intersection(
                target_group_set
            ):
                continue

        matching_updates = sorted(
            {
                u.update_id
                for u in (dev.pending_updates or [])
                if u.update_id in target_uid_set
                and (
                    not payload.security_only
                    or u.category.lower() == "security"
                    or u.severity.lower() in ("critical", "important", "security")
                )
            }
        )
        if not matching_updates:
            continue

        # Avoid creating an identical duplicate queued job
        existing_res = await session.execute(
            select(Job).where(
                Job.device_id == dev.id,
                Job.job_type == "os_patch",
                Job.status.in_(ACTIVE_JOB_STATUSES),
            )
        )
        already_covered = set()
        for ej in existing_res.scalars().all():
            already_covered.update(ej.update_ids or [])
        to_install = [uid for uid in matching_updates if uid not in already_covered]
        if not to_install:
            continue

        cmd = "security" if payload.security_only else ""
        job = create_signed_job(
            device_id=dev.id,
            job_type="os_patch",
            action="install",
            target_id=",".join(to_install),
            version="",
            sha256="",
            command=cmd,
            update_ids=to_install,
            security_only=payload.security_only,
            respect_maintenance_window=payload.respect_maintenance_window,
            signing=signing,
            settings=settings,
            now=now,
        )
        session.add(job)
        await session.flush()
        created_job_ids.append(job.id)

        windows = await get_device_maintenance_windows(session, dev)
        if can_dispatch_job_now(job, windows, now):
            await ws_hub.send_to_device(
                dev.id, {"type": "job_available", "job_id": job.id}
            )

    session.add(
        AuditLog(
            actor=admin.username,
            action="patches.approve",
            target_type="os_patch",
            target_id=",".join(payload.update_ids[:5]),
            details={
                "update_ids": payload.update_ids,
                "group_ids": payload.group_ids,
                "jobs_created": len(created_job_ids),
            },
        )
    )
    await session.commit()
    return {
        "approved_update_ids": payload.update_ids,
        "jobs_created": len(created_job_ids),
        "job_ids": created_job_ids,
    }


@router.post("/exclude")
async def exclude_patches(
    payload: PatchExcludeRequest,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> dict[str, Any]:
    signing = request.app.state.signing
    pol_res = await session.execute(
        select(PatchPolicy).where(PatchPolicy.update_id.in_(payload.update_ids))
    )
    pol_map = {p.update_id: p for p in pol_res.scalars().all()}

    dev_res = await session.execute(select(Device).where(Device.status != "revoked"))
    meta_by_uid: dict[str, tuple[str, str, str, str]] = {}
    for d in dev_res.scalars().all():
        d_os = normalize_os_family_str(d.os_family or "")
        if d_os not in ("windows", "linux", "macos"):
            d_os = d.os_family
        for u in d.pending_updates or []:
            if u.update_id not in meta_by_uid:
                meta_by_uid[u.update_id] = (
                    d_os,
                    u.title or u.update_id,
                    u.severity or "important",
                    u.category or "security",
                )

    for uid in payload.update_ids:
        pol = pol_map.get(uid)
        if pol is None:
            os_fam, title, sev, cat = meta_by_uid.get(
                uid, (_infer_fallback_patch_os(uid), uid, "important", "security")
            )
            pol = PatchPolicy(
                update_id=uid,
                os_family=os_fam,
                title=title,
                severity=sev,
                category=cat,
                approved=False,
                excluded=payload.excluded,
                updated_at=utc_now(),
            )
            session.add(pol)
        else:
            if uid in meta_by_uid:
                pol.os_family = meta_by_uid[uid][0]
            pol.excluded = payload.excluded
            if payload.excluded:
                pol.approved = False
            pol.updated_at = utc_now()

    # If excluded, strip excluded update_ids from any still-queued os_patch jobs
    if payload.excluded:
        excl_set = set(payload.update_ids)
        q_res = await session.execute(
            select(Job).where(Job.job_type == "os_patch", Job.status == "queued")
        )
        for job in q_res.scalars().all():
            remaining = [u for u in (job.update_ids or []) if u not in excl_set]
            if not remaining:
                await session.delete(job)
            elif len(remaining) != len(job.update_ids or []):
                job.update_ids = remaining
                job.target_id = ",".join(remaining)
                job.expires_at_unix = (
                    int(current_utc_time().timestamp()) + settings.JOB_SIGNATURE_TTL_SECONDS
                )
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

    session.add(
        AuditLog(
            actor=admin.username,
            action="patches.exclude",
            target_type="os_patch",
            target_id=",".join(payload.update_ids[:5]),
            details={"update_ids": payload.update_ids, "excluded": payload.excluded},
        )
    )
    await session.commit()
    return {"update_ids": payload.update_ids, "excluded": payload.excluded}
