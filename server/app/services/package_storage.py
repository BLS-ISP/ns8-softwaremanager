from __future__ import annotations

import hashlib
import re
import shutil
from collections.abc import AsyncIterator
from pathlib import Path
from typing import Optional

from fastapi import HTTPException, UploadFile, status
from fastapi.responses import StreamingResponse

CHUNK_READ_SIZE = 65536
SAFE_FILENAME_RE = re.compile(r"[^A-Za-z0-9._+-]")
RANGE_HEADER_RE = re.compile(r"^bytes=(\d*)-(\d*)$")


def sanitize_filename(name: Optional[str], fallback: str = "package.bin") -> str:
    if not name:
        return fallback
    base = Path(name).name.strip()
    cleaned = SAFE_FILENAME_RE.sub("_", base)
    return cleaned or fallback


class PackageStorageService:
    """Handles streaming package uploads, chunked uploads, SHA-256 digests, and Range responses."""

    def __init__(self, data_dir: Path) -> None:
        self.packages_dir = data_dir / "packages"
        self.chunks_dir = self.packages_dir / "_chunks"

    def ensure_dirs(self) -> None:
        self.packages_dir.mkdir(parents=True, exist_ok=True)
        self.chunks_dir.mkdir(parents=True, exist_ok=True)

    async def save_full_upload(
        self, package_id: str, upload: UploadFile
    ) -> tuple[Path, str, int, str]:
        """Streams a multipart UploadFile directly to disk and returns (path, filename, size, sha256)."""
        self.ensure_dirs()
        filename = sanitize_filename(upload.filename, f"{package_id}.bin")
        pkg_dir = self.packages_dir / package_id
        pkg_dir.mkdir(parents=True, exist_ok=True)
        target_path = pkg_dir / filename
        temp_path = pkg_dir / f".{filename}.uploading"

        digest = hashlib.sha256()
        total_bytes = 0
        with temp_path.open("wb") as out_f:
            while True:
                chunk = await upload.read(CHUNK_READ_SIZE)
                if not chunk:
                    break
                out_f.write(chunk)
                digest.update(chunk)
                total_bytes += len(chunk)

        temp_path.replace(target_path)
        return target_path, filename, total_bytes, digest.hexdigest()

    async def save_chunk(
        self,
        package_id: str,
        chunk_index: int,
        total_chunks: int,
        upload: UploadFile,
        explicit_filename: Optional[str] = None,
    ) -> tuple[bool, Optional[Path], Optional[str], Optional[int], Optional[str]]:
        """Stores a single chunk; when all chunks [0..total_chunks-1] exist, assembles final file."""
        if total_chunks < 1 or chunk_index < 0 or chunk_index >= total_chunks:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail="Invalid chunk_index or total_chunks",
            )
        self.ensure_dirs()
        staging = self.chunks_dir / package_id
        staging.mkdir(parents=True, exist_ok=True)

        meta_name_path = staging / "filename.txt"
        chosen_name = explicit_filename or upload.filename
        if chosen_name and chosen_name not in ("blob", "chunk"):
            meta_name_path.write_text(sanitize_filename(chosen_name), encoding="utf-8")

        part_path = staging / f"chunk_{chunk_index:06d}.part"
        with part_path.open("wb") as out_f:
            while True:
                data = await upload.read(CHUNK_READ_SIZE)
                if not data:
                    break
                out_f.write(data)

        expected_parts = [staging / f"chunk_{i:06d}.part" for i in range(total_chunks)]
        if not all(p.exists() for p in expected_parts):
            return False, None, None, None, None

        final_filename = (
            meta_name_path.read_text(encoding="utf-8").strip()
            if meta_name_path.exists()
            else sanitize_filename(chosen_name, f"{package_id}.bin")
        )
        pkg_dir = self.packages_dir / package_id
        pkg_dir.mkdir(parents=True, exist_ok=True)
        final_path = pkg_dir / final_filename
        temp_assembled = pkg_dir / f".{final_filename}.assembling"

        digest = hashlib.sha256()
        total_bytes = 0
        with temp_assembled.open("wb") as out_f:
            for p in expected_parts:
                with p.open("rb") as in_f:
                    while True:
                        buf = in_f.read(CHUNK_READ_SIZE)
                        if not buf:
                            break
                        out_f.write(buf)
                        digest.update(buf)
                        total_bytes += len(buf)

        temp_assembled.replace(final_path)
        shutil.rmtree(staging, ignore_errors=True)
        return True, final_path, final_filename, total_bytes, digest.hexdigest()

    def delete_package_files(self, package_id: str) -> None:
        pkg_dir = self.packages_dir / package_id
        staging = self.chunks_dir / package_id
        shutil.rmtree(pkg_dir, ignore_errors=True)
        shutil.rmtree(staging, ignore_errors=True)


def parse_byte_range(range_header: str, file_size: int) -> tuple[int, int]:
    """Parses HTTP Range header (`bytes=start-end`, `bytes=start-`, `bytes=-suffix`)."""
    if file_size <= 0:
        raise ValueError("Empty file cannot satisfy byte range")
    match = RANGE_HEADER_RE.match(range_header.strip())
    if not match:
        raise ValueError("Invalid Range header syntax")
    start_str, end_str = match.groups()
    if start_str == "" and end_str == "":
        raise ValueError("Empty byte range bounds")

    if start_str == "":
        # Suffix byte range: bytes=-N
        suffix_len = int(end_str)
        if suffix_len <= 0:
            raise ValueError("Invalid suffix length")
        start = max(0, file_size - suffix_len)
        end = file_size - 1
    else:
        start = int(start_str)
        end = int(end_str) if end_str != "" else file_size - 1
        if end >= file_size:
            end = file_size - 1

    if start < 0 or start > end or start >= file_size:
        raise ValueError("Range out of bounds")
    return start, end


async def iter_file_range(
    path: Path, start: int, end: int, chunk_size: int = CHUNK_READ_SIZE
) -> AsyncIterator[bytes]:
    remaining = end - start + 1
    with path.open("rb") as f:
        f.seek(start)
        while remaining > 0:
            to_read = min(chunk_size, remaining)
            data = f.read(to_read)
            if not data:
                break
            remaining -= len(data)
            yield data


def build_range_file_response(
    path: Path,
    filename: str,
    sha256: str,
    range_header: Optional[str],
) -> StreamingResponse:
    if not path.exists() or not path.is_file():
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Package binary not found on server",
        )
    file_size = path.stat().st_size
    common_headers = {
        "Accept-Ranges": "bytes",
        "X-Package-SHA256": sha256,
        "Content-Disposition": f'attachment; filename="{sanitize_filename(filename)}"',
    }

    if not range_header:
        common_headers["Content-Length"] = str(file_size)
        if file_size == 0:
            return StreamingResponse(
                iter([b""]),
                status_code=status.HTTP_200_OK,
                media_type="application/octet-stream",
                headers=common_headers,
            )
        return StreamingResponse(
            iter_file_range(path, 0, file_size - 1),
            status_code=status.HTTP_200_OK,
            media_type="application/octet-stream",
            headers=common_headers,
        )

    try:
        start, end = parse_byte_range(range_header, file_size)
    except ValueError as exc:
        raise HTTPException(
            status_code=status.HTTP_416_RANGE_NOT_SATISFIABLE,
            detail=str(exc),
            headers={"Content-Range": f"bytes */{file_size}", "Accept-Ranges": "bytes"},
        ) from exc

    content_length = end - start + 1
    headers = {
        **common_headers,
        "Content-Range": f"bytes {start}-{end}/{file_size}",
        "Content-Length": str(content_length),
    }
    return StreamingResponse(
        iter_file_range(path, start, end),
        status_code=status.HTTP_206_PARTIAL_CONTENT,
        media_type="application/octet-stream",
        headers=headers,
    )
