from __future__ import annotations

from datetime import datetime
from typing import Optional

from sqlalchemy import BigInteger, Boolean, DateTime, ForeignKey, Integer, JSON, String, Text
from sqlalchemy.orm import Mapped, mapped_column

from app.db import Base
from app.models.base_utils import new_uuid, utc_now


class MaintenanceWindow(Base):
    __tablename__ = "maintenance_windows"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_uuid)
    group_id: Mapped[str] = mapped_column(
        String(36), ForeignKey("groups.id", ondelete="CASCADE"), nullable=False, index=True
    )
    name: Mapped[str] = mapped_column(String(128), nullable=False)
    days_of_week: Mapped[list[int]] = mapped_column(
        JSON, nullable=False, default=list
    )  # 0=Mon .. 6=Sun
    start_time: Mapped[str] = mapped_column(String(8), nullable=False)  # HH:MM
    end_time: Mapped[str] = mapped_column(String(8), nullable=False)  # HH:MM
    timezone: Mapped[str] = mapped_column(String(64), nullable=False, default="UTC")
    catch_up: Mapped[bool] = mapped_column(Boolean, nullable=False, default=True)
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, default=utc_now
    )


class Job(Base):
    __tablename__ = "jobs"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_uuid)
    device_id: Mapped[str] = mapped_column(
        String(36), ForeignKey("devices.id", ondelete="CASCADE"), nullable=False, index=True
    )
    deployment_id: Mapped[Optional[str]] = mapped_column(
        String(36), ForeignKey("deployments.id", ondelete="SET NULL"), nullable=True, index=True
    )
    job_type: Mapped[str] = mapped_column(
        String(32), nullable=False, index=True
    )  # os_patch | software | self_update | scan_updates
    action: Mapped[str] = mapped_column(
        String(32), nullable=False
    )  # install | update | uninstall | scan
    target_id: Mapped[str] = mapped_column(String(512), nullable=False)
    version: Mapped[str] = mapped_column(String(128), nullable=False, default="")
    sha256: Mapped[str] = mapped_column(String(64), nullable=False, default="")
    command: Mapped[str] = mapped_column(Text, nullable=False, default="")
    download_url: Mapped[Optional[str]] = mapped_column(Text, nullable=True)
    update_ids: Mapped[list[str]] = mapped_column(JSON, nullable=False, default=list)
    security_only: Mapped[bool] = mapped_column(Boolean, nullable=False, default=False)
    respect_maintenance_window: Mapped[bool] = mapped_column(
        Boolean, nullable=False, default=True
    )
    status: Mapped[str] = mapped_column(
        String(32), nullable=False, default="queued", index=True
    )  # queued | dispatched | running | succeeded | failed | waiting_for_process
    exit_code: Mapped[Optional[int]] = mapped_column(Integer, nullable=True)
    reboot_required: Mapped[bool] = mapped_column(Boolean, nullable=False, default=False)
    stdout: Mapped[str] = mapped_column(Text, nullable=False, default="")
    stderr: Mapped[str] = mapped_column(Text, nullable=False, default="")
    expires_at_unix: Mapped[int] = mapped_column(BigInteger, nullable=False)
    signature_b64: Mapped[str] = mapped_column(String(128), nullable=False)
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, default=utc_now, index=True
    )
    dispatched_at: Mapped[Optional[datetime]] = mapped_column(
        DateTime(timezone=True), nullable=True
    )
    started_at: Mapped[Optional[datetime]] = mapped_column(
        DateTime(timezone=True), nullable=True
    )
    completed_at: Mapped[Optional[datetime]] = mapped_column(
        DateTime(timezone=True), nullable=True
    )
