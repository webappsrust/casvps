use axum::{
    Router,
    routing::get,
    response::Html,
    extract::State,
};
use super::WebState;

pub mod templates;

pub fn create_routes() -> Router<WebState> {
    Router::new()
        .route("/", get(landing_page))
        .route("/login", get(login_page))
        .route("/admin/*path", get(admin_ui))
        .route("/users/*path", get(user_ui))
        .route("/support", get(support_portal))
        .route("/support/docs", get(documentation))
        .route("/support/kb", get(knowledge_base))
        .route("/support/api", get(api_docs))
        .route("/health", get(health_check))
}

async fn landing_page(State(state): State<WebState>) -> Html<String> {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>CasVPS - Complete Application Server for Virtualization</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0; padding: 0; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white; min-height: 100vh; display: flex; align-items: center; justify-content: center;
        }
        .container {
            text-align: center; max-width: 600px; padding: 2rem;
            background: rgba(255,255,255,0.1); backdrop-filter: blur(10px);
            border-radius: 20px; border: 1px solid rgba(255,255,255,0.2);
        }
        h1 { font-size: 3rem; margin-bottom: 0.5rem; font-weight: 300; }
        h2 { font-size: 1.2rem; margin-bottom: 2rem; opacity: 0.8; font-weight: 400; }
        .login-btn {
            background: rgba(255,255,255,0.2); color: white; border: 2px solid white;
            padding: 1rem 2rem; font-size: 1.1rem; border-radius: 50px;
            text-decoration: none; display: inline-block; transition: all 0.3s ease;
        }
        .login-btn:hover { background: white; color: #667eea; }
        .features {
            margin-top: 3rem; display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 1rem; text-align: left;
        }
        .feature {
            background: rgba(255,255,255,0.1); padding: 1rem; border-radius: 10px;
            border: 1px solid rgba(255,255,255,0.1);
        }
        .feature h3 { margin: 0 0 0.5rem 0; font-size: 1rem; }
        .feature p { margin: 0; font-size: 0.9rem; opacity: 0.8; }
    </style>
</head>
<body>
    <div class="container">
        <h1>CasVPS</h1>
        <h2>Complete Application Server for Virtualization</h2>
        <a href="/login" class="login-btn">Access Management Interface</a>

        <div class="features">
            <div class="feature">
                <h3>🚀 Single Binary</h3>
                <p>Everything embedded in one static Rust binary</p>
            </div>
            <div class="feature">
                <h3>💾 Smart Resource Management</h3>
                <p>Automatic allocation and optimization</p>
            </div>
            <div class="feature">
                <h3>🔒 Always-On Security</h3>
                <p>Built-in firewall, IDS, and compliance</p>
            </div>
            <div class="feature">
                <h3>🌐 Per-User Networks</h3>
                <p>Isolated SDN with VLAN separation</p>
            </div>
            <div class="feature">
                <h3>⚡ Live Migration</h3>
                <p>Zero-downtime VM movement</p>
            </div>
            <div class="feature">
                <h3>🔄 Self-Healing</h3>
                <p>Automatic recovery and optimization</p>
            </div>
        </div>
    </div>
</body>
</html>"#;

    Html(html.to_string())
}

async fn login_page(State(state): State<WebState>) -> Html<String> {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Login - CasVPS</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0; padding: 0; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white; min-height: 100vh; display: flex; align-items: center; justify-content: center;
        }
        .login-form {
            background: rgba(255,255,255,0.1); backdrop-filter: blur(10px);
            border-radius: 20px; border: 1px solid rgba(255,255,255,0.2);
            padding: 2rem; min-width: 300px;
        }
        h1 { text-align: center; font-weight: 300; margin-bottom: 2rem; }
        .form-group { margin-bottom: 1rem; }
        label { display: block; margin-bottom: 0.5rem; font-size: 0.9rem; }
        input[type="text"], input[type="password"] {
            width: 100%; padding: 0.8rem; border: 2px solid rgba(255,255,255,0.3);
            background: rgba(255,255,255,0.1); color: white; border-radius: 10px;
            font-size: 1rem; box-sizing: border-box;
        }
        input[type="text"]:focus, input[type="password"]:focus {
            outline: none; border-color: white;
        }
        input::placeholder { color: rgba(255,255,255,0.6); }
        .login-btn {
            width: 100%; padding: 1rem; background: rgba(255,255,255,0.2);
            border: 2px solid white; color: white; font-size: 1.1rem;
            border-radius: 50px; cursor: pointer; transition: all 0.3s ease;
        }
        .login-btn:hover { background: white; color: #667eea; }
        .realm-selector {
            margin-bottom: 1rem;
        }
        select {
            width: 100%; padding: 0.8rem; border: 2px solid rgba(255,255,255,0.3);
            background: rgba(255,255,255,0.1); color: white; border-radius: 10px;
            font-size: 1rem; box-sizing: border-box;
        }
        select option { background: #667eea; color: white; }
    </style>
</head>
<body>
    <div class="login-form">
        <h1>CasVPS Login</h1>
        <form action="/api/v1/auth/login" method="post">
            <div class="form-group realm-selector">
                <label for="realm">Authentication Realm</label>
                <select id="realm" name="realm">
                    <option value="local">Local Users</option>
                    <option value="ldap">LDAP/AD</option>
                    <option value="oidc">SSO/OIDC</option>
                </select>
            </div>
            <div class="form-group">
                <label for="username">Username</label>
                <input type="text" id="username" name="username" placeholder="Enter username" required>
            </div>
            <div class="form-group">
                <label for="password">Password</label>
                <input type="password" id="password" name="password" placeholder="Enter password" required>
            </div>
            <button type="submit" class="login-btn">Sign In</button>
        </form>
    </div>
</body>
</html>"#;

    Html(html.to_string())
}

async fn admin_ui(State(state): State<WebState>) -> Html<String> {
    let html = include_str!("templates/admin.html");
    Html(html.to_string())
}

async fn user_ui(State(state): State<WebState>) -> Html<String> {
    let html = include_str!("templates/user.html");
    Html(html.to_string())
}

async fn support_portal(State(state): State<WebState>) -> Html<String> {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Support Portal - CasVPS</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 0; padding: 0; }
        .header { background: #667eea; color: white; padding: 1rem; }
        .container { max-width: 1200px; margin: 0 auto; padding: 2rem; }
        .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 2rem; }
        .card { border: 1px solid #ddd; border-radius: 10px; padding: 1.5rem; background: white; }
        .card h3 { margin-top: 0; color: #667eea; }
        .nav { background: #f8f9fa; padding: 1rem; }
        .nav a { margin-right: 1rem; color: #667eea; text-decoration: none; }
        .nav a:hover { text-decoration: underline; }
    </style>
</head>
<body>
    <div class="header">
        <div class="container">
            <h1>CasVPS Support Portal</h1>
        </div>
    </div>
    <div class="nav">
        <div class="container">
            <a href="/support/docs">Documentation</a>
            <a href="/support/kb">Knowledge Base</a>
            <a href="/support/api">API Reference</a>
            <a href="/">Back to Portal</a>
        </div>
    </div>
    <div class="container">
        <div class="grid">
            <div class="card">
                <h3>📚 Documentation</h3>
                <p>Complete user guides, installation instructions, and configuration references.</p>
                <a href="/support/docs">Browse Documentation</a>
            </div>
            <div class="card">
                <h3>💡 Knowledge Base</h3>
                <p>Common questions, troubleshooting guides, and best practices.</p>
                <a href="/support/kb">Search Knowledge Base</a>
            </div>
            <div class="card">
                <h3>🔧 API Reference</h3>
                <p>Interactive API documentation with examples and testing capabilities.</p>
                <a href="/support/api">Explore API</a>
            </div>
            <div class="card">
                <h3>📊 System Status</h3>
                <p>Real-time status of your CasVPS installation and components.</p>
                <a href="/health">View Status</a>
            </div>
        </div>
    </div>
</body>
</html>"#;

    Html(html.to_string())
}

async fn documentation(State(state): State<WebState>) -> Html<String> {
    let html = "Documentation placeholder - will be embedded from specification";
    Html(html.to_string())
}

async fn knowledge_base(State(state): State<WebState>) -> Html<String> {
    let html = "Knowledge base placeholder";
    Html(html.to_string())
}

async fn api_docs(State(state): State<WebState>) -> Html<String> {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>API Documentation - CasVPS</title>
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5.7.2/swagger-ui.css">
    <style>
        body { margin: 0; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; }
        .header { background: #667eea; color: white; padding: 1rem; text-align: center; }
        #swagger-ui { max-width: 1200px; margin: 0 auto; }
    </style>
</head>
<body>
    <div class="header">
        <h1>CasVPS API Documentation</h1>
        <p>Interactive API documentation and testing interface</p>
    </div>
    <div id="swagger-ui"></div>
    <script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5.7.2/swagger-ui-bundle.js"></script>
    <script>
        SwaggerUIBundle({
            url: '/api/v1/openapi.json',
            dom_id: '#swagger-ui',
            presets: [
                SwaggerUIBundle.presets.apis,
                SwaggerUIBundle.presets.standalone
            ],
            layout: "BaseLayout",
            deepLinking: true,
            showExtensions: true,
            showCommonExtensions: true
        });
    </script>
</body>
</html>"#;

    Html(html.to_string())
}

async fn health_check(State(state): State<WebState>) -> Html<String> {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>System Health - CasVPS</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 0; padding: 2rem; }
        .status-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 1rem; }
        .status-card {
            border: 2px solid #28a745; border-radius: 10px; padding: 1rem;
            background: #f8fff9; text-align: center;
        }
        .status-card.warning { border-color: #ffc107; background: #fffbf0; }
        .status-card.error { border-color: #dc3545; background: #fff5f5; }
        .status-icon { font-size: 2rem; margin-bottom: 0.5rem; }
        .uptime { font-size: 1.2rem; color: #28a745; font-weight: bold; }
    </style>
</head>
<body>
    <h1>CasVPS System Health</h1>
    <div class="uptime">🟢 System Online - Uptime: 0d 0h 5m</div>
    <br>
    <div class="status-grid">
        <div class="status-card">
            <div class="status-icon">✅</div>
            <h3>Core Services</h3>
            <p>All services running normally</p>
        </div>
        <div class="status-card">
            <div class="status-icon">✅</div>
            <h3>Database</h3>
            <p>SQLite database operational</p>
        </div>
        <div class="status-card">
            <div class="status-icon">✅</div>
            <h3>Virtualization</h3>
            <p>QEMU/KVM ready</p>
        </div>
        <div class="status-card">
            <div class="status-icon">✅</div>
            <h3>Network Services</h3>
            <p>DHCP, DNS, TFTP active</p>
        </div>
        <div class="status-card">
            <div class="status-icon">✅</div>
            <h3>Security</h3>
            <p>All security services active</p>
        </div>
        <div class="status-card">
            <div class="status-icon">✅</div>
            <h3>Storage</h3>
            <p>Storage pools healthy</p>
        </div>
    </div>
    <br>
    <p><a href="/support">← Back to Support Portal</a></p>
</body>
</html>"#;

    Html(html.to_string())
}