from __future__ import annotations

import json
import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
UI_DIR = REPO_ROOT / "ui"


def _resolve_nested_key(data: dict, dotted_key: str):
    cur = data
    for part in dotted_key.split("."):
        if not isinstance(cur, dict) or part not in cur:
            return None
        cur = cur[part]
    return cur


def test_ns8_ui_rule_1_notification_api_usage() -> None:
    """UI Rule 1: Zero calls to non-existent createSuccessNotificationForApp; 2-arg createErrorNotificationForApp."""
    call_re = re.compile(r"createErrorNotificationForApp\s*\(([^)]*)\)")
    vue_and_js = list((UI_DIR / "src").rglob("*.vue")) + list(
        (UI_DIR / "src").rglob("*.js")
    )
    assert len(vue_and_js) >= 5

    found_error_calls = 0
    for path in vue_and_js:
        txt = path.read_text(encoding="utf-8")
        assert (
            "createSuccessNotificationForApp" not in txt
        ), f"Forbidden createSuccessNotificationForApp in {path}"
        for match in call_re.finditer(txt):
            args_str = match.group(1).strip()
            # Must pass 2 arguments: (err, message)
            assert "," in args_str, f"1-arg createErrorNotificationForApp in {path}"
            found_error_calls += 1

    assert found_error_calls >= 1


def test_ns8_ui_rule_2_carbon_icons_registered() -> None:
    """UI Rule 2: Every @carbon/icons-vue icon used in <template> is registered in components / data()."""
    import_re = re.compile(
        r'import\s+([A-Za-z0-9_]+)\s+from\s+["\']@carbon/icons-vue/[^"\']+["\']'
    )
    tag_icon_re = re.compile(r"<([A-Z][A-Za-z0-9]*(?:16|20|24|32))\b")
    prop_icon_re = re.compile(r":icon=\"([A-Z][A-Za-z0-9]*(?:16|20|24|32))\"")

    for vue_path in (UI_DIR / "src").rglob("*.vue"):
        txt = vue_path.read_text(encoding="utf-8")
        tpl_match = re.search(r"<template>(.*)</template>", txt, flags=re.DOTALL)
        script_match = re.search(r"<script>(.*)</script>", txt, flags=re.DOTALL)
        if not tpl_match or not script_match:
            continue

        tpl = tpl_match.group(1)
        script = script_match.group(1)
        imported_icons = set(import_re.findall(script))

        comp_block_match = re.search(
            r"components\s*:\s*\{([^}]*)\}", script, flags=re.DOTALL
        )
        comp_block = comp_block_match.group(1) if comp_block_match else ""

        for tag_icon in set(tag_icon_re.findall(tpl)):
            assert (
                tag_icon in imported_icons
            ), f"{vue_path.name} uses <{tag_icon}> without importing it"
            assert (
                tag_icon in comp_block
            ), f"{vue_path.name} uses <{tag_icon}> without registering in components"

        for prop_icon in set(prop_icon_re.findall(tpl)):
            assert (
                prop_icon in imported_icons
            ), f"{vue_path.name} binds :icon=\"{prop_icon}\" without importing it"
            assert (
                prop_icon in script
            ), f"{vue_path.name} binds :icon=\"{prop_icon}\" without exposing it in script"


def test_ns8_ui_rule_3_i18n_locales_and_mandatory_keys() -> None:
    """UI Rule 3: main.js configures de + en with fallbackLocale 'en', and mandatory NS8 keys exist."""
    main_js = (UI_DIR / "src" / "main.js").read_text(encoding="utf-8")
    assert 'fallbackLocale: "en"' in main_js or "fallbackLocale: 'en'" in main_js
    assert "de:" in main_js or '"de"' in main_js
    assert "en:" in main_js or '"en"' in main_js

    mandatory_keys = [
        "action.list-installed-modules",
        "task.cannot_create_task",
        "error.cannot_retrieve_installed_modules",
        "error.cannot_retrieve_module_info",
    ]
    for lang in ("de", "en"):
        trans_path = UI_DIR / "public" / "i18n" / lang / "translation.json"
        assert trans_path.is_file(), f"Missing {trans_path}"
        data = json.loads(trans_path.read_text(encoding="utf-8"))
        for key in mandatory_keys:
            val = _resolve_nested_key(data, key)
            assert isinstance(val, str) and val.strip(), (
                f"Missing mandatory NS8 i18n key '{key}' in {lang}/translation.json"
            )


def test_ns8_ui_rule_4_iframe_navigation_and_query_param_routing() -> None:
    """UI Rule 4: Never push `/apps/<id>/<subpage>` on parent router; use 2-arg goToAppPage(instanceName, page) + changeRoute."""
    app_vue = (UI_DIR / "src" / "App.vue").read_text(encoding="utf-8")
    side_menu = (UI_DIR / "src" / "components" / "AppSideMenuContent.vue").read_text(
        encoding="utf-8"
    )
    status_vue = (UI_DIR / "src" / "views" / "Status.vue").read_text(encoding="utf-8")

    assert 'addEventListener("changeRoute"' in app_vue or "addEventListener('changeRoute'" in app_vue
    assert "getQueryParamsForApp()" in app_vue

    # AppSideMenuContent must delegate to ns8-ui-lib's 2-arg goToAppPage(instanceName, page)
    assert "goToAppPage(this.instanceName, page)" in side_menu
    assert "goToAppPage(path)" not in side_menu

    # Status.vue must navigate via goToAppPage(this.instanceName, "settings"), never `/apps/.../settings`
    assert 'goToAppPage(this.instanceName, "settings")' in status_vue
    for vue_path in (UI_DIR / "src").rglob("*.vue"):
        txt = vue_path.read_text(encoding="utf-8")
        assert "/settings`" not in txt, (
            f"{vue_path.name} pushes invalid parent sub-route `/apps/.../settings` instead of `?page=settings`"
        )


def test_ns8_ui_rule_5_pure_ns8_core_css_no_carbon_override() -> None:
    """UI Rule 5: Do not re-import carbon-components.min.css over /cluster-admin/css/core.css or bind :active on cv-side-nav-link."""
    index_html = (UI_DIR / "public" / "index.html").read_text(encoding="utf-8")
    assert "/cluster-admin/css/core.css" in index_html
    assert 'id="ns8-app"' in index_html

    for asset in list((UI_DIR / "src").rglob("*.vue")) + list(
        (UI_DIR / "src").rglob("*.scss")
    ) + list((UI_DIR / "src").rglob("*.js")):
        txt = asset.read_text(encoding="utf-8")
        assert "carbon-components.min.css" not in txt, (
            f"{asset.name} imports carbon-components.min.css and overrides NS8 core.css"
        )

    side_menu_content = (
        UI_DIR / "src" / "components" / "AppSideMenuContent.vue"
    ).read_text(encoding="utf-8")
    assert ":active=" not in side_menu_content
    assert "current-page" in side_menu_content
