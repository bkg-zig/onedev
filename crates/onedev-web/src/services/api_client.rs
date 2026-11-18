//! API Client for Web UI

use crate::WebConfig;
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// API client for communicating with OneDev API
#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
    token: Option<String>,
}

impl ApiClient {
    pub fn new(config: &WebConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.api.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: config.api.base_url.clone(),
            token: None,
        }
    }

    pub fn with_token(mut self, token: String) -> Self {
        self.token = Some(token);
        self
    }

    /// Make a GET request
    pub async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = format!("{}/api{}", self.base_url, path);
        let mut request = self.client.get(&url);

        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }

        let response = request.send().await?;
        response.error_for_status_ref()?;
        Ok(response.json().await?)
    }

    /// Make a POST request
    pub async fn post<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}/api{}", self.base_url, path);
        let mut request = self.client.post(&url).json(body);

        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }

        let response = request.send().await?;
        response.error_for_status_ref()?;
        Ok(response.json().await?)
    }

    /// Make a PATCH request
    pub async fn patch<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}/api{}", self.base_url, path);
        let mut request = self.client.patch(&url).json(body);

        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }

        let response = request.send().await?;
        response.error_for_status_ref()?;
        Ok(response.json().await?)
    }

    /// Make a DELETE request
    pub async fn delete(&self, path: &str) -> Result<()> {
        let url = format!("{}/api{}", self.base_url, path);
        let mut request = self.client.delete(&url);

        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }

        let response = request.send().await?;
        response.error_for_status_ref()?;
        Ok(())
    }
}

/// API response wrapper
#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
    pub meta: Option<ApiMeta>,
}

#[derive(Debug, Deserialize)]
pub struct ApiMeta {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub total: Option<u64>,
    pub total_pages: Option<u32>,
}

/// Paginated list response
#[derive(Debug, Deserialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
    pub total_pages: u32,
}
