from __future__ import annotations

from datetime import datetime
from typing import Any, Optional

from sqlalchemy import BigInteger, Boolean, DateTime, JSON, String, Text
from sqlalchemy.orm import Mapped, mapped_column

from app.db import Base
from app.models.base_utils import new_uuid, utc_now


class CatalogItem(Base):
    __tablename__ = "catalog_items"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_uuid)
    name: Mapped[str] = mapped_column(String(255), nullable=False, index=True)
    version: Mapped[str] = mapped_column(String(128), nullable=False, default="latest")
    vendor: Mapped[str] = mapped_column(String(255), nullable=False, default="")
    description: Mapped[str] = mapped_column(Text, nullable=False, default="")
    category: Mapped[str] = mapped_column(String(128), nullable=False, default="General")
    icon_url: Mapped[str] = mapped_column(Text, nullable=False, default="")
    os_family: Mapped[str] = mapped_column(
        String(32), nullable=False, index=True
    )  # windows | linux
    source_type: Mapped[str] = mapped_column(
        String(32), nullable=False, index=True
    )  # winget | apt | dnf | flatpak | custom
    package_ref: Mapped[str] = mapped_column(String(255), nullable=False, default="")
    install_args: Mapped[str] = mapped_column(Text, nullable=False, default="")
    uninstall_args: Mapped[str] = mapped_column(Text, nullable=False, default="")
    blocked_processes: Mapped[list[str]] = mapped_column(JSON, nullable=False, default=list)
    allow_uninstall: Mapped[bool] = mapped_column(Boolean, nullable=False, default=True)
    return_codes: Mapped[dict[str, str]] = mapped_column(
        JSON,
        nullable=False,
        default=lambda: {"0": "success", "3010": "reboot_required", "1641": "reboot_required"},
    )
    detection_rule: Mapped[dict[str, Any]] = mapped_column(JSON, nullable=False, default=dict)
    filename: Mapped[Optional[str]] = mapped_column(String(255), nullable=True)
    file_size: Mapped[int] = mapped_column(BigInteger, nullable=False, default=0)
    sha256: Mapped[str] = mapped_column(String(64), nullable=False, default="")
    storage_path: Mapped[Optional[str]] = mapped_column(Text, nullable=True)
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, default=utc_now
    )
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, default=utc_now, onupdate=utc_now
    )
