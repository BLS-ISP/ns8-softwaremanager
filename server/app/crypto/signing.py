from __future__ import annotations

import base64
import hashlib
import os
import time
from pathlib import Path
from typing import Optional

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import (
    Ed25519PrivateKey,
    Ed25519PublicKey,
)


def compute_sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def compute_sha256_file(path: Path, chunk_size: int = 65536) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as f:
        while True:
            block = f.read(chunk_size)
            if not block:
                break
            digest.update(block)
    return digest.hexdigest()


class SigningService:
    """Manages the persistent 32-byte raw Ed25519 keypair and signs manifests."""

    def __init__(self, data_dir: Path) -> None:
        self.keys_dir = data_dir / "keys"
        self.private_key_path = self.keys_dir / "private.key"
        self.public_key_path = self.keys_dir / "public.key"
        self._private_key: Optional[Ed25519PrivateKey] = None
        self._public_key: Optional[Ed25519PublicKey] = None

    def ensure_keys(self) -> None:
        if self._private_key is not None and self._public_key is not None:
            return
        self.keys_dir.mkdir(parents=True, exist_ok=True)
        if self.private_key_path.exists() and self.public_key_path.exists():
            raw_priv = self.private_key_path.read_bytes()
            raw_pub = self.public_key_path.read_bytes()
            if len(raw_priv) != 32 or len(raw_pub) != 32:
                raise ValueError("Corrupted Ed25519 key files: expected 32 raw bytes each.")
            self._private_key = Ed25519PrivateKey.from_private_bytes(raw_priv)
            self._public_key = Ed25519PublicKey.from_public_bytes(raw_pub)
            return

        priv = Ed25519PrivateKey.generate()
        pub = priv.public_key()
        raw_priv = priv.private_bytes(
            encoding=serialization.Encoding.Raw,
            format=serialization.PrivateFormat.Raw,
            encryption_algorithm=serialization.NoEncryption(),
        )
        raw_pub = pub.public_bytes(
            encoding=serialization.Encoding.Raw,
            format=serialization.PublicFormat.Raw,
        )
        self.private_key_path.write_bytes(raw_priv)
        self.public_key_path.write_bytes(raw_pub)
        try:
            os.chmod(self.private_key_path, 0o600)
        except OSError:
            pass
        self._private_key = priv
        self._public_key = pub

    @property
    def private_key(self) -> Ed25519PrivateKey:
        self.ensure_keys()
        assert self._private_key is not None
        return self._private_key

    @property
    def public_key(self) -> Ed25519PublicKey:
        self.ensure_keys()
        assert self._public_key is not None
        return self._public_key

    def public_key_bytes(self) -> bytes:
        return self.public_key.public_bytes(
            encoding=serialization.Encoding.Raw,
            format=serialization.PublicFormat.Raw,
        )

    def public_key_b64(self) -> str:
        return base64.b64encode(self.public_key_bytes()).decode("ascii")

    @staticmethod
    def canonical_job_string(
        *,
        job_id: str,
        device_id: str,
        job_type: str,
        action: str,
        target_id: str,
        version: Optional[str],
        sha256: Optional[str],
        command: Optional[str],
        expires_at_unix: int,
    ) -> str:
        return (
            f"v1|job|{job_id}|{device_id}|{job_type}|{action}|{target_id}|"
            f"{version or ''}|{sha256 or ''}|{command or ''}|{expires_at_unix}"
        )

    @staticmethod
    def canonical_catalog_string(
        *,
        package_id: str,
        device_id: str,
        source_type: str,
        package_ref: str,
        version: Optional[str],
        sha256: Optional[str],
        install_cmd: Optional[str],
        uninstall_cmd: Optional[str],
        allow_uninstall: bool,
    ) -> str:
        allow_flag = "1" if allow_uninstall else "0"
        return (
            f"v1|catalog|{package_id}|{device_id}|{source_type}|{package_ref}|"
            f"{version or ''}|{sha256 or ''}|{install_cmd or ''}|{uninstall_cmd or ''}|{allow_flag}"
        )

    def sign_canonical(self, canonical_text: str) -> str:
        sig = self.private_key.sign(canonical_text.encode("utf-8"))
        return base64.b64encode(sig).decode("ascii")

    def sign_job(
        self,
        *,
        job_id: str,
        device_id: str,
        job_type: str,
        action: str,
        target_id: str,
        version: Optional[str],
        sha256: Optional[str],
        command: Optional[str],
        expires_at_unix: int,
    ) -> str:
        canonical = self.canonical_job_string(
            job_id=job_id,
            device_id=device_id,
            job_type=job_type,
            action=action,
            target_id=target_id,
            version=version,
            sha256=sha256,
            command=command,
            expires_at_unix=expires_at_unix,
        )
        return self.sign_canonical(canonical)

    def sign_catalog_item(
        self,
        *,
        package_id: str,
        device_id: str,
        source_type: str,
        package_ref: str,
        version: Optional[str],
        sha256: Optional[str],
        install_cmd: Optional[str],
        uninstall_cmd: Optional[str],
        allow_uninstall: bool,
    ) -> str:
        canonical = self.canonical_catalog_string(
            package_id=package_id,
            device_id=device_id,
            source_type=source_type,
            package_ref=package_ref,
            version=version,
            sha256=sha256,
            install_cmd=install_cmd,
            uninstall_cmd=uninstall_cmd,
            allow_uninstall=allow_uninstall,
        )
        return self.sign_canonical(canonical)

    @staticmethod
    def verify_canonical_with_b64_pubkey(
        public_key_b64: str,
        canonical_text: str,
        signature_b64: str,
    ) -> bool:
        try:
            raw_pub = base64.b64decode(public_key_b64, validate=True)
            raw_sig = base64.b64decode(signature_b64, validate=True)
            if len(raw_pub) != 32 or len(raw_sig) != 64:
                return False
            pub = Ed25519PublicKey.from_public_bytes(raw_pub)
            pub.verify(raw_sig, canonical_text.encode("utf-8"))
            return True
        except (ValueError, InvalidSignature):
            return False

    def verify_job(
        self,
        *,
        job_id: str,
        device_id: str,
        job_type: str,
        action: str,
        target_id: str,
        version: Optional[str],
        sha256: Optional[str],
        command: Optional[str],
        expires_at_unix: int,
        signature_b64: str,
        now_unix: Optional[int] = None,
    ) -> bool:
        now_ts = int(time.time()) if now_unix is None else now_unix
        if expires_at_unix < now_ts:
            return False
        canonical = self.canonical_job_string(
            job_id=job_id,
            device_id=device_id,
            job_type=job_type,
            action=action,
            target_id=target_id,
            version=version,
            sha256=sha256,
            command=command,
            expires_at_unix=expires_at_unix,
        )
        return self.verify_canonical_with_b64_pubkey(
            self.public_key_b64(),
            canonical,
            signature_b64,
        )

    def verify_catalog_item(
        self,
        *,
        package_id: str,
        device_id: str,
        source_type: str,
        package_ref: str,
        version: Optional[str],
        sha256: Optional[str],
        install_cmd: Optional[str],
        uninstall_cmd: Optional[str],
        allow_uninstall: bool,
        signature_b64: str,
    ) -> bool:
        canonical = self.canonical_catalog_string(
            package_id=package_id,
            device_id=device_id,
            source_type=source_type,
            package_ref=package_ref,
            version=version,
            sha256=sha256,
            install_cmd=install_cmd,
            uninstall_cmd=uninstall_cmd,
            allow_uninstall=allow_uninstall,
        )
        return self.verify_canonical_with_b64_pubkey(
            self.public_key_b64(),
            canonical,
            signature_b64,
        )
