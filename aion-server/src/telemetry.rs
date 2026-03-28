//! 遥测初始化：Prometheus 指标导出
//!
//! 安装 `metrics-exporter-prometheus` 作为全局 recorder，
//! 返回 `PrometheusHandle` 供 `/v1/metrics` 端点渲染。

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

/// 初始化 Prometheus 指标导出器，返回用于渲染的 handle
pub fn init_prometheus() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus metrics recorder")
}
