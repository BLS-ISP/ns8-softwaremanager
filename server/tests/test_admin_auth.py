from __future__ import annotations

import httpx
import pytest

from app.config import Settings


@pytest.mark.asyncio
async def test_local_admin_login_and_me(
    client: httpx.AsyncClient, test_settings: Settings
) -> None:
    # Unauthenticated calls to /api/v1/admin/* must be rejected with 401
    for path in (
        "/api/v1/admin/auth/me",
        "/api/v1/admin/dashboard/summary",
        "/api/v1/admin/devices",
        "/api/v1/admin/groups",
        "/api/v1/admin/catalog",
    ):
        unauth = await client.get(path)
        assert unauth.status_code == 401, f"Expected 401 on {path}"

    # Wrong password -> 401
    bad = await client.post(
        "/api/v1/admin/auth/login",
        json={
            "username": test_settings.ADMIN_USERNAME,
            "password": "wrong-password",
            "auth_source": "local",
        },
    )
    assert bad.status_code == 401

    # Valid local login -> 200 + JWT
    good = await client.post(
        "/api/v1/admin/auth/login",
        json={
            "username": test_settings.ADMIN_USERNAME,
            "password": test_settings.ADMIN_PASSWORD,
            "auth_source": "local",
        },
    )
    assert good.status_code == 200
    body = good.json()
    assert body["token_type"] == "bearer"
    assert body["user"]["username"] == "admin"
    assert body["user"]["auth_source"] == "local"

    me = await client.get(
        "/api/v1/admin/auth/me",
        headers={"Authorization": f"Bearer {body['access_token']}"},
    )
    assert me.status_code == 200
    assert me.json()["username"] == "admin"

    # Tampered token -> 401
    tampered = await client.get(
        "/api/v1/admin/auth/me",
        headers={"Authorization": f"Bearer {body['access_token']}tampered"},
    )
    assert tampered.status_code == 401


@pytest.mark.asyncio
async def test_ldap_login_mocked_and_unconfigured(
    client: httpx.AsyncClient,
    test_settings: Settings,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # Unconfigured LDAP_HOST -> 400
    unconf = await client.post(
        "/api/v1/admin/auth/login",
        json={"username": "jdoe", "password": "secret", "auth_source": "ldap"},
    )
    assert unconf.status_code == 400

    # Configure LDAP settings and mock authenticate_ldap
    test_settings.LDAP_HOST = "127.0.0.1"
    test_settings.LDAP_BASE_DN = "dc=nethserver,dc=test"

    async def fake_ldap_auth(username: str, password: str, settings: Settings):
        if username == "domain.admin" and password == "AdPass!2026":
            return {
                "username": "domain.admin",
                "dn": "cn=domain.admin,dc=nethserver,dc=test",
                "role": "admin",
            }
        return None

    monkeypatch.setattr("app.api.admin_auth.authenticate_ldap", fake_ldap_auth)

    fail_ldap = await client.post(
        "/api/v1/admin/auth/login",
        json={"username": "domain.admin", "password": "wrong", "auth_source": "ldap"},
    )
    assert fail_ldap.status_code == 401

    ok_ldap = await client.post(
        "/api/v1/admin/auth/login",
        json={
            "username": "domain.admin",
            "password": "AdPass!2026",
            "auth_source": "ldap",
        },
    )
    assert ok_ldap.status_code == 200
    data = ok_ldap.json()
    assert data["user"]["username"] == "domain.admin"
    assert data["user"]["auth_source"] == "ldap"

    me = await client.get(
        "/api/v1/admin/auth/me",
        headers={"Authorization": f"Bearer {data['access_token']}"},
    )
    assert me.status_code == 200
    assert me.json()["auth_source"] == "ldap"


def test_ldap3_client_bind_flow_escaping_and_blank_password_guard(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    from types import SimpleNamespace
    from app.auth.ldap_provider import _sync_ldap_authenticate

    # Empty systemd env strings normalize cleanly
    cfg = Settings(
        LDAP_HOST="127.0.0.1",
        LDAP_PORT="",  # type: ignore[arg-type]
        LDAP_USE_SSL="",  # type: ignore[arg-type]
        LDAP_BASE_DN="dc=ad,dc=example,dc=org",
        LDAP_BIND_DN="cn=svc,dc=ad,dc=example,dc=org",
        LDAP_BIND_PASSWORD="svc-password",
    )
    assert cfg.LDAP_PORT == 389
    assert cfg.LDAP_USE_SSL is False

    # Blank password MUST return None without attempting anonymous LDAP bind
    assert _sync_ldap_authenticate("alice", "", cfg) is None

    captured_filters: list[str] = []

    class FakeConn:
        def __init__(self, _server, user=None, password=None, **_kw):
            self.user = user
            self.password = password
            self.bound = password in ("svc-password", "UserGoodPass!")
            self.entries = []

        def __enter__(self):
            return self

        def __exit__(self, *args):
            return False

        def search(self, search_base, search_filter, **_kw):
            captured_filters.append(search_filter)
            self.entries = [
                SimpleNamespace(
                    entry_dn="cn=alice,dc=ad,dc=example,dc=org",
                    uid=SimpleNamespace(value="alice"),
                )
            ]

    monkeypatch.setattr("app.auth.ldap_provider.Server", lambda *a, **kw: object())
    monkeypatch.setattr("app.auth.ldap_provider.Connection", FakeConn)

    ok = _sync_ldap_authenticate("alice*)(|(uid=*", "UserGoodPass!", cfg)
    assert ok is not None
    assert ok["username"] == "alice"
    # Verify filter metacharacters were escaped (`\2a`, `\28`, `\29`)
    assert r"\2a" in captured_filters[0]
    assert _sync_ldap_authenticate("alice", "WrongPass", cfg) is None
