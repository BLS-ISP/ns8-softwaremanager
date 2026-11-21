from __future__ import annotations

from datetime import timedelta

from fastapi import APIRouter, Depends, HTTPException, Request, status
from sqlalchemy import desc, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.health_and_dl import resolve_public_base_url
from app.auth import (
    generate_enrollment_token,
    get_current_admin,
    get_db,
    get_settings_dep,
    hash_token,
)
from app.config import Settings
from app.models import AdminUser, AuditLog, EnrollmentToken, Group, utc_now
from app.schemas import EnrollmentTokenCreate, EnrollmentTokenOut

router = APIRouter(prefix="/api/v1/admin/enrollment-tokens", tags=["enrollment"])


def _to_enrollment_out(
    tok: EnrollmentToken, base_url: str
) -> EnrollmentTokenOut:
    ps1_url = f"{base_url}/dl/install.ps1?token={tok.token_plain}&headless=0"
    sh_url = f"{base_url}/dl/install.sh?token={tok.token_plain}&headless=0"
    return EnrollmentTokenOut(
        id=tok.id,
        name=tok.name,
        token=tok.token_plain,
        expires_at=tok.expires_at,
        max_uses=tok.max_uses,
        uses_count=tok.uses_count,
        auto_approve=tok.auto_approve,
        group_id=tok.group_id,
        revoked=tok.revoked,
        created_at=tok.created_at,
        install_ps1_cmd=f"irm '{ps1_url}' | iex",
        install_sh_cmd=f"curl -fsSL '{sh_url}' | sudo bash",
    )


@router.get("", response_model=list[EnrollmentTokenOut])
async def list_enrollment_tokens(
    request: Request,
    _admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> list[EnrollmentTokenOut]:
    base_url = resolve_public_base_url(request, settings)
    res = await session.execute(
        select(EnrollmentToken).order_by(desc(EnrollmentToken.created_at))
    )
    return [_to_enrollment_out(t, base_url) for t in res.scalars().all()]


@router.post("", response_model=EnrollmentTokenOut)
async def create_enrollment_token(
    payload: EnrollmentTokenCreate,
    request: Request,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> EnrollmentTokenOut:
    if payload.group_id:
        grp = await session.get(Group, payload.group_id)
        if grp is None:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail="Specified group_id does not exist",
            )

    raw_token = generate_enrollment_token()
    now = utc_now()
    tok = EnrollmentToken(
        name=payload.name,
        token_hash=hash_token(raw_token),
        token_plain=raw_token,
        expires_at=now + timedelta(hours=payload.expires_in_hours),
        max_uses=payload.max_uses,
        uses_count=0,
        auto_approve=payload.auto_approve,
        group_id=payload.group_id,
        revoked=False,
        created_at=now,
    )
    session.add(tok)
    session.add(
        AuditLog(
            actor=admin.username,
            action="enrollment_token.create",
            target_type="enrollment_token",
            target_id=tok.id,
            details={"name": tok.name, "auto_approve": tok.auto_approve},
        )
    )
    await session.commit()
    await session.refresh(tok)
    base_url = resolve_public_base_url(request, settings)
    return _to_enrollment_out(tok, base_url)


@router.delete("/{token_id}")
async def delete_enrollment_token(
    token_id: str,
    admin: AdminUser = Depends(get_current_admin),
    session: AsyncSession = Depends(get_db),
) -> dict[str, str]:
    tok = await session.get(EnrollmentToken, token_id)
    if tok is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Enrollment token not found",
        )
    tok.revoked = True
    await session.delete(tok)
    session.add(
        AuditLog(
            actor=admin.username,
            action="enrollment_token.delete",
            target_type="enrollment_token",
            target_id=token_id,
            details={"name": tok.name},
        )
    )
    await session.commit()
    return {"status": "deleted", "id": token_id}
