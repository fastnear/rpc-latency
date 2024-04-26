use crate::metrics::*;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::env;
use std::sync::Arc;
use std::time::Duration;

const TARGET_RPC: &str = "rpc";

#[derive(Debug, Clone, Deserialize)]
pub struct Endpoint {
    pub url: String,
    pub bearer_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Payload {
    pub name: String,
    /// None means get status call
    pub data: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct RpcServiceConfig {
    pub timeout_ms: u64,
    pub period_ms: u64,
    pub concurrency: usize,
    pub endpoints: Vec<Endpoint>,
    pub payloads: Vec<Payload>,
}

impl RpcServiceConfig {
    pub fn from_env() -> Self {
        let config_path = env::var("RPC_CONFIG_PATH").expect("RPC_CONFIG_PATH is not set");
        let config = std::fs::read_to_string(config_path).expect("Failed to read config file");
        serde_json::from_str(&config).expect("Failed to parse config")
    }
}

#[derive(Debug)]
pub enum RpcError {
    ReqwestError(reqwest::Error),
}

impl From<reqwest::Error> for RpcError {
    fn from(error: reqwest::Error) -> Self {
        RpcError::ReqwestError(error)
    }
}

async fn rpc_json_request(
    data: Value,
    client: &Client,
    url: &String,
    bearer_token: &Option<String>,
    timeout: Duration,
) -> Result<Value, RpcError> {
    let mut response = client.post(url);
    if let Some(bearer) = bearer_token {
        response = response.bearer_auth(bearer);
    }
    let response = response.json(&data).timeout(timeout).send().await?;
    let response = response.json::<Value>().await?;

    Ok(response)
}

pub async fn request_rpc_status(
    client: &Client,
    url: &String,
    bearer_token: &Option<String>,
    timeout: Duration,
) -> Result<Value, RpcError> {
    let mut response = client.get(format!("{}/status", url));
    if let Some(bearer) = bearer_token {
        response = response.bearer_auth(bearer);
    }
    let response = response.timeout(timeout).send().await?;
    let response = response.json::<Value>().await?;

    Ok(response)
}

pub async fn start_service(config: RpcServiceConfig) {
    let client = Client::new();
    // start interval loop with tokio
    let timeout = Duration::from_millis(config.timeout_ms);
    let mut interval = tokio::time::interval(Duration::from_millis(config.period_ms));
    loop {
        RPC_LOOP_COUNTER.inc();
        tracing::info!(target: TARGET_RPC, "Starting RPC loop");
        let loop_start = std::time::Instant::now();
        let mut tasks = vec![];
        let permits = Arc::new(tokio::sync::Semaphore::new(config.concurrency));
        for endpoint in &config.endpoints {
            let mut endpoint_tasks = vec![];
            for payload in config.payloads.clone() {
                let url = endpoint.url.clone();
                let bearer_token = endpoint.bearer_token.clone();
                let client = client.clone();
                let permits = permits.clone();
                endpoint_tasks.push(async move {
                    let _permit = permits.acquire().await.unwrap();
                    let name = &payload.name;
                    tracing::debug!(target: TARGET_RPC, "Requesting {} from {}", name, url);
                    RPC_REQUEST_COUNTER.with_label_values(&[&url, &name]).inc();
                    let start = std::time::Instant::now();
                    let response = if let Some(data) = payload.data {
                        rpc_json_request(data, &client, &url, &bearer_token, timeout).await
                    } else {
                        request_rpc_status(&client, &url, &bearer_token, timeout).await
                    };
                    let elapsed = start.elapsed().as_nanos() as f64 * 1e-9;
                    let is_ok = match response {
                        Ok(response) => {
                            if serde_json::from_value::<Value>(response).is_ok() {
                                RPC_SUCCESS_COUNTER.with_label_values(&[&url, &name]).inc();
                                true
                            } else {
                                RPC_ERROR_JSON_COUNTER
                                    .with_label_values(&[&url, &name])
                                    .inc();
                                false
                            }
                        }
                        Err(RpcError::ReqwestError(err)) => {
                            if err.is_timeout() {
                                RPC_ERROR_TIMEOUT_COUNTER
                                    .with_label_values(&[&url, &name])
                                    .inc();
                            } else {
                                RPC_ERROR_REQWEST_COUNTER
                                    .with_label_values(&[&url, &name])
                                    .inc();
                            }
                            false
                        }
                    };
                    if is_ok {
                        RPC_SUCCESS_LATENCY_HISTOGRAM
                            .with_label_values(&[&url, &name])
                            .observe(elapsed);
                    } else {
                        RPC_ERROR_LATENCY_HISTOGRAM
                            .with_label_values(&[&url, &name])
                            .observe(elapsed);
                    }
                });
            }
            tasks.push(endpoint_tasks);
        }
        // Reordering tasks to avoid hitting the same endpoint multiple times in a row
        let mut ordered_tasks = vec![];
        let mut iterators: Vec<_> = tasks.into_iter().map(|tasks| tasks.into_iter()).collect();
        for _ in &config.payloads {
            for iter in iterators.iter_mut() {
                ordered_tasks.push(iter.next().unwrap());
            }
        }
        // Spawning tasks with concurrency
        let tasks = ordered_tasks.into_iter().map(|task| tokio::spawn(task));
        futures::future::join_all(tasks).await;
        let loop_elapsed = loop_start.elapsed().as_millis();
        tracing::info!(target: TARGET_RPC, "Finished RPC loop {}ms", loop_elapsed);

        interval.tick().await;
    }
}
