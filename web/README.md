# NS8-SoftwareManager — Web Consoles (`web/` & `ui/`)

NS8-SoftwareManager intentionally separates its two browser interfaces so each adheres cleanly to its host environment (see [ADR-002](../docs/decisions/ADR-002-dual-ui-ns8-cockpit-and-standalone-console.md)):

| Directory | Stack | Purpose & URL |
|---|---|---|
| **`web/`** (this folder) | **Vue 3 + TypeScript + Vite + TailwindCSS** | Full daily IT administration console served directly at the configured Traefik FQDN (e.g. `https://swm.example.com`): Fleet & bulk controls, 5-tab client rollout (`GPO`, `Ansible`, `macOS MDM`), OS patching (`WUA`, `APT`/`DNF`, `softwareupdate`), software catalog (`WinGet`, `APT`/`DNF`/`Flatpak`, `Homebrew`, custom uploads), deployments, maintenance windows, and desktop client policies. |
| **`../ui/`** | **Vue 2 + `@nethserver/ns8-ui-lib` + Carbon** | Native NethServer 8 Cluster-Admin module micro-frontend (`Status`, `Settings`, `About`, localized in `en` & `de`) embedded inside NS8's `/cluster-admin/` iframe for configuring Traefik routing, Let's Encrypt, NS8 LDAP/AD domain binding, and viewing pod/backup status. |

---

## 1. Standalone Admin Web Console (`web/`)

```bash
npm --prefix web install

# Run Vitest component & router suite:
npm --prefix web test -- --run

# Start Vite dev server (proxies /api and /dl to 127.0.0.1:8080):
npm --prefix web run dev

# Type-check (vue-tsc) and build production bundle into web/dist/:
npm --prefix web run build
```

---

## 2. NS8 Cluster-Admin Cockpit UI (`../ui/`)

```bash
npm --prefix ui install --legacy-peer-deps
npm --prefix ui run build
```

### Critical NS8 Cockpit Styling & Routing Gotchas (Enforced by `server/tests/test_ns8_ui_rules.py`)
1. **Pure NS8 `core.css` — Never Import `carbon-components.min.css`:**
   Inside NS8 Cluster-Admin, `<link rel="stylesheet" href="/cluster-admin/css/core.css">` already provides Carbon styles plus NS8's dark sidebar overrides (`#393939`, `.current-page`). Importing `~carbon-components/css/carbon-components.min.css` or binding `:active` on `<cv-side-nav-link>` overrides NS8's dark theme with white-on-white active links.
2. **Iframe Top Offset (`top: 0`, Never `top: 3rem`):**
   NS8 renders module UIs inside an `<iframe>` that is already positioned below NS8's `48px` top header bar. Never offset `.app-side-nav` by `top: 3rem`, or a white gap will appear above the instance name.
3. **Subpage Navigation via `goToAppPage` (`?page=settings`):**
   Always navigate between `status`, `settings`, and `about` using `@nethserver/ns8-ui-lib`'s `goToAppPage(instanceName, page)` query-parameter convention (`?page=...`), never raw parent router path pushes.
4. **Vue 2 `NsInfoCard` Non-Reactive `$slots.content`:**
   In `@nethserver/ns8-ui-lib`, `NsInfoCard` evaluates `!!this.$slots.content` non-reactively on mount. Always place an unconditional wrapper `<div>` inside `<template slot="content">` and put `v-if` conditions *inside* that wrapper.
