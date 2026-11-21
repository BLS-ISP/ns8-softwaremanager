from __future__ import annotations

import hashlib

import httpx
import pytest

from tests.conftest import enroll_test_device


@pytest.mark.asyncio
async def test_chunked_upload_sha256_and_http_range_206_downloads(
    client: httpx.AsyncClient, admin_headers: dict[str, str]
) -> None:
    dev = await enroll_test_device(
        client, admin_headers, hostname="range-client", os_family="windows"
    )
    dev_headers = {"Authorization": f"Bearer {dev['device_token']}"}

    pkg = (
        await client.post(
            "/api/v1/admin/catalog",
            headers=admin_headers,
            json={
                "name": "Custom ERP Suite",
                "version": "5.4.0",
                "vendor": "ACME Corp",
                "os_family": "windows",
                "source_type": "custom",
                "install_args": "/qn /norestart",
                "uninstall_args": "/x {11111111-2222-3333-4444-555555555555} /qn",
                "return_codes": {"0": "success", "3010": "reboot_required"},
                "detection_rule": {
                    "type": "registry",
                    "key": "HKLM\\SOFTWARE\\ACME\\ERP",
                    "value": "Version",
                    "version": "5.4.0",
                },
            },
        )
    ).json()
    pkg_id = pkg["id"]

    # Build a deterministic 250 KB payload split into 3 chunks
    part0 = b"A" * 100_000
    part1 = b"B" * 100_000
    part2 = b"C" * 56_000
    full_payload = part0 + part1 + part2
    expected_sha256 = hashlib.sha256(full_payload).hexdigest()

    for idx, part in enumerate((part0, part1, part2)):
        c_resp = await client.post(
            f"/api/v1/admin/catalog/{pkg_id}/upload-chunk",
            headers=admin_headers,
            data={
                "chunk_index": str(idx),
                "total_chunks": "3",
                "filename": "acme-erp-5.4.0.msi",
            },
            files={"file": ("chunk.part", part, "application/octet-stream")},
        )
        assert c_resp.status_code == 200, c_resp.text
        if idx < 2:
            assert c_resp.json()["completed"] is False
        else:
            body = c_resp.json()
            assert body["completed"] is True
            assert body["sha256"] == expected_sha256
            assert body["file_size"] == len(full_payload)
            assert body["catalog_item"]["filename"] == "acme-erp-5.4.0.msi"

    # Full download -> 200 OK + X-Package-SHA256 + Accept-Ranges: bytes
    dl_full = await client.get(
        f"/api/v1/agent/packages/{pkg_id}/download", headers=dev_headers
    )
    assert dl_full.status_code == 200
    assert dl_full.headers["Accept-Ranges"] == "bytes"
    assert dl_full.headers["X-Package-SHA256"] == expected_sha256
    assert dl_full.content == full_payload

    # Resumable HTTP Range slice 1: bytes=0-99999 -> 206 Partial Content
    dl_r1 = await client.get(
        f"/api/v1/agent/packages/{pkg_id}/download",
        headers={**dev_headers, "Range": "bytes=0-99999"},
    )
    assert dl_r1.status_code == 206
    assert dl_r1.headers["Content-Range"] == f"bytes 0-99999/{len(full_payload)}"
    assert dl_r1.content == part0

    # Resumable HTTP Range slice 2 (open-ended): bytes=100000- -> 206 Partial Content
    dl_r2 = await client.get(
        f"/api/v1/agent/packages/{pkg_id}/download",
        headers={**dev_headers, "Range": "bytes=100000-"},
    )
    assert dl_r2.status_code == 206
    assert (
        dl_r2.headers["Content-Range"]
        == f"bytes 100000-{len(full_payload) - 1}/{len(full_payload)}"
    )
    assert dl_r1.content + dl_r2.content == full_payload

    # Out-of-bounds Range -> 416 Range Not Satisfiable
    dl_416 = await client.get(
        f"/api/v1/agent/packages/{pkg_id}/download",
        headers={**dev_headers, "Range": f"bytes={len(full_payload) + 500}-"},
    )
    assert dl_416.status_code == 416

    # Also test single-shot multipart /upload endpoint
    single_bytes = b"MSI-SINGLE-SHOT-INSTALLER-CONTENT"
    up_single = await client.post(
        f"/api/v1/admin/catalog/{pkg_id}/upload",
        headers=admin_headers,
        files={"file": ("single.msi", single_bytes, "application/octet-stream")},
    )
    assert up_single.status_code == 200
    assert up_single.json()["sha256"] == hashlib.sha256(single_bytes).hexdigest()
