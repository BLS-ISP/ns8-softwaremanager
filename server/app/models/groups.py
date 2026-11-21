from __future__ import annotations

from datetime import datetime

from sqlalchemy import Column, DateTime, ForeignKey, Integer, String, Table
from sqlalchemy.orm import Mapped, mapped_column

from app.db import Base
from app.models.base_utils import new_uuid, utc_now

device_group_links = Table(
    "device_group_links",
    Base.metadata,
    Column(
        "device_id",
        String(36),
        ForeignKey("devices.id", ondelete="CASCADE"),
        primary_key=True,
    ),
    Column(
        "group_id",
        String(36),
        ForeignKey("groups.id", ondelete="CASCADE"),
        primary_key=True,
    ),
)


class Group(Base):
    __tablename__ = "groups"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_uuid)
    name: Mapped[str] = mapped_column(String(128), nullable=False, unique=True, index=True)
    description: Mapped[str] = mapped_column(String(512), nullable=False, default="")
    reboot_policy: Mapped[str] = mapped_column(
        String(32), nullable=False, default="never"
    )  # never | maintenance_window | grace_period
    grace_period_minutes: Mapped[int] = mapped_column(Integer, nullable=False, default=240)
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, default=utc_now
    )
