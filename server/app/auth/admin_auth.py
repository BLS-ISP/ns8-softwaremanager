from __future__ import annotations

from datetime import timedelta
from typing import Optional

import jwt
from fastapi import Depends, HTTPException, Request, status
from fastapi.security import HTTPAuthorizationCredentials, HTTPBearer
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.auth.passwords import hash_password, hash_token, verify_password
from app.config import Settings
from app.models import AdminUser, Device, utc_now

bearer_scheme = HTTPBearer(auto_error=False)


async def get_settings_dep(request: Request) -> Settings:
    return request.app.state.settings


async def get_db(request: Request):
    async with request.app.state.db.session_factory() as session:
        yield session


async def ensure_bootstrap_admin(session: AsyncSession, settings: Settings) -> AdminUser:
    stmt = select(AdminUser).where(
        AdminUser.username == settings.ADMIN_USERNAME,
        AdminUser.auth_source == "local",
    )
    result = await session.execute(stmt)
    existing = result.scalar_one_or_none()
    if existing is not None:
        if not existing.password_hash or not verify_password(
            settings.ADMIN_PASSWORD, existing.password_hash
        ):
            existing.password_hash = hash_password(settings.ADMIN_PASSWORD)
            await session.commit()
            await session.refresh(existing)
        return existing

    admin = AdminUser(
        username=settings.ADMIN_USERNAME,
        password_hash=hash_password(settings.ADMIN_PASSWORD),
        role="admin",
        auth_source="local",
    )
    session.add(admin)
    await session.commit()
    await session.refresh(admin)
    return admin


def create_access_token(user: AdminUser, settings: Settings) -> str:
    now = utc_now()
    exp = now + timedelta(minutes=settings.JWT_EXPIRE_MINUTES)
    payload = {
        "sub": user.id,
        "username": user.username,
        "role": user.role,
        "auth_source": user.auth_source,
        "iat": int(now.timestamp()),
        "exp": int(exp.timestamp()),
    }
    return jwt.encode(
        payload,
        settings.effective_jwt_secret(),
        algorithm=settings.JWT_ALGORITHM,
    )


def decode_access_token(token: str, settings: Settings) -> dict:
    try:
        return jwt.decode(
            token,
            settings.effective_jwt_secret(),
            algorithms=[settings.JWT_ALGORITHM],
        )
    except jwt.PyJWTError as exc:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid or expired authentication token",
            headers={"WWW-Authenticate": "Bearer"},
        ) from exc


async def get_current_admin(
    credentials: Optional[HTTPAuthorizationCredentials] = Depends(bearer_scheme),
    session: AsyncSession = Depends(get_db),
    settings: Settings = Depends(get_settings_dep),
) -> AdminUser:
    if credentials is None or credentials.scheme.lower() != "bearer":
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Missing Bearer authentication token",
            headers={"WWW-Authenticate": "Bearer"},
        )
    payload = decode_access_token(credentials.credentials, settings)
    user_id = payload.get("sub")
    if not user_id:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Malformed token payload",
        )
    stmt = select(AdminUser).where(AdminUser.id == user_id)
    res = await session.execute(stmt)
    user = res.scalar_one_or_none()
    if user is None:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Admin user not found",
        )
    return user


async def get_device_by_bearer(
    credentials: Optional[HTTPAuthorizationCredentials] = Depends(bearer_scheme),
    session: AsyncSession = Depends(get_db),
) -> Device:
    if credentials is None or credentials.scheme.lower() != "bearer":
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Missing device Bearer token",
            headers={"WWW-Authenticate": "Bearer"},
        )
    th = hash_token(credentials.credentials)
    stmt = select(Device).where(Device.token_hash == th)
    res = await session.execute(stmt)
    device = res.scalar_one_or_none()
    if device is None:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid device token",
        )
    return device


async def get_non_revoked_device(
    device: Device = Depends(get_device_by_bearer),
) -> Device:
    if device.status == "revoked":
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Device access has been revoked",
        )
    return device


async def get_approved_device(
    device: Device = Depends(get_device_by_bearer),
) -> Device:
    if device.status == "revoked":
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Device access has been revoked",
        )
    if device.status != "approved":
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail=f"Device is not approved yet (current status: {device.status})",
        )
    return device
