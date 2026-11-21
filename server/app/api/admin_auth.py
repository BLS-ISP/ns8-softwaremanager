from __future__ import annotations

from fastapi import APIRouter, Depends, HTTPException, Request, status
from sqlalchemy import desc, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.auth import (
    LdapConfigError,
    authenticate_ldap,
    create_access_token,
    ensure_bootstrap_admin,
    get_current_admin,
    get_db,
    get_settings_dep,
    verify_password,
)
from app.config import Settings
from app.models import AdminUser, AuditLog, Device, Job, PatchPolicy
from app.schemas import (
    AdminUserOut,
    DashboardSummaryOut,
    JobOut,
    LoginRequest,
    LoginResponse,
    OsBreakdownOut,
)

router = APIRouter(prefix="/api/v1/admin", tags=["admin-auth"])


def job_to_out(job: Job, hostname: str = "") -> JobOut:
    return JobOut(
        id=job.id,
        device_id=job.device_id,
        device_hostname=hostname,
        deployment_id=job.deployment_id,
        job_type=job.job_type,
        action=job.action,
        target_id=job.target_id,
        version=job.version,
        sha256=job.sha256,
        command=job.command,
        download_url=job.download_url,
        update_ids=list(job.update_ids or []),
        security_only=job.security_only,
        respect_maintenance_window=job.respect_maintenance_window,
        status=job.status,
        exit_code=job.exit_code,
        reboot_required=job.reboot_required,
        stdout=job.stdout,
        stderr=job.stderr,
        expires_at_unix=job.expires_at_unix,
        signature_b64=job.signature_b64,
        created_at=job.created_at,
        dispatched_at=job.dispatched_at,
        started_at=job.started_at,
        completed_at=job.completed_at,
    )


@router.post("/auth/login", response_model=LoginResponse)
async def admin_login(
    payload: LoginRequest,
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> LoginResponse:
    await ensure_bootstrap_admin(session, settings)

    if payload.auth_source == "local":
        stmt = select(AdminUser).where(
            AdminUser.username == payload.username,
            AdminUser.auth_source == "local",
        )
        res = await session.execute(stmt)
        user = res.scalar_one_or_none()
        if (
            user is None
            or not user.password_hash
            or not verify_password(payload.password, user.password_hash)
        ):
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Invalid username or password",
            )
    else:
        try:
            ldap_info = await authenticate_ldap(
                payload.username, payload.password, settings
            )
        except LdapConfigError as exc:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=str(exc),
            ) from exc

        if ldap_info is None:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="LDAP authentication failed",
            )

        canonical_user = ldap_info["username"]
        stmt = select(AdminUser).where(
            AdminUser.username == canonical_user,
            AdminUser.auth_source == "ldap",
        )
        res = await session.execute(stmt)
        user = res.scalar_one_or_none()
        if user is None:
            user = AdminUser(
                username=canonical_user,
                password_hash=None,
                role=ldap_info.get("role", "admin"),
                auth_source="ldap",
            )
            session.add(user)
            await session.flush()

    session.add(
        AuditLog(
            actor=user.username,
            action="admin.login",
            target_type="admin_user",
            target_id=user.id,
            details={"auth_source": user.auth_source},
        )
    )
    await session.commit()
    await session.refresh(user)

    token = create_access_token(user, settings)
    return LoginResponse(
        access_token=token,
        token_type="bearer",
        user=AdminUserOut.model_validate(user),
    )


@router.get("/auth/me", response_model=AdminUserOut)
async def admin_me(
    current_admin: AdminUser = Depends(get_current_admin),
) -> AdminUserOut:
    return AdminUserOut.model_validate(current_admin)


@router.get("/dashboard/summary", response_model=DashboardSummaryOut)
async def dashboard_summary(
    request: Request,
    _admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> DashboardSummaryOut:
    ws_hub = request.app.state.ws_hub
    dev_res = await session.execute(select(Device))
    devices = list(dev_res.scalars().all())
    hostname_map = {d.id: d.hostname for d in devices}

    pol_res = await session.execute(
        select(PatchPolicy.update_id).where(PatchPolicy.excluded.is_(True))
    )
    excluded_ids = set(pol_res.scalars().all())

    total_devices = len(devices)
    online_devices = sum(
        1
        for d in devices
        if d.status == "approved" and (d.is_online or ws_hub.is_online(d.id))
    )
    pending_approval_devices = sum(1 for d in devices if d.status == "pending")
    approved_devices = [d for d in devices if d.status == "approved"]
    reboot_required_devices = sum(1 for d in approved_devices if d.reboot_required)

    win_count = sum(1 for d in devices if d.os_family == "windows")
    linux_count = sum(1 for d in devices if d.os_family == "linux")

    compliant_count = 0
    pending_critical_patches = 0
    for d in approved_devices:
        non_excluded = [
            u for u in d.pending_updates if u.update_id not in excluded_ids
        ]
        crit = [
            u
            for u in non_excluded
            if u.severity.lower() in ("critical", "important", "security")
            or u.category.lower() == "security"
        ]
        pending_critical_patches += len(crit)
        if not crit:
            compliant_count += 1

    compliance_pct = (
        round((compliant_count / len(approved_devices)) * 100.0, 1)
        if approved_devices
        else 100.0
    )

    jobs_res = await session.execute(
        select(Job).order_by(desc(Job.created_at)).limit(10)
    )
    recent_jobs = [
        job_to_out(j, hostname_map.get(j.device_id, ""))
        for j in jobs_res.scalars().all()
    ]

    return DashboardSummaryOut(
        total_devices=total_devices,
        online_devices=online_devices,
        pending_approval_devices=pending_approval_devices,
        reboot_required_devices=reboot_required_devices,
        patch_compliance_percent=compliance_pct,
        pending_critical_patches=pending_critical_patches,
        recent_jobs=recent_jobs,
        os_breakdown=OsBreakdownOut(windows=win_count, linux=linux_count),
    )
