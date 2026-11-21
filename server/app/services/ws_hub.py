from __future__ import annotations

import logging
from collections import defaultdict
from typing import Any

from fastapi import WebSocket

logger = logging.getLogger(__name__)


class WebSocketHub:
    """Tracks live agent WebSocket connections and pushes server-to-agent events."""

    def __init__(self) -> None:
        self._connections: dict[str, set[WebSocket]] = defaultdict(set)

    def register(self, device_id: str, websocket: WebSocket) -> None:
        self._connections[device_id].add(websocket)

    def unregister(self, device_id: str, websocket: WebSocket) -> bool:
        """Removes socket synchronously (cancellation-safe) and returns True if still online."""
        sockets = self._connections.get(device_id)
        if sockets is not None:
            sockets.discard(websocket)
            if not sockets:
                self._connections.pop(device_id, None)
                return False
            return True
        return False

    def is_online(self, device_id: str) -> bool:
        return bool(self._connections.get(device_id))

    def connected_device_ids(self) -> set[str]:
        return {
            device_id
            for device_id, sockets in self._connections.items()
            if sockets
        }

    async def send_to_device(self, device_id: str, message: dict[str, Any]) -> int:
        sockets = list(self._connections.get(device_id, ()))
        delivered = 0
        stale: list[WebSocket] = []
        for ws in sockets:
            try:
                await ws.send_json(message)
                delivered += 1
            except Exception:
                stale.append(ws)
        if stale:
            cur = self._connections.get(device_id)
            if cur is not None:
                for ws in stale:
                    cur.discard(ws)
                if not cur:
                    self._connections.pop(device_id, None)
        return delivered

    async def revoke_device(self, device_id: str) -> None:
        sockets = list(self._connections.pop(device_id, ()))
        for ws in sockets:
            try:
                await ws.send_json({"type": "revoked"})
                await ws.close(code=1008, reason="Device revoked")
            except Exception:
                pass
