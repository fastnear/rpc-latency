use crate::*;
use lazy_static::lazy_static;
use prometheus::{
    exponential_buckets, register_histogram_vec, register_int_counter, register_int_counter_vec,
    Encoder, HistogramVec, IntCounter, IntCounterVec,
};

lazy_static! {
    static ref METRICS_COUNTER: IntCounter =
        register_int_counter!("metrics_requests", "Number of times metrics were requested")
            .unwrap();
    pub static ref RPC_LOOP_COUNTER: IntCounter =
        register_int_counter!("rpc_loop_count", "Number of times the RPC loop has run").unwrap();
    pub static ref RPC_REQUEST_COUNTER: IntCounterVec = register_int_counter_vec!(
        "rpc_request_count",
        "Number of RPC requests",
        &["url", "name"]
    )
    .unwrap();
    pub static ref RPC_SUCCESS_COUNTER: IntCounterVec = register_int_counter_vec!(
        "rpc_success_count",
        "Number of successful RPC requests",
        &["url", "name"]
    )
    .unwrap();
    pub static ref RPC_ERROR_JSON_COUNTER: IntCounterVec = register_int_counter_vec!(
        "rpc_error_json_count",
        "Number of JSON errors in RPC requests",
        &["url", "name"]
    )
    .unwrap();
    pub static ref RPC_ERROR_TIMEOUT_COUNTER: IntCounterVec = register_int_counter_vec!(
        "rpc_error_timeout_count",
        "Number of timeouts in RPC requests",
        &["url", "name"]
    )
    .unwrap();
    pub static ref RPC_ERROR_REQWEST_COUNTER: IntCounterVec = register_int_counter_vec!(
        "rpc_error_reqwest_count",
        "Number of Reqwest errors in RPC requests",
        &["url", "name"]
    )
    .unwrap();
    pub static ref RPC_SUCCESS_LATENCY_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "rpc_success_latency",
        "Latency of RPC requests",
        &["url", "name"],
        exponential_buckets(0.001, 2.0, 16).unwrap()
    )
    .unwrap();
    pub static ref RPC_ERROR_LATENCY_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "rpc_error_latency",
        "Latency of RPC errors",
        &["url", "name"],
        exponential_buckets(0.001, 2.0, 16).unwrap()
    )
    .unwrap();
}

#[get("/metrics")]
pub async fn get_metrics() -> Result<impl Responder, actix_web::Error> {
    METRICS_COUNTER.inc();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];
    let encoder = prometheus::TextEncoder::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    Ok(HttpResponse::Ok()
        .content_type(encoder.format_type())
        .body(buffer))
}
