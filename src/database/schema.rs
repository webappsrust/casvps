pub const SCHEMA_SYSTEM_CONFIG: &str = r#"
CREATE TABLE IF NOT EXISTS system_config (
    key TEXT PRIMARY KEY,
    value JSON NOT NULL,
    category TEXT,
    node_specific BOOLEAN DEFAULT FALSE,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_by TEXT
)"#;

pub const SCHEMA_USERS: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    user_id TEXT PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    realm TEXT DEFAULT 'local',
    email TEXT,
    role TEXT DEFAULT 'user',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_login TIMESTAMP,
    enabled BOOLEAN DEFAULT TRUE
)"#;

pub const SCHEMA_VMS: &str = r#"
CREATE TABLE IF NOT EXISTS vms (
    vm_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    user_id TEXT,
    config JSON,
    state TEXT DEFAULT 'stopped',
    node_id TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)"#;

pub const SCHEMA_CONTAINERS: &str = r#"
CREATE TABLE IF NOT EXISTS containers (
    container_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    user_id TEXT,
    image TEXT,
    config JSON,
    state TEXT DEFAULT 'stopped',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)"#;

pub const SCHEMA_API_TOKENS: &str = r#"
CREATE TABLE IF NOT EXISTS api_tokens (
    token_hash TEXT PRIMARY KEY,
    token_prefix TEXT,
    name TEXT NOT NULL,
    user_id TEXT NOT NULL,
    scopes JSON,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_used TIMESTAMP,
    expires_at TIMESTAMP,
    active BOOLEAN DEFAULT TRUE
)"#;

pub const SCHEMA_BOOT_ORDER: &str = r#"
CREATE TABLE IF NOT EXISTS boot_order (
    instance_id TEXT PRIMARY KEY,
    instance_type TEXT,
    priority INTEGER DEFAULT 999,
    delay_seconds INTEGER DEFAULT 10,
    autostart BOOLEAN DEFAULT FALSE
)"#;

pub const SCHEMA_ISO_LIBRARY: &str = r#"
CREATE TABLE IF NOT EXISTS iso_library (
    id TEXT PRIMARY KEY,
    distro_name TEXT NOT NULL,
    major_version TEXT NOT NULL,
    minor_version TEXT,
    architecture TEXT,
    filename TEXT NOT NULL,
    source_url TEXT,
    local_path TEXT,
    auto_update BOOLEAN DEFAULT TRUE,
    UNIQUE(distro_name, major_version, architecture)
)"#;

pub const SCHEMA_SNAPSHOTS: &str = r#"
CREATE TABLE IF NOT EXISTS snapshots (
    snapshot_id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    name TEXT NOT NULL,
    include_memory BOOLEAN DEFAULT FALSE,
    size_bytes INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)"#;

pub const SCHEMA_BACKUP_JOBS: &str = r#"
CREATE TABLE IF NOT EXISTS backup_jobs (
    job_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    schedule TEXT,
    source_type TEXT,
    source_id TEXT,
    destination TEXT,
    retention_policy TEXT,
    compression TEXT DEFAULT 'zstd',
    deduplication BOOLEAN DEFAULT TRUE,
    encryption_key TEXT,
    enabled BOOLEAN DEFAULT TRUE
)"#;

pub const SCHEMA_CLUSTER_NODES: &str = r#"
CREATE TABLE IF NOT EXISTS cluster_nodes (
    node_id TEXT PRIMARY KEY,
    node_name TEXT NOT NULL,
    address TEXT NOT NULL,
    role TEXT,
    status TEXT,
    joined_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_heartbeat TIMESTAMP
)"#;

pub const SCHEMA_USER_NETWORKS: &str = r#"
CREATE TABLE IF NOT EXISTS user_networks (
    network_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    subnet TEXT NOT NULL,
    vlan_id INTEGER UNIQUE,
    domain TEXT,
    gateway TEXT,
    dns_servers TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)"#;

pub const SCHEMA_IP_ALLOCATIONS: &str = r#"
CREATE TABLE IF NOT EXISTS ip_allocations (
    ip_address TEXT PRIMARY KEY,
    subnet_id TEXT,
    allocation_type TEXT,
    resource_type TEXT,
    resource_id TEXT,
    hostname TEXT,
    mac_address TEXT,
    allocated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)"#;

pub const SCHEMA_CERTIFICATES: &str = r#"
CREATE TABLE IF NOT EXISTS certificates (
    id TEXT PRIMARY KEY,
    domain TEXT NOT NULL,
    type TEXT,
    cert_path TEXT,
    key_path TEXT,
    expires_at TIMESTAMP,
    auto_renew BOOLEAN DEFAULT TRUE
)"#;

pub const SCHEMA_RECOVERY_RULES: &str = r#"
CREATE TABLE IF NOT EXISTS recovery_rules (
    rule_id TEXT PRIMARY KEY,
    condition TEXT,
    action TEXT,
    priority INTEGER,
    cooldown_seconds INTEGER DEFAULT 300,
    max_triggers_per_hour INTEGER DEFAULT 10,
    enabled BOOLEAN DEFAULT TRUE
)"#;

pub const SCHEMA_SCHEDULED_TASKS: &str = r#"
CREATE TABLE IF NOT EXISTS scheduled_tasks (
    task_id TEXT PRIMARY KEY,
    name TEXT,
    schedule TEXT,
    command TEXT,
    enabled BOOLEAN DEFAULT TRUE,
    last_run TIMESTAMP,
    next_run TIMESTAMP
)"#;

pub const SCHEMA_COMPLIANCE_CONFIG: &str = r#"
CREATE TABLE IF NOT EXISTS compliance_config (
    compliance_type TEXT PRIMARY KEY,
    enabled BOOLEAN DEFAULT FALSE,
    config JSON
)"#;

pub const SCHEMA_AUDIT_LOG: &str = r#"
CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    user_id TEXT,
    action TEXT,
    resource_type TEXT,
    resource_id TEXT,
    details JSON,
    ip_address TEXT
)"#;

pub const SCHEMA_USERNAME_BLACKLIST: &str = r#"
CREATE TABLE IF NOT EXISTS username_blacklist (
    username TEXT PRIMARY KEY
)"#;

pub const SCHEMA_AUTH_REALMS: &str = r#"
CREATE TABLE IF NOT EXISTS auth_realms (
    realm_id TEXT PRIMARY KEY,
    realm_type TEXT,
    priority INTEGER,
    config JSON,
    enabled BOOLEAN DEFAULT TRUE
)"#;

pub const SCHEMA_NOTIFICATION_CHANNELS: &str = r#"
CREATE TABLE IF NOT EXISTS notification_channels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    type TEXT,
    enabled BOOLEAN DEFAULT TRUE,
    config JSON
)"#;

pub const SCHEMA_NOTIFICATION_RULES: &str = r#"
CREATE TABLE IF NOT EXISTS notification_rules (
    id TEXT PRIMARY KEY,
    name TEXT,
    event_type TEXT,
    severity TEXT,
    channel_id TEXT,
    cooldown_minutes INTEGER DEFAULT 60
)"#;

pub const SCHEMA_REPORT_TEMPLATES: &str = r#"
CREATE TABLE IF NOT EXISTS report_templates (
    id TEXT PRIMARY KEY,
    name TEXT,
    category TEXT,
    format TEXT,
    schedule TEXT,
    recipients TEXT,
    template_data TEXT
)"#;

pub const SCHEMA_IP_NETWORKS: &str = r#"
CREATE TABLE IF NOT EXISTS ip_networks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    network_cidr TEXT NOT NULL,
    type TEXT DEFAULT 'user',
    user_id TEXT,
    vlan_id INTEGER,
    ipv6_network TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)"#;

pub const SCHEMA_MONITORED_SERVICES: &str = r#"
CREATE TABLE IF NOT EXISTS monitored_services (
    id TEXT PRIMARY KEY,
    resource_id TEXT,
    service_name TEXT,
    check_type TEXT,
    check_config JSON,
    check_interval INTEGER DEFAULT 60,
    timeout INTEGER DEFAULT 10,
    retry_count INTEGER DEFAULT 3,
    enabled BOOLEAN DEFAULT TRUE,
    auto_restart BOOLEAN DEFAULT FALSE,
    last_status TEXT
)"#;

pub const SCHEMA_SYSCTL_CONFIG: &str = r#"
CREATE TABLE IF NOT EXISTS sysctl_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    category TEXT,
    applied BOOLEAN DEFAULT FALSE
)"#;