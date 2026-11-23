# ADR-002: Dual-UI Architecture — Pure NS8 Cockpit Module (`ui/`) + Standalone Admin Console (`web/`)

## Status
Accepted

## Date
2026-11-23

## Context
NethServer 8 modules expose configuration screens inside the NS8 Cluster-Admin shell (`/cluster-admin/#/apps/<instance>`), which embeds a Vue 2 + Carbon Design System (`@nethserver/ns8-ui-lib`) micro-frontend in an `<iframe>` styled by `/cluster-admin/css/core.css`.

Meanwhile, daily endpoint management (real-time WebSocket job logs, chunked multi-hundred-MB installer uploads, fleet multi-select bulk toolbars, patch compliance matrices, and AD/LDAP operator logins for IT staff who are *not* full NS8 cluster root administrators) benefits from a dedicated application hosted on its own Traefik FQDN (e.g. `https://swm.example.com`).

Early mixing of custom Carbon CSS imports (`carbon-components.min.css`), `top: 3rem` sidebar offsets, and custom component styling inside `ui/` caused visual collisions with NS8's `/cluster-admin/css/core.css` (a white `48px` gap above the instance sidebar and unreadable white-on-white active navigation items).

## Decision
Maintain a strict **two-UI boundary**:
1. **NS8 Cluster-Admin Module UI (`ui/` — Vue 2 + `@nethserver/ns8-ui-lib`):**
   - Strictly handles NS8 cluster lifecycle concerns (`Status`, `Settings` for FQDN / Let's Encrypt / NS8 `Ldapproxy` user-domain binding, and `About`, localized in `en` & `de`) plus quick links to launch the Admin Console and download bootstrap binaries.
   - Uses **pure native NS8 `core.css`** (`cv-grid`, `NsInfoCard`, `NsTag`, `NsInlineNotification`, `cv-structured-list`, `NsEmptyState`) with zero duplicate `carbon-components.min.css` imports and zero iframe top-offset overrides, enforced by automated tests in `server/tests/test_ns8_ui_rules.py`.
2. **Standalone Admin Web Console (`web/` — Vue 3 + TypeScript + Vite + TailwindCSS):**
   - Served by the FastAPI app container behind NS8 Traefik on its dedicated FQDN (`https://swm.example.com`) with Light & Dark mode, NS8 AD/OpenLDAP login, and full fleet/patch/catalog workflows.

## Alternatives Considered

### Embedding the Entire Endpoint Management Console Inside NS8 Cockpit (`ui/`)
- **Rejected:** Requires granting every helpdesk/IT operator full NS8 Cluster-Admin access, constrains large chunked uploads and WebSockets to Cockpit's API bridge, and locks a complex SPA to legacy Vue 2.7 tooling.

### Custom-Theming `ui/` to Match `web/`
- **Rejected:** Visually clashes with every other module inside NS8 Cluster-Admin and breaks NS8's dark sidebar rules in `/cluster-admin/css/core.css`.

## Consequences
- Inside NS8 Cluster-Admin, `SoftwareManager` looks and behaves identically to official Nethesis / BLS-ISP NS8 modules.
- IT operators get a fast, purpose-built Vue 3 console on `https://swm.example.com` authenticated via their NS8 Active Directory or OpenLDAP domain without needing cluster-root privileges.
