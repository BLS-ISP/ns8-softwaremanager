from __future__ import annotations

from datetime import datetime

from sqlalchemy import Boolean, DateTime, ForeignKey, JSON, String
from sqlalchemy.orm import Mapped, mapped_column

from app.db import Base
from app.models.base_utils import new_uuid, utc_now


class Deployment(Base):
    __tablename__ = "deployments"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_uuid)
    package_id: Mapped[str] = mapped_column(
        String(36),
        ForeignKey("catalog_items.id", ondelete="CASCADE"),
        nullable=False,
        index=True,
    )
    target_group_ids: Mapped[list[str]] = mapped_column(JSON, nullable=False, default=list)
    target_device_ids: Mapped[list[str]] = mapped_column(JSON, nullable=False, default=list)
    target_ad_groups: Mapped[list[str]] = mapped_column(JSON, nullable=False, default=list)
    mode: Mapped[str] = mapped_column(
        String(32), nullable=False, index=True
    )  # required | available | uninstall
    respect_maintenance_window: Mapped[bool] = mapped_column(
        Boolean, nullable=False, default=True
    )
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, default=utc_now
    )
