from __future__ import annotations

import asyncio
import logging
from typing import Optional

from ldap3 import Connection, NONE, SUBTREE, Server
from ldap3.utils.conv import escape_filter_chars

from app.config import Settings

logger = logging.getLogger(__name__)


class LdapConfigError(Exception):
    """Raised when LDAP authentication is requested without LDAP_HOST configured."""


def _sync_ldap_authenticate(
    username: str,
    password: str,
    settings: Settings,
) -> Optional[dict[str, str]]:
    if not settings.LDAP_HOST:
        raise LdapConfigError("LDAP authentication is not configured on this server.")
    if not username or not password:
        return None

    server = Server(
        settings.LDAP_HOST,
        port=settings.LDAP_PORT,
        use_ssl=settings.LDAP_USE_SSL,
        get_info=NONE,
        connect_timeout=5,
    )

    escaped_user = escape_filter_chars(username)
    search_filter = settings.LDAP_USER_FILTER.format(username=escaped_user)
    user_dn: Optional[str] = None
    canonical_username = username

    if settings.LDAP_BIND_DN and settings.LDAP_BASE_DN:
        # Step 1a: Service bind (e.g. Samba AD) to locate user DN safely
        with Connection(
            server,
            user=settings.LDAP_BIND_DN,
            password=settings.LDAP_BIND_PASSWORD,
            auto_bind=True,
            raise_exceptions=False,
        ) as svc_conn:
            if not svc_conn.bound:
                logger.warning("LDAP service bind failed for %s", settings.LDAP_BIND_DN)
                return None
            svc_conn.search(
                search_base=settings.LDAP_BASE_DN,
                search_filter=search_filter,
                search_scope=SUBTREE,
                attributes=["uid", "sAMAccountName", "cn"],
            )
            if not svc_conn.entries:
                return None
            entry = svc_conn.entries[0]
            user_dn = str(entry.entry_dn)
            for attr_name in ("uid", "sAMAccountName", "cn"):
                attr_obj = getattr(entry, attr_name, None)
                if attr_obj is not None and getattr(attr_obj, "value", None):
                    val = attr_obj.value
                    canonical_username = str(val[0] if isinstance(val, list) else val)
                    break
    elif settings.LDAP_BASE_DN:
        # Step 1b: Anonymous subtree search (e.g. NS8 OpenLDAP rfc2307 where bind_dn is empty)
        try:
            with Connection(
                server,
                auto_bind=True,
                raise_exceptions=False,
            ) as anon_conn:
                if anon_conn.bound:
                    anon_conn.search(
                        search_base=settings.LDAP_BASE_DN,
                        search_filter=search_filter,
                        search_scope=SUBTREE,
                        attributes=["uid", "sAMAccountName", "cn"],
                    )
                    if anon_conn.entries:
                        entry = anon_conn.entries[0]
                        user_dn = str(entry.entry_dn)
                        for attr_name in ("uid", "sAMAccountName", "cn"):
                            attr_obj = getattr(entry, attr_name, None)
                            if attr_obj is not None and getattr(attr_obj, "value", None):
                                val = attr_obj.value
                                canonical_username = str(val[0] if isinstance(val, list) else val)
                                break
        except Exception:
            user_dn = None

        if not user_dn:
            if "ou=people" in settings.LDAP_BASE_DN.lower():
                user_dn = f"uid={escaped_user},{settings.LDAP_BASE_DN}"
            else:
                user_dn = f"uid={escaped_user},ou=People,{settings.LDAP_BASE_DN}"
    else:
        user_dn = username

    if not user_dn:
        return None

    # Step 2: Bind with the end user's own DN and password
    try:
        with Connection(
            server,
            user=user_dn,
            password=password,
            auto_bind=True,
            raise_exceptions=False,
        ) as user_conn:
            if not user_conn.bound:
                return None
            return {
                "username": canonical_username,
                "dn": user_dn,
                "role": "admin",
            }
    except Exception as exc:
        logger.warning("LDAP user bind failed for %s: %s", username, exc)
        return None


async def authenticate_ldap(
    username: str,
    password: str,
    settings: Settings,
) -> Optional[dict[str, str]]:
    return await asyncio.to_thread(_sync_ldap_authenticate, username, password, settings)
