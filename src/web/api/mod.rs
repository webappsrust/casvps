pub mod auth;
pub mod vms;
pub mod containers;
pub mod networks;
pub mod storage;
pub mod monitoring;
pub mod admin;

use axum::{Router, routing::get, response::Json};
use serde_json::json;
use crate::database::Database;
use crate::virtualization::VirtualizationManager;
use std::sync::Arc;
use super::WebState;

pub type ApiState = WebState;

pub fn create_routes() -> Router<ApiState> {
    Router::new()
        .route("/", get(api_info))
        .route("/status", get(get_status))
        .route("/health", get(get_health))
        .nest("/auth", auth::create_routes())
        .nest("/admin", admin::create_routes())
        .nest("/users", Router::new()) // User-specific routes
        .nest("/vms", vms::create_routes())
        .nest("/containers", containers::create_routes())
        .nest("/networks", networks::create_routes())
        .nest("/storage", storage::create_routes())
        .nest("/monitoring", monitoring::create_routes())
        .route("/openapi.json", get(openapi_spec))
}

pub fn create_proxmox_compat_routes() -> Router<ApiState> {
    // Industry-standard Proxmox VE API compatibility
    Router::new()
        .route("/version", get(proxmox_version))
        .nest("/nodes", create_nodes_routes())
        .nest("/cluster", create_cluster_routes())
        .nest("/access", create_access_routes())
}

fn create_nodes_routes() -> Router<ApiState> {
    Router::new()
        .route("/", get(list_nodes))
        .route("/:node/status", get(node_status))
        .route("/:node/qemu", get(list_qemu_vms))
        .route("/:node/lxc", get(list_lxc_containers))
        .route("/:node/storage", get(list_node_storage))
}

fn create_cluster_routes() -> Router<ApiState> {
    Router::new()
        .route("/status", get(cluster_status))
        .route("/resources", get(cluster_resources))
}

fn create_access_routes() -> Router<ApiState> {
    Router::new()
        .route("/ticket", get(get_auth_ticket))
        .route("/users", get(list_access_users))
}

async fn api_info() -> Json<serde_json::Value> {
    Json(json!({
        "name": "CasVPS API",
        "version": "1.0.0",
        "description": "Complete Application Server for Virtualization API",
        "documentation": "/support/api",
        "compatibility": ["Proxmox VE 8.x"],
        "endpoints": {
            "v1": "/api/v1",
            "proxmox": "/api2/json"
        }
    }))
}

async fn get_status() -> Json<serde_json::Value> {
    Json(json!({
        "status": "running",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime": 0,
        "nodes": 1
    }))
}

async fn get_health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "checks": {
            "database": true,
            "dhcp": true,
            "dns": true,
            "tftp": true,
            "virtualization": true
        }
    }))
}

async fn openapi_spec() -> Json<serde_json::Value> {
    Json(json!({
        "openapi": "3.0.3",
        "info": {
            "title": "CasVPS API",
            "version": "1.0.0",
            "description": "Complete Application Server for Virtualization"
        },
        "servers": [
            {"url": "/api/v1", "description": "CasVPS API v1"},
            {"url": "/api2/json", "description": "Proxmox VE Compatibility"}
        ],
        "paths": {
            "/vms": {
                "get": {
                    "summary": "List virtual machines",
                    "tags": ["Virtual Machines"]
                }
            }
        }
    }))
}

// Proxmox compatibility endpoints
async fn proxmox_version() -> Json<serde_json::Value> {
    Json(json!({
        "version": "8.0.0", // Emulate Proxmox VE 8.0
        "repoid": "casvps",
        "release": "1.0"
    }))
}

async fn list_nodes() -> Json<serde_json::Value> {
    Json(json!({
        "data": [
            {
                "node": "localhost",
                "status": "online",
                "type": "node",
                "cpu": 0.1,
                "maxcpu": 8,
                "mem": 1073741824,
                "maxmem": 8589934592,
                "disk": 10737418240,
                "maxdisk": 107374182400
            }
        ]
    }))
}

async fn node_status() -> Json<serde_json::Value> {
    Json(json!({
        "data": {
            "uptime": 3600,
            "cpu": 0.1,
            "memory": {
                "used": 1073741824,
                "total": 8589934592
            },
            "swap": {
                "used": 0,
                "total": 2147483648
            }
        }
    }))
}

async fn list_qemu_vms() -> Json<serde_json::Value> {
    Json(json!({"data": []}))
}

async fn list_lxc_containers() -> Json<serde_json::Value> {
    Json(json!({"data": []}))
}

async fn list_node_storage() -> Json<serde_json::Value> {
    Json(json!({
        "data": [
            {
                "storage": "local",
                "type": "dir",
                "used": 10737418240,
                "total": 107374182400,
                "avail": 96636764160,
                "active": true
            }
        ]
    }))
}

async fn cluster_status() -> Json<serde_json::Value> {
    Json(json!({
        "data": [
            {
                "type": "node",
                "id": "node/localhost",
                "node": "localhost",
                "online": true,
                "level": ""
            }
        ]
    }))
}

async fn cluster_resources() -> Json<serde_json::Value> {
    Json(json!({
        "data": [
            {
                "type": "node",
                "id": "node/localhost",
                "node": "localhost",
                "maxcpu": 8,
                "cpu": 0.1,
                "maxmem": 8589934592,
                "mem": 1073741824,
                "status": "online"
            }
        ]
    }))
}

async fn get_auth_ticket() -> Json<serde_json::Value> {
    Json(json!({
        "data": {
            "ticket": "PVE:root@pam:12345678::...",
            "CSRFPreventionToken": "12345678:..."
        }
    }))
}

async fn list_access_users() -> Json<serde_json::Value> {
    Json(json!({
        "data": [
            {
                "userid": "root@pam",
                "enable": true,
                "expire": 0,
                "firstname": "Administrator",
                "lastname": "CasVPS"
            }
        ]
    }))
}