from __future__ import annotations

from datetime import datetime

from sqlalchemy import Boolean, DateTime, JSON, String
from sqlalchemy.orm import Mapped, mapped_column

from app.db import Base
from app.models.base_utils import utc_now


class PatchPolicy(Base):
    __tablename__ = "patch_policies"

    update_id: Mapped[str] = mapped_column(String(128), primary_key=True)
    os_family: Mapped[str] = mapped_column(String(32), nullable=False, default="windows")
    title: Mapped[str] = mapped_column(String(512), nullable=False, default="")
    severity: Mapped[str] = mapped_column(String(32), nullable=False, default="important")
    category: Mapped[str] = mapped_column(String(64), nullable=False, default="security")
    approved: Mapped[bool] = mapped_column(Boolean, nullable=False, default=False)
    excluded: Mapped[bool] = mapped_column(Boolean, nullable=False, default=False)
    approved_group_ids: Mapped[list[str]] = mapped_column(JSON, nullable=False, default=list)
    respect_maintenance_window: Mapped[bool] = mapped_column(
        Boolean, nullable=False, default=True
    )
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, default=utc_now, onupdate=utc_now
    )
