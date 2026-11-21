use crate::config::normalize_server_url;
use anyhow::{bail, Context, Result};
use reqwest::{Client, StatusCode};
use std::time::Duration;
use swm_common::{
    AgentStatusResponse, EnrollRequest, EnrollResponse, InventoryAckResponse, InventoryReport,
    JobResult, PendingJobsResponse, SelfServiceTriggerRequest, SignedJob,
};

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceAuthStatus {
    Approved(AgentStatusResponse),
    Pending(AgentStatusResponse),
    Revoked,
}

#[derive(Clone)]
pub struct AgentApiClient {
    http: Client,
    server_url: String,
    device_token: Option<String>,
}

impl AgentApiClient {
    pub fn new(server_url: &str, device_token: Option<String>) -> Result<Self> {
        let accept_invalid_certs = std::env::var("SWM_INSECURE_TLS")
            .ok()
            .map(|v| matches!(v.trim(), "1" | "true" | "TRUE"))
            .unwrap_or(false);

        let http = Client::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(15))
            .danger_accept_invalid_certs(accept_invalid_certs)
            .user_agent(format!("NS8-SWM-Agent/{}", swm_common::AGENT_VERSION))
            .build()
            .context("Failed building HTTPS reqwest client")?;

        Ok(Self {
            http,
            server_url: normalize_server_url(server_url),
            device_token,
        })
    }

    pub fn http_client(&self) -> &Client {
        &self.http
    }

    pub fn server_url(&self) -> &str {
        &self.server_url
    }

    pub fn device_token(&self) -> Option<&str> {
        self.device_token.as_deref()
    }

    fn bearer_token(&self) -> Result<&str> {
        self.device_token
            .as_deref()
            .filter(|t| !t.is_empty())
            .context("Missing device_token for authenticated agent endpoint")
    }

    pub fn resolve_url(&self, path_or_url: &str) -> String {
        let trimmed = path_or_url.trim();
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            trimmed.to_string()
        } else if trimmed.starts_with('/') {
            format!("{}{}", self.server_url, trimmed)
        } else {
            format!("{}/{}", self.server_url, trimmed)
        }
    }

    pub async fn enroll(&self, req: &EnrollRequest) -> Result<EnrollResponse> {
        let url = format!("{}/api/v1/agent/enroll", self.server_url);
        let resp = self
            .http
            .post(&url)
            .json(req)
            .send()
            .await
            .with_context(|| format!("Failed POST {url}"))?;

        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Enrollment rejected by server ({code}): {body}");
        }

        resp.json::<EnrollResponse>()
            .await
            .context("Invalid EnrollResponse JSON from server")
    }

    pub async fn get_status(&self) -> Result<DeviceAuthStatus> {
        let token = self.bearer_token()?;
        let url = format!("{}/api/v1/agent/status", self.server_url);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .with_context(|| format!("Failed GET {url}"))?;

        if resp.status() == StatusCode::FORBIDDEN || resp.status() == StatusCode::UNAUTHORIZED {
            return Ok(DeviceAuthStatus::Revoked);
        }
        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("GET /api/v1/agent/status failed ({code}): {body}");
        }

        let parsed: AgentStatusResponse = resp
            .json()
            .await
            .context("Invalid AgentStatusResponse JSON")?;

        match parsed.status.trim().to_ascii_lowercase().as_str() {
            "revoked" => Ok(DeviceAuthStatus::Revoked),
            "pending" => Ok(DeviceAuthStatus::Pending(parsed)),
            _ => Ok(DeviceAuthStatus::Approved(parsed)),
        }
    }

    pub async fn post_inventory(&self, report: &InventoryReport) -> Result<InventoryAckResponse> {
        let token = self.bearer_token()?;
        let url = format!("{}/api/v1/agent/inventory", self.server_url);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(token)
            .json(report)
            .send()
            .await
            .with_context(|| format!("Failed POST {url}"))?;

        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("POST /api/v1/agent/inventory failed ({code}): {body}");
        }

        resp.json::<InventoryAckResponse>()
            .await
            .context("Invalid InventoryAckResponse JSON")
    }

    pub async fn get_pending_jobs(&self) -> Result<PendingJobsResponse> {
        let token = self.bearer_token()?;
        let url = format!("{}/api/v1/agent/jobs/pending", self.server_url);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .with_context(|| format!("Failed GET {url}"))?;

        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("GET /api/v1/agent/jobs/pending failed ({code}): {body}");
        }

        resp.json::<PendingJobsResponse>()
            .await
            .context("Invalid PendingJobsResponse JSON")
    }

    pub async fn post_job_result(&self, job_id: &str, result: &JobResult) -> Result<()> {
        let token = self.bearer_token()?;
        let url = format!("{}/api/v1/agent/jobs/{job_id}/result", self.server_url);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(token)
            .json(result)
            .send()
            .await
            .with_context(|| format!("Failed POST {url}"))?;

        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("POST /api/v1/agent/jobs/{job_id}/result failed ({code}): {body}");
        }
        Ok(())
    }

    pub async fn trigger_self_service(
        &self,
        package_id: &str,
        action: &str,
    ) -> Result<SignedJob> {
        let token = self.bearer_token()?;
        let url = format!("{}/api/v1/agent/self-service/trigger", self.server_url);
        let payload = SelfServiceTriggerRequest {
            package_id: package_id.to_string(),
            action: action.to_string(),
        };
        let resp = self
            .http
            .post(&url)
            .bearer_auth(token)
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("Failed POST {url}"))?;

        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Self-service trigger rejected ({code}): {body}");
        }

        resp.json::<SignedJob>()
            .await
            .context("Invalid SignedJob JSON returned by self-service trigger")
    }
}
