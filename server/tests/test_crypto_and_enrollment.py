from __future__ import annotations

import base64
import time
from datetime import timedelta
from pathlib import Path

import httpx
import pytest
from sqlalchemy import select

from app.crypto import SigningService
from app.models import EnrollmentToken, utc_now


def test_ed25519_keypair_persistence_and_tamper_detection(tmp_path: Path) -> None:
    svc1 = SigningService(tmp_path)
    svc1.ensure_keys()

    priv_bytes = (tmp_path / "keys" / "private.key").read_bytes()
    pub_bytes = (tmp_path / "keys" / "public.key").read_bytes()
    assert len(priv_bytes) == 32
    assert len(pub_bytes) == 32
    pub_b64 = svc1.public_key_b64()
    assert len(base64.b64decode(pub_b64)) == 32

    # Reload from disk and verify deterministic identity
    svc2 = SigningService(tmp_path)
    assert svc2.public_key_b64() == pub_b64

    now_ts = int(time.time())
    exp = now_ts + 3600
    sig = svc1.sign_job(
        job_id="job-1",
        device_id="dev-1",
        job_type="software",
        action="install",
        target_id="pkg-1",
        version="1.2.3",
        sha256="a" * 64,
        command="/qn",
        expires_at_unix=exp,
    )
    assert len(base64.b64decode(sig)) == 64

    assert svc2.verify_job(
        job_id="job-1",
        device_id="dev-1",
        job_type="software",
        action="install",
        target_id="pkg-1",
        version="1.2.3",
        sha256="a" * 64,
        command="/qn",
        expires_at_unix=exp,
        signature_b64=sig,
        now_unix=now_ts,
    )

    # Tampered command MUST fail verification
    assert not svc2.verify_job(
        job_id="job-1",
        device_id="dev-1",
        job_type="software",
        action="install",
        target_id="pkg-1",
        version="1.2.3",
        sha256="a" * 64,
        command="/qn && calc.exe",
        expires_at_unix=exp,
        signature_b64=sig,
        now_unix=now_ts,
    )

    # Expired job MUST fail verification
    assert not svc2.verify_job(
        job_id="job-1",
        device_id="dev-1",
        job_type="software",
        action="install",
        target_id="pkg-1",
        version="1.2.3",
        sha256="a" * 64,
        command="/qn",
        expires_at_unix=exp,
        signature_b64=sig,
        now_unix=exp + 1,
    )

    # Catalog item signing & tamper check
    cat_sig = svc1.sign_catalog_item(
        package_id="pkg-9",
        device_id="dev-1",
        source_type="winget",
        package_ref="Mozilla.Firefox",
        version="132.0",
        sha256="",
        install_cmd="--silent",
        uninstall_cmd="",
        allow_uninstall=False,
    )
    assert svc2.verify_catalog_item(
        package_id="pkg-9",
        device_id="dev-1",
        source_type="winget",
        package_ref="Mozilla.Firefox",
        version="132.0",
        sha256="",
        install_cmd="--silent",
        uninstall_cmd="",
        allow_uninstall=False,
        signature_b64=cat_sig,
    )
    # Flipping allow_uninstall from False (0) to True (1) MUST fail verification
    assert not svc2.verify_catalog_item(
        package_id="pkg-9",
        device_id="dev-1",
        source_type="winget",
        package_ref="Mozilla.Firefox",
        version="132.0",
        sha256="",
        install_cmd="--silent",
        uninstall_cmd="",
        allow_uninstall=True,
        signature_b64=cat_sig,
    )


def test_rfc8032_and_rust_seed_cross_language_vectors(tmp_path: Path) -> None:
    # Exact RFC 8032 Section 7.1 TEST 1 vector matching Rust swm-common/src/crypto.rs
    secret_bytes = bytes.fromhex(
        "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"
    )
    expected_pub_bytes = bytes.fromhex(
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
    )
    expected_sig_bytes = bytes.fromhex(
        "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e0652249015"
        "55fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
    )
    keys_dir = tmp_path / "keys"
    keys_dir.mkdir(parents=True, exist_ok=True)
    (keys_dir / "private.key").write_bytes(secret_bytes)
    (keys_dir / "public.key").write_bytes(expected_pub_bytes)

    svc = SigningService(tmp_path)
    assert svc.public_key_b64() == base64.b64encode(expected_pub_bytes).decode("ascii")
    assert svc.sign_canonical("") == base64.b64encode(expected_sig_bytes).decode("ascii")


@pytest.mark.asyncio
async def test_enrollment_tokens_and_bootstrap_scripts(
    client: httpx.AsyncClient,
    admin_headers: dict[str, str],
    test_app,
) -> None:
    create_resp = await client.post(
        "/api/v1/admin/enrollment-tokens",
        headers=admin_headers,
        json={
            "name": "Office Rollout",
            "expires_in_hours": 12,
            "max_uses": 1,
            "auto_approve": False,
        },
    )
    assert create_resp.status_code == 200
    tok_data = create_resp.json()
    raw_token = tok_data["token"]
    assert "install.ps1?token=" in tok_data["install_ps1_cmd"]
    assert "install.sh?token=" in tok_data["install_sh_cmd"]

    # Fetch PowerShell & Bash bootstrap scripts
    ps1 = await client.get(f"/dl/install.ps1?token={raw_token}&headless=1")
    assert ps1.status_code == 200
    assert raw_token in ps1.text
    assert "$Headless = 1" in ps1.text

    sh = await client.get(f"/dl/install.sh?token={raw_token}&headless=0")
    assert sh.status_code == 200
    assert "\r\n" not in sh.text
    assert f'ENROLLMENT_TOKEN="{raw_token}"' in sh.text

    # Reject script injection in token query param
    bad_ps1 = await client.get('/dl/install.ps1?token=";Remove-Item')
    assert bad_ps1.status_code == 400

    # First enrollment succeeds (max_uses=1, auto_approve=False -> status="pending")
    enr1 = await client.post(
        "/api/v1/agent/enroll",
        json={
            "enrollment_token": raw_token,
            "hostname": "pc-01",
            "os_family": "windows",
            "os_name": "Windows 11",
            "os_version": "23H2",
            "arch": "amd64",
            "agent_version": "1.0.0",
            "machine_uid": "uid-pc-01",
        },
    )
    assert enr1.status_code == 200
    assert enr1.json()["status"] == "pending"
    assert len(base64.b64decode(enr1.json()["server_public_key_b64"])) == 32

    # Second enrollment fails because max_uses == 1
    enr2 = await client.post(
        "/api/v1/agent/enroll",
        json={
            "enrollment_token": raw_token,
            "hostname": "pc-02",
            "os_family": "linux",
            "machine_uid": "uid-pc-02",
        },
    )
    assert enr2.status_code == 401

    # Expired token rejection
    exp_resp = await client.post(
        "/api/v1/admin/enrollment-tokens",
        headers=admin_headers,
        json={"name": "Expired Token", "expires_in_hours": 1},
    )
    exp_id = exp_resp.json()["id"]
    exp_tok = exp_resp.json()["token"]
    async with test_app.state.db.session_factory() as session:
        res = await session.execute(
            select(EnrollmentToken).where(EnrollmentToken.id == exp_id)
        )
        row = res.scalar_one()
        row.expires_at = utc_now() - timedelta(minutes=5)
        await session.commit()

    enr_exp = await client.post(
        "/api/v1/agent/enroll",
        json={
            "enrollment_token": exp_tok,
            "hostname": "pc-03",
            "os_family": "linux",
            "machine_uid": "uid-pc-03",
        },
    )
    assert enr_exp.status_code == 401

    # Revoke/delete token -> enrollment rejected
    del_resp = await client.delete(
        f"/api/v1/admin/enrollment-tokens/{tok_data['id']}",
        headers=admin_headers,
    )
    assert del_resp.status_code == 200
