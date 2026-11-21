from __future__ import annotations

import pytest
from fastapi.testclient import TestClient
from starlette.websockets import WebSocketDisconnect

from app.config import Settings
from app.main import create_app


def test_agent_websocket_online_offline_log_chunks_and_revoke(
    test_settings: Settings,
) -> None:
    app = create_app(test_settings)
    with TestClient(app) as tc:
        login = tc.post(
            "/api/v1/admin/auth/login",
            json={
                "username": test_settings.ADMIN_USERNAME,
                "password": test_settings.ADMIN_PASSWORD,
                "auth_source": "local",
            },
        )
        assert login.status_code == 200
        headers = {"Authorization": f"Bearer {login.json()['access_token']}"}

        # Create pending enrollment token
        tok = tc.post(
            "/api/v1/admin/enrollment-tokens",
            headers=headers,
            json={"name": "ws-test", "auto_approve": False},
        ).json()["token"]

        enr = tc.post(
            "/api/v1/agent/enroll",
            json={
                "enrollment_token": tok,
                "hostname": "ws-client-01",
                "os_family": "linux",
                "os_name": "Debian 12",
                "os_version": "12",
                "arch": "amd64",
                "agent_version": "1.0.0",
                "machine_uid": "uid-ws-01",
            },
        ).json()
        dev_id = enr["device_id"]
        dev_tok = enr["device_token"]

        # Unapproved device cannot open WebSocket
        with pytest.raises(WebSocketDisconnect):
            with tc.websocket_connect(f"/api/v1/agent/ws?token={dev_tok}"):
                pass

        # Pending device can poll /status (200 with status="pending"), cannot poll /jobs/pending (403)
        st_pending = tc.get(
            "/api/v1/agent/status", headers={"Authorization": f"Bearer {dev_tok}"}
        )
        assert st_pending.status_code == 200
        assert st_pending.json()["status"] == "pending"
        assert (
            tc.get(
                "/api/v1/agent/jobs/pending",
                headers={"Authorization": f"Bearer {dev_tok}"},
            ).status_code
            == 403
        )

        # Approve device
        app_resp = tc.post(f"/api/v1/admin/devices/{dev_id}/approve", headers=headers)
        assert app_resp.status_code == 200
        assert app_resp.json()["status"] == "approved"

        # Connect WebSocket -> device becomes online
        with tc.websocket_connect(f"/api/v1/agent/ws?token={dev_tok}") as ws:
            ws.send_json(
                {"type": "heartbeat", "agent_version": "1.0.1", "reboot_required": True}
            )
            ack = ws.receive_json()
            assert ack["type"] == "heartbeat_ack"

            dev_info = tc.get(f"/api/v1/admin/devices/{dev_id}", headers=headers).json()
            assert dev_info["is_online"] is True
            assert dev_info["agent_version"] == "1.0.1"
            assert dev_info["reboot_required"] is True

            # Trigger scan-updates job -> pushes job_available over WS
            scan_job = tc.post(
                f"/api/v1/admin/devices/{dev_id}/scan-updates", headers=headers
            ).json()
            pushed = ws.receive_json()
            assert pushed == {"type": "job_available", "job_id": scan_job["id"]}

            # Stream live log chunks over WS
            ws.send_json(
                {
                    "type": "log_chunk",
                    "job_id": scan_job["id"],
                    "stream": "stdout",
                    "chunk": "Scanning APT repositories...\n",
                }
            )
            ws.send_json({"type": "heartbeat"})
            assert ws.receive_json()["type"] == "heartbeat_ack"

            job_detail = tc.get(
                f"/api/v1/admin/jobs/{scan_job['id']}", headers=headers
            ).json()
            assert "Scanning APT repositories..." in job_detail["stdout"]
            assert job_detail["status"] == "running"

            # Finalize job via REST /result, then send a late WS log_chunk -> must NOT duplicate stdout!
            fin = tc.post(
                f"/api/v1/agent/jobs/{scan_job['id']}/result",
                headers={"Authorization": f"Bearer {dev_tok}"},
                json={
                    "status": "succeeded",
                    "exit_code": 0,
                    "reboot_required": False,
                    "stdout": "Final scan output.\n",
                    "stderr": "",
                },
            )
            assert fin.status_code == 200

            ws.send_json(
                {
                    "type": "log_chunk",
                    "job_id": scan_job["id"],
                    "stream": "stdout",
                    "chunk": "Final scan output.\n",
                }
            )
            ws.send_json({"type": "heartbeat"})
            assert ws.receive_json()["type"] == "heartbeat_ack"

            job_after_late_chunk = tc.get(
                f"/api/v1/admin/jobs/{scan_job['id']}", headers=headers
            ).json()
            assert job_after_late_chunk["status"] == "succeeded"
            assert job_after_late_chunk["stdout"] == "Final scan output.\n"

        # After closing WebSocket -> device becomes offline
        dev_after = tc.get(f"/api/v1/admin/devices/{dev_id}", headers=headers).json()
        assert dev_after["is_online"] is False

        # Reconnect and test admin revoke closing socket and returning 403 on REST
        with tc.websocket_connect(f"/api/v1/agent/ws?token={dev_tok}") as ws2:
            rev = tc.post(f"/api/v1/admin/devices/{dev_id}/revoke", headers=headers)
            assert rev.status_code == 200
            msg = ws2.receive_json()
            assert msg["type"] == "revoked"

        assert (
            tc.get(
                "/api/v1/agent/status", headers={"Authorization": f"Bearer {dev_tok}"}
            ).status_code
            == 403
        )
