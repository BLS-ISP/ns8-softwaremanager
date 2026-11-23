from __future__ import annotations

import json
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
IMAGEROOT_DIR = REPO_ROOT / "imageroot"
CONTAINERFILE = REPO_ROOT / "Containerfile"


def test_ns8_rule_1_no_custom_get_status() -> None:
    """Rule 1: Never override imageroot/actions/get-status; expose config via get-configuration."""
    assert not (IMAGEROOT_DIR / "actions" / "get-status").exists()
    get_cfg = IMAGEROOT_DIR / "actions" / "get-configuration" / "20read"
    assert get_cfg.is_file()


def test_ns8_rule_2_dump_env_follows_set_env() -> None:
    """Rule 2: Every script calling agent.set_env() must call agent.dump_env()."""
    required_scripts = [
        IMAGEROOT_DIR / "actions" / "create-module" / "20initialize",
        IMAGEROOT_DIR / "actions" / "configure-module" / "20configure",
        IMAGEROOT_DIR / "actions" / "restore-module" / "06copyenv",
        IMAGEROOT_DIR / "actions" / "clone-module" / "50configure",
    ]
    for script in required_scripts:
        assert script.is_file(), f"Missing {script}"
        content = script.read_text(encoding="utf-8")
        assert "agent.set_env(" in content, f"{script.name} missing agent.set_env()"
        assert "agent.dump_env()" in content, f"{script.name} missing agent.dump_env()"
        assert content.rfind("agent.dump_env()") > content.rfind("agent.set_env(")

    # Also verify across all Python action/hook scripts in imageroot
    for path in IMAGEROOT_DIR.rglob("*"):
        if path.is_file() and path.suffix != ".json":
            txt = path.read_text(encoding="utf-8")
            if "agent.set_env(" in txt:
                assert "agent.dump_env()" in txt, f"{path} calls set_env without dump_env"


def test_ns8_rule_3_traefik_hostname_collision_validation_and_schemas() -> None:
    """Rule 3: 01hostname-validation checks http_route_in_use -> validation-failed + sys.exit(2)."""
    val_script = IMAGEROOT_DIR / "actions" / "configure-module" / "01hostname-validation"
    assert val_script.is_file()
    txt = val_script.read_text(encoding="utf-8")
    assert "http_route_in_use" in txt
    assert "validation-failed" in txt
    assert "sys.exit(2)" in txt

    # Verify JSON schemas parse cleanly
    schema_files = list((IMAGEROOT_DIR / "actions").rglob("validate-*.json"))
    assert len(schema_files) >= 5
    for sf in schema_files:
        parsed = json.loads(sf.read_text(encoding="utf-8"))
        assert isinstance(parsed, dict)


def test_ns8_rules_4_5_6_pod_binding_pull_missing_and_pg_isready() -> None:
    """Rules 4–6: 127.0.0.1:${TCP_PORT}:8080 binding, --pull=missing (no --pull=always), pg_isready."""
    systemd_dir = IMAGEROOT_DIR / "systemd" / "user"
    pod_unit = (systemd_dir / "swm.service").read_text(encoding="utf-8")
    db_unit = (systemd_dir / "swm-db.service").read_text(encoding="utf-8")
    app_unit = (systemd_dir / "swm-app.service").read_text(encoding="utf-8")

    assert "127.0.0.1:${TCP_PORT}:8080" in pod_unit
    for name, unit_txt in (
        ("swm.service", pod_unit),
        ("swm-db.service", db_unit),
        ("swm-app.service", app_unit),
    ):
        assert "--pull=always" not in unit_txt, f"Forbidden --pull=always in {name}"

    assert "--pull=missing" in db_unit
    assert "--pull=missing" in app_unit
    assert "pg_isready" in app_unit

    cfile_txt = CONTAINERFILE.read_text(encoding="utf-8")
    assert "org.nethserver.images" in cfile_txt
    assert "postgres:16-alpine" in cfile_txt

    pull_hook = IMAGEROOT_DIR / "update-module.d" / "20pull_images"
    assert pull_hook.is_file()


def test_ns8_rule_7_restic_backup_restore_and_clone() -> None:
    """Rule 7: state-include.conf, module-dump-state pg_dump, restore-module, clone-module."""
    state_inc = (IMAGEROOT_DIR / "etc" / "state-include.conf").read_text(encoding="utf-8")
    assert "state/database.sql" in state_inc
    assert "state/environment" in state_inc
    assert "volumes/swm-app-data" in state_inc

    dump_script = (IMAGEROOT_DIR / "bin" / "module-dump-state").read_text(encoding="utf-8")
    assert "pg_dump" in dump_script
    assert "state/database.sql" in dump_script
    assert (IMAGEROOT_DIR / "bin" / "module-cleanup-state").is_file()
    assert (IMAGEROOT_DIR / "actions" / "restore-module" / "50configure").is_file()
    assert (IMAGEROOT_DIR / "actions" / "clone-module" / "50configure").is_file()


def test_ns8_rule_8_lf_line_endings_and_containerfile_sanitization() -> None:
    """Rule 8: Zero CRLF in imageroot/**, *.sh, Containerfile*, plus sed CRLF strip & chmod +x."""
    checked = 0
    for p in list(IMAGEROOT_DIR.rglob("*")) + list(REPO_ROOT.glob("*.sh")) + list(
        REPO_ROOT.glob("Containerfile*")
    ):
        if p.is_file():
            raw = p.read_bytes()
            assert b"\r" not in raw, f"CR/CRLF byte found in {p}"
            checked += 1
    assert checked >= 15

    cfile_txt = CONTAINERFILE.read_text(encoding="utf-8")
    assert r"sed -i 's/\r$//'" in cfile_txt
    assert "chmod +x" in cfile_txt
    # NS8 extract-ui / extract-image calls `podman create <image>` with no command;
    # a FROM scratch module image MUST define ENTRYPOINT or CMD (e.g. ENTRYPOINT ["/"]) to avoid exit 125!
    scratch_stage = cfile_txt.split("FROM scratch", 1)[1]
    assert 'ENTRYPOINT ["/"]' in scratch_stage or 'CMD ["/"]' in scratch_stage


def test_ns8_rule_9_ldapproxy_contract_and_mock_action_execution(
    tmp_path: Path, monkeypatch
) -> None:
    """Rule 9: Never import non-existent LdapDiscovery; execute NS8 actions against mock agent & Ldapproxy."""
    import io
    import os
    import runpy
    import sys
    from types import ModuleType, SimpleNamespace

    for p in IMAGEROOT_DIR.rglob("*"):
        if p.is_file():
            txt = p.read_text(encoding="utf-8", errors="replace")
            assert "LdapDiscovery" not in txt, f"Invalid LdapDiscovery reference in {p}"

    state_env: dict[str, str] = {}
    env_file = tmp_path / "environment"

    fake_agent = ModuleType("agent")
    fake_agent.SD_WARNING = "<4>"  # type: ignore[attr-defined]
    fake_agent.set_env = lambda k, v: state_env.__setitem__(str(k), str(v))  # type: ignore[attr-defined]
    fake_agent.dump_env = lambda: env_file.write_text(  # type: ignore[attr-defined]
        "\n".join(f"{k}={v}" for k, v in state_env.items()) + "\n",
        encoding="utf-8",
    )
    fake_agent.read_envfile = lambda p: dict(  # type: ignore[attr-defined]
        line.split("=", 1)
        for line in Path(p).read_text(encoding="utf-8").splitlines()
        if "=" in line
    )
    fake_agent.resolve_agent_id = lambda name: "traefik1"  # type: ignore[attr-defined]
    fake_agent.assert_exp = lambda cond: (_ for _ in ()).throw(AssertionError()) if not cond else None  # type: ignore[attr-defined]
    fake_agent.http_route_in_use = lambda domain: False  # type: ignore[attr-defined]
    fake_agent.tasks = SimpleNamespace(  # type: ignore[attr-defined]
        run=lambda **_kw: {"exit_code": 0, "output": {}}
    )

    fake_ldapproxy_mod = ModuleType("agent.ldapproxy")

    class FakeLdapproxy:
        def get_domains_list(self) -> list[str]:
            # Real NS8 Ldapproxy.get_domains_list() returns list[str] of domain names!
            return ["ad.bls-isp.net", "ldap.bls-isp.net"]

        def get_domain(self, name: str) -> dict[str, object]:
            if name == "ad.bls-isp.net":
                return {
                    "host": "127.0.0.1",
                    "port": 20015,
                    "schema": "ad",
                    "base_dn": "dc=ad,dc=bls-isp,dc=net",
                    "bind_dn": "cn=svc,dc=ad,dc=bls-isp,dc=net",
                    "bind_password": "secret",
                    "tls": True,
                }
            return {
                "host": "127.0.0.1",
                "port": 20016,
                "schema": "rfc2307",
                "base_dn": "dc=directory,dc=nh",
                "bind_dn": "",
                "bind_password": "",
                "tls": False,
            }

    fake_ldapproxy_mod.Ldapproxy = FakeLdapproxy  # type: ignore[attr-defined]
    fake_agent.ldapproxy = fake_ldapproxy_mod  # type: ignore[attr-defined]

    monkeypatch.setitem(sys.modules, "agent", fake_agent)
    monkeypatch.setitem(sys.modules, "agent.ldapproxy", fake_ldapproxy_mod)
    monkeypatch.chdir(tmp_path)
    monkeypatch.setenv("MODULE_ID", "softwaremanager2")
    monkeypatch.setenv("TCP_PORTS", "20084")
    monkeypatch.delenv("TCP_PORT", raising=False)

    # 1. Execute create-module/20initialize
    out_init = io.StringIO()
    monkeypatch.setattr(sys, "stdout", out_init)
    runpy.run_path(str(IMAGEROOT_DIR / "actions" / "create-module" / "20initialize"))
    assert json.loads(out_init.getvalue()) == {"tcp_port": 20084}
    assert state_env["TCP_PORT"] == "20084"

    # 2. Execute get-configuration/20read on unconfigured instance
    out_read1 = io.StringIO()
    monkeypatch.setattr(sys, "stdout", out_read1)
    runpy.run_path(str(IMAGEROOT_DIR / "actions" / "get-configuration" / "20read"))
    cfg1 = json.loads(out_read1.getvalue())
    assert cfg1["configured"] is False
    assert cfg1["tcp_port"] == 20084
    assert cfg1["user_domains"] == [
        {"name": "ad.bls-isp.net", "schema": "ad"},
        {"name": "ldap.bls-isp.net", "schema": "rfc2307"},
    ]

    # 3. Execute configure-module/20configure with NS8 AD user domain
    for k, v in state_env.items():
        monkeypatch.setenv(k, v)
    in_cfg = io.StringIO(
        json.dumps(
            {
                "domain_name": "swm.bls-isp.net",
                "lets_encrypt": True,
                "http2https": True,
                "ldap_domain": "ad.bls-isp.net",
                "admin_username": "admin",
                "admin_password": "NewStrongPassword123!",
            }
        )
    )
    out_cfg = io.StringIO()
    monkeypatch.setattr(sys, "stdin", in_cfg)
    monkeypatch.setattr(sys, "stdout", out_cfg)
    runpy.run_path(str(IMAGEROOT_DIR / "actions" / "configure-module" / "20configure"))
    assert json.loads(out_cfg.getvalue()) == {"app_url": "https://swm.bls-isp.net"}
    # Local 127.0.0.1 ldapproxy MUST map to 10.0.2.2 inside slirp4netns pod with plaintext LDAP
    assert state_env["LDAP_HOST"] == "10.0.2.2"
    assert state_env["LDAP_PORT"] == "20015"
    assert state_env["LDAP_TLS"] == "false"
    assert state_env["LDAP_SCHEMA"] == "ad"

    # 4. Re-read configuration after configure-module
    for k, v in state_env.items():
        monkeypatch.setenv(k, v)
    out_read2 = io.StringIO()
    monkeypatch.setattr(sys, "stdout", out_read2)
    runpy.run_path(str(IMAGEROOT_DIR / "actions" / "get-configuration" / "20read"))
    cfg2 = json.loads(out_read2.getvalue())
    assert cfg2["configured"] is True
    assert cfg2["domain_name"] == "swm.bls-isp.net"
    assert cfg2["ldap_domain"] == "ad.bls-isp.net"
    assert os.path.isfile(env_file)
