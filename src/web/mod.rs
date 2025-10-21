pub mod api;
pub mod ui;

use anyhow::Result;
use axum::{Router, routing::get};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;
use crate::database::Database;
use crate::virtualization::VirtualizationManager;

#[derive(Clone)]
pub struct WebState {
    pub database: Arc<Database>,
    pub virtualization: Arc<VirtualizationManager>,
}

pub struct WebServer {
    state: WebState,
    port: u16,
}

impl WebServer {
    pub async fn new(
        database: Arc<Database>,
        virtualization: Arc<VirtualizationManager>,
    ) -> Result<Self> {
        let state = WebState {
            database,
            virtualization,
        };

        Ok(Self {
            state,
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
        info!("Access CasVPS at: {}", self.get_best_url());

        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await?;

        Ok(())
    }

    fn create_router(&self) -> Router {
        Router::new()
            // UI routes
            .merge(ui::create_routes())
            // API v1 routes
            .nest("/api/v1", api::create_routes())
            // Proxmox compatibility
            .nest("/api2/json", api::create_proxmox_compat_routes())
            // Static health check
            .route("/health", get(|| async { "OK" }))
            // Add state and CORS
            .with_state(self.state.clone())
            .layer(CorsLayer::permissive())
    }

    fn detect_fqdn_reverse_proxy(&self) -> Option<String> {
        // Check if behind reverse proxy with FQDN
        // Look for reverse proxy headers, DNS resolution, etc.
        None
    }

    fn detect_wan_ip(&self) -> Option<String> {
        // Try to detect WAN IP via external services
        None
    }

    fn detect_lan_ip(&self) -> Option<String> {
        // Get LAN IP
        let interfaces = pnet::datalink::interfaces();
        for interface in interfaces {
            if !interface.is_loopback() && interface.is_up() {
                for ip in &interface.ips {
                    if let pnet::ipnetwork::IpNetwork::V4(ipv4) = ip {
                        let ip_addr = ipv4.ip();
                        if !ip_addr.is_loopback() &&
                           !ip_addr.is_multicast() &&
                           !ip_addr.is_broadcast() {
                            return Some(ip_addr.to_string());
                        }
                    }
                }
            }
        }
        None
    }
}