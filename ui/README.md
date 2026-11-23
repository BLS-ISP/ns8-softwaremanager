# NS8-SoftwareManager — NS8 Cluster-Admin Cockpit UI (`ui/`)

Native NethServer 8 Cluster-Admin micro-frontend (`Vue 2.7` + `@nethserver/ns8-ui-lib` + `@carbon/vue`) packaged inside the rootless module image (`Containerfile` -> `/ui`) and extracted by NS8's `extract-ui` helper during `add-module` / `update-module`.

---

## Commands

```bash
npm --prefix ui install --legacy-peer-deps
npm --prefix ui run build
```

Compliance rules (`server/tests/test_ns8_ui_rules.py`) are verified automatically as part of `python -m pytest server/tests -v`.

---

## Mandatory NS8 Cockpit Rules

1. **Localization (`public/i18n/{en,de}/translation.json`):** Every translation key must exist in both `en` and `de` bundles with matching placeholder tokens.
2. **Pure NS8 `core.css` Styling:** Do **not** `@import "~carbon-components/css/carbon-components.min.css"`, do **not** set `.app-side-nav { top: 3rem }`, and do **not** bind `:active` on `<cv-side-nav-link>`. NS8's host `/cluster-admin/css/core.css` provides Carbon styles and dark sidebar navigation (`#393939`, `.current-page`).
3. **Query-Parameter Subpage Routing:** Always switch between `status`, `settings`, and `about` via `this.goToAppPage(this.instanceName, page)` (`?page=...`) paired with `initUrlBindingForApp`.
4. **Carbon Icons:** Every `<Icon20 />` component used in templates must be explicitly imported from `@carbon/icons-vue/es/...` and registered in `components` (or `NS8Icons` mixin).

For details on how `ui/` relates to the standalone Vue 3 Admin Console (`../web/`), see [../web/README.md](../web/README.md) and [ADR-002](../docs/decisions/ADR-002-dual-ui-ns8-cockpit-and-standalone-console.md).
