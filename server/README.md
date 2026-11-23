# NS8-SoftwareManager — Central Management Backend (`server/`)

Asynchronous Python `FastAPI` backend (`SQLAlchemy 2.0` async) powering the Admin REST API, Agent REST & WebSocket hub, chunked package repository, background maintenance-window scheduler, and `Ed25519` cryptographic signing service.

---

## Quick Start & Tests

Requires **Python 3.12+**:

```bash
python -m pip install -r server/requirements.txt

# Run the full backend + NS8 imageroot + NS8 Cockpit UI compliance suite:
python -m pytest server/tests -v

# Start local development server (uses SQLite in ./data/swm.db by default):
cd server
uvicorn app.main:app --reload --host 127.0.0.1 --port 8080
```

---

## Directory Structure

```text
server/
├── app/
│   ├── api/
│   │   ├── admin_auth.py                  # JWT login (local + LDAP/AD) & dashboard summary
│   │   ├── admin_devices_groups.py        # Devices, groups, bulk actions & signed remote controls
│   │   ├── admin_patches.py               # WUA, APT/DNF & macOS softwareupdate approval/exclusions
│   │   ├── admin_catalog_deployments.py   # Catalog CRUD, chunked uploads, deployments & self-update
│   │   ├── agent_api.py                   # Agent enroll, status, delta inventory, jobs & WSS hub
│   │   └── health_and_dl.py               # /api/v1/health, /dl/install.{ps1,sh} & static binaries
│   ├── auth/                              # JWT tokens, password hashing & NS8 LDAP/AD bind provider
│   ├── models/                            # SQLAlchemy 2.0 ORM tables (devices, groups, catalog, jobs)
│   ├── schemas/                           # Pydantic v2 request/response DTOs
│   ├── services/                          # Ed25519 signing, background scheduler & WS hub
│   ├── config.py                          # Environment settings (pydantic-settings)
│   ├── db.py                              # Async engine, backoff & idempotent ensure_schema_upgrades()
│   └── main.py                            # FastAPI lifespan, routers & SPA fallback
└── tests/                                 # Pytest integration, crypto, macOS & NS8 architecture suites
```

---

## Key Operational Details & Gotchas

1. **Zero-Downtime Live Schema Upgrades (`app/db.py`):**
   Production NS8 upgrades retain the existing `swm-db-data` PostgreSQL volume. On every startup, `connect_with_backoff()` runs `Base.metadata.create_all` followed immediately by `ensure_schema_upgrades(sync_conn)`, which inspects live table columns and executes idempotent `ALTER TABLE ... ADD COLUMN` statements on both PostgreSQL and SQLite whenever new columns are introduced.
2. **NS8 `Ldapproxy` Host-Loopback Integration (`imageroot/` + `app/auth/ldap_provider.py`):**
   Inside NS8's rootless Podman pod (`--network=slirp4netns:allow_host_loopback=true`), host `127.0.0.1` `ldapproxy` endpoints discovered via `from agent.ldapproxy import Ldapproxy` are mapped to `10.0.2.2` (`LDAP_USE_SSL=false`). Both *Samba Active Directory* (`ad`) and *OpenLDAP* (`rfc2307`, including anonymous subtree DN lookup when `bind_dn` is empty) are supported.
3. **Configuration Environment Variables (`app/config.py`):**
   - `DATABASE_URL` — `postgresql+asyncpg://...` in NS8 pod (`sqlite+aiosqlite:///./data/swm.db` in local dev)
   - `DATA_DIR` — Persistent storage for `keys/` (`Ed25519` + `jwt.secret`) and uploaded `packages/` (`/data` in container)
   - `STATIC_AGENTS_DIR` — Directory holding bundled `windows/`, `linux-amd64/`, and `macos-universal/` client binaries (`/app/agents` in container)
   - `WEB_DIST_DIR` — Compiled Vue 3 Admin Console SPA (`/app/web/dist` in container)
   - `PUBLIC_BASE_URL` — Externally reachable Traefik HTTPS URL (e.g. `https://swm.example.com`)
   - `ADMIN_USERNAME` / `ADMIN_PASSWORD` — Local fallback administrator credentials
   - `LDAP_HOST`, `LDAP_PORT`, `LDAP_USE_SSL`, `LDAP_BASE_DN`, `LDAP_BIND_DN`, `LDAP_BIND_PASSWORD` — NS8 user-domain settings
