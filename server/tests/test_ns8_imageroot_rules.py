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
