use anyhow::Result;
use axum::{Router, routing::get};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;
use crate::database::Database;
use crate::virtualization::VirtualizationManager;

pub struct WebServer {
    database: Arc<Database>,
    virtualization: Arc<VirtualizationManager>,
    port: u16,
}

impl WebServer {
    pub async fn new(
        database: Arc<Database>,
        virtualization: Arc<VirtualizationManager>,
    ) -> Result<Self> {
        Ok(Self {
            database,
            virtualization,
            port: 8006,
        })
    }

    pub fn get_best_url(&self) -> String {
        // Priority order for URL display
        if let Some(fqdn) = self.detect_fqdn_reverse_proxy() {
            return format!("https://{}", fqdn);
        }

        if let Some(wan_ip) = self.detect_wan_ip() {
            return format!("https://{}:{}", wan_ip, self.port);
        }

        if let Some(lan_ip) = self.detect_lan_ip() {
            return format!("https://{}:{}", lan_ip, self.port);
        }

        format!("https://localhost:{}", self.port)
    }

    pub async fn run(&self) -> Result<()> {
        let app = self.create_router();

        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        info!("Web server listening on {}", addr);

        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await?;

        Ok(())
    }

    fn create_router(&self) -> Router {
        Router::new()
            .route("/", get(|| async { "CasVPS v1.0.0" }))
            .route("/health", get(|| async { "OK" }))
            .route("/api/v1/status", get(|| async { r#"{"status":"running"}"# }))
            .layer(CorsLayer::permissive())
    }

    fn detect_fqdn_reverse_proxy(&self) -> Option<String> {
        // Check if behind reverse proxy with FQDN
        None
    }

    fn detect_wan_ip(&self) -> Option<String> {
        // Try to detect WAN IP
        None
    }

    fn detect_lan_ip(&self) -> Option<String> {
        // Get LAN IP
        let interfaces = pnet::datalink::interfaces();
        for interface in interfaces {
            if !interface.is_loopback() && interface.is_up() {
                for ip in &interface.ips {
                    if let pnet::ipnetwork::IpNetwork::V4(ipv4) = ip {
                        if !ipv4.ip().is_loopback() {
                            return Some(ipv4.ip().to_string());
                        }
                    }
                }
            }
        }
        None
    }
}