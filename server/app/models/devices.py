from __future__ import annotations

from datetime import datetime
from typing import Any, Optional

from sqlalchemy import BigInteger, Boolean, DateTime, ForeignKey, JSON, String
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.db import Base
from app.models.base_utils import new_uuid, utc_now
from app.models.groups import Group, device_group_links


class Device(Base):
    __tablename__ = "devices"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_uuid)
    hostname: Mapped[str] = mapped_column(String(255), nullable=False, index=True)
    domain: Mapped[str] = mapped_column(String(255), nullable=False, default="")
    os_family: Mapped[str] = mapped_column(String(32), nullable=False, index=True)
    os_name: Mapped[str] = mapped_column(String(128), nullable=False, default="")
    os_version: Mapped[str] = mapped_column(String(128), nullable=False, default="")
    kernel_or_build: Mapped[str] = mapped_column(String(128), nullable=False, default="")
    arch: Mapped[str] = mapped_column(String(32), nullable=False, default="amd64")
    agent_version: Mapped[str] = mapped_column(String(64), nullable=False, default="1.0.0")
    machine_uid: Mapped[str] = mapped_column(String(255), nullable=False, index=True)
    token_hash: Mapped[str] = mapped_column(String(64), nullable=False, unique=True, index=True)
    status: Mapped[str] = mapped_column(
        String(32), nullable=False, default="pending", index=True
    )  # pending | approved | revoked
    is_online: Mapped[bool] = mapped_column(Boolean, nullable=False, default=False)
    reboot_required: Mapped[bool] = mapped_column(Boolean, nullable=False, default=False)
    center_running: Mapped[bool] = mapped_column(Boolean, nullable=False, default=False)
    hardware: Mapped[dict[str, Any]] = mapped_column(JSON, nullable=False, default=dict)
    ip_addresses: Mapped[list[str]] = mapped_column(JSON, nullable=False, default=list)
    logged_in_users: Mapped[list[str]] = mapped_column(JSON, nullable=False, default=list)
    tags: Mapped[list[str]] = mapped_column(JSON, nullable=False, default=list)
    software_hash: Mapped[Optional[str]] = mapped_column(String(128), nullable=True)
    last_seen_at: Mapped[Optional[datetime]] = mapped_column(
        DateTime(timezone=True), nullable=True
    )
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, default=utc_now
    )

    groups: Mapped[list[Group]] = relationship(
        Group, secondary=device_group_links, lazy="selectin"
    )
    installed_software: Mapped[list["InstalledSoftware"]] = relationship(
        "InstalledSoftware",
        back_populates="device",
        cascade="all, delete-orphan",
        lazy="selectin",
    )
    pending_updates: Mapped[list["DevicePendingUpdate"]] = relationship(
        "DevicePendingUpdate",
        back_populates="device",
        cascade="all, delete-orphan",
        lazy="selectin",
    )


class InstalledSoftware(Base):
    __tablename__ = "installed_software"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_uuid)
    device_id: Mapped[str] = mapped_column(
        String(36), ForeignKey("devices.id", ondelete="CASCADE"), nullable=False, index=True
    )
    name: Mapped[str] = mapped_column(String(255), nullable=False, index=True)
    version: Mapped[str] = mapped_column(String(128), nullable=False, default="")
    publisher: Mapped[str] = mapped_column(String(255), nullable=False, default="")
    source: Mapped[str] = mapped_column(String(64), nullable=False, default="system")
    scope: Mapped[str] = mapped_column(String(32), nullable=False, default="machine")
    package_ref: Mapped[str] = mapped_column(String(255), nullable=False, default="", index=True)

    device: Mapped[Device] = relationship("Device", back_populates="installed_software")


class DevicePendingUpdate(Base):
    __tablename__ = "device_pending_updates"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_uuid)
    device_id: Mapped[str] = mapped_column(
        String(36), ForeignKey("devices.id", ondelete="CASCADE"), nullable=False, index=True
    )
    update_id: Mapped[str] = mapped_column(String(128), nullable=False, index=True)
    title: Mapped[str] = mapped_column(String(512), nullable=False, default="")
    severity: Mapped[str] = mapped_column(String(32), nullable=False, default="important")
    category: Mapped[str] = mapped_column(String(64), nullable=False, default="security")
    size_bytes: Mapped[int] = mapped_column(BigInteger, nullable=False, default=0)
    reboot_behavior: Mapped[str] = mapped_column(
        String(64), nullable=False, default="may_require_reboot"
    )
    reported_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, default=utc_now
    )

    device: Mapped[Device] = relationship("Device", back_populates="pending_updates")
