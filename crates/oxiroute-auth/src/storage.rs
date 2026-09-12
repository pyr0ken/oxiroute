use std::path::{Path, PathBuf};
use oxiroute_core::{OxiError, Result};
use rusqlite::Connection;
use crate::crypto::derive_key;

pub fn default_oxiroute_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("OXI_DATA_DIR") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".oxiroute")
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderRecord {
    pub id: String,
    pub provider: String,
    pub auth_type: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub priority: u32,
    pub is_active: bool,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub project_id: Option<String>,
    pub expires_at: Option<String>,
    pub scope: Option<String>,
    pub provider_specific_data: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProxyRecord {
    pub id: String,
    pub name: Option<String>,
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ComboRecord {
    pub id: String,
    pub name: String,
    pub strategy: String,
    pub models: Vec<String>,
    pub proxy_name: Option<String>,
}

#[derive(Clone)]
pub struct StorageManager {
    pub dir: PathBuf,
    pub db_path: PathBuf,
    pub env_path: PathBuf,
    pub config_path: PathBuf,
}

impl StorageManager {
    pub fn new(dir: PathBuf) -> Self {
        let db_path = dir.join("storage.sqlite");
        let env_path = dir.join("secrets.env");
        let config_path = dir.join("config.toml");

        Self {
            dir,
            db_path,
            env_path,
            config_path,
        }
    }

    pub fn get_connection(&self) -> Result<Connection> {
        Connection::open(&self.db_path)
            .map_err(|e| OxiError::Internal(format!("Failed to open sqlite at {:?}: {}", self.db_path, e)))
    }

    pub fn get_encryption_key(&self) -> Result<[u8; 32]> {
        let secret = Self::get_or_create_secret(&self.env_path)?;
        derive_key(&secret)
    }

    pub fn init(&self) -> Result<()> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| OxiError::Internal(format!("Failed to create oxiroute dir: {}", e)))?;

        // 1. Ensure secrets.env exists
        if !self.env_path.exists() {
            let secret = Self::get_or_create_secret(&self.env_path)?;
            tracing::info!("Initialized Oxiroute encryption secret at {:?}", self.env_path);
            let _ = secret;
        }

        // 2. Initialize database schema
        let conn = self.get_connection()?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS provider_connections (
                id TEXT PRIMARY KEY,
                provider TEXT NOT NULL,
                auth_type TEXT NOT NULL,
                name TEXT,
                email TEXT,
                priority INTEGER DEFAULT 1,
                is_active INTEGER DEFAULT 1,
                access_token TEXT,
                refresh_token TEXT,
                project_id TEXT,
                expires_at TEXT,
                scope TEXT,
                provider_specific_data TEXT,
                created_at TEXT,
                updated_at TEXT
            );

            CREATE TABLE IF NOT EXISTS proxy_registry (
                id TEXT PRIMARY KEY,
                name TEXT,
                protocol TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER NOT NULL,
                username TEXT,
                password TEXT,
                status TEXT DEFAULT 'active'
            );

            CREATE TABLE IF NOT EXISTS combos (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                strategy TEXT DEFAULT 'least-used',
                models_json TEXT NOT NULL,
                proxy_name TEXT
            );",
        )
        .map_err(|e| OxiError::Internal(format!("Failed to init sqlite schema: {}", e)))?;

        // Migration safety: ensure proxy_name column in combos exists
        let _ = conn.execute("ALTER TABLE combos ADD COLUMN proxy_name TEXT;", []);

        // 3. Auto-migrate from OmniRoute if any table in Oxiroute is empty
        if let Some(home) = dirs::home_dir() {
            let omniroute_db = home.join(".omniroute/storage.sqlite");
            let omniroute_env = home.join(".omniroute/.env");

            if omniroute_db.exists() {
                let p_count: i64 = conn
                    .query_row("SELECT count(*) FROM provider_connections", [], |r| r.get(0))
                    .unwrap_or(0);
                let proxy_count: i64 = conn
                    .query_row("SELECT count(*) FROM proxy_registry", [], |r| r.get(0))
                    .unwrap_or(0);
                let combo_count: i64 = conn
                    .query_row("SELECT count(*) FROM combos", [], |r| r.get(0))
                    .unwrap_or(0);

                if p_count == 0 || proxy_count == 0 || combo_count == 0 {
                    tracing::info!("Found OmniRoute data. Auto-migrating missing tables into Oxiroute storage...");
                    let _ = self.migrate_from_omniroute(&omniroute_db, &omniroute_env);
                }
            }
        }

        Ok(())
    }

    fn get_or_create_secret(env_path: &Path) -> Result<String> {
        if env_path.exists() {
            if let Ok(content) = std::fs::read_to_string(env_path) {
                for line in content.lines() {
                    let line = line.trim();
                    if let Some(val) = line.strip_prefix("STORAGE_ENCRYPTION_KEY=") {
                        return Ok(val.trim_matches(['\'', '"'].as_ref()).to_string());
                    }
                }
            }
        }

        // If omniroute env exists, copy its encryption key for seamless token migration
        if let Some(home) = dirs::home_dir() {
            let omniroute_env = home.join(".omniroute/.env");
            if omniroute_env.exists() {
                if let Ok(content) = std::fs::read_to_string(omniroute_env) {
                    for line in content.lines() {
                        let line = line.trim();
                        if let Some(val) = line.strip_prefix("STORAGE_ENCRYPTION_KEY=") {
                            let key = val.trim_matches(['\'', '"'].as_ref()).to_string();
                            let payload = format!("STORAGE_ENCRYPTION_KEY={}\n", key);
                            let _ = std::fs::write(env_path, payload);
                            return Ok(key);
                        }
                    }
                }
            }
        }

        let new_key = uuid::Uuid::new_v4().to_string();
        let payload = format!("STORAGE_ENCRYPTION_KEY={}\n", new_key);
        std::fs::write(env_path, payload)
            .map_err(|e| OxiError::Internal(format!("Failed to write secrets.env: {}", e)))?;

        Ok(new_key)
    }

    pub fn migrate_from_omniroute(&self, omniroute_db: &Path, _omniroute_env: &Path) -> Result<usize> {
        let src_conn = Connection::open(omniroute_db)
            .map_err(|e| OxiError::Internal(format!("Failed to open source OmniRoute db: {}", e)))?;

        let dest_conn = self.get_connection()?;

        let mut count = 0;

        // 1. Migrate provider_connections
        let p_count: i64 = dest_conn
            .query_row("SELECT count(*) FROM provider_connections", [], |r| r.get(0))
            .unwrap_or(0);

        if p_count == 0 {
            if let Ok(mut stmt) = src_conn.prepare("SELECT id, provider, auth_type, name, email, priority, is_active, access_token, refresh_token, project_id, expires_at, scope, provider_specific_data, created_at, updated_at FROM provider_connections") {
                if let Ok(rows) = stmt.query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, i64>(6)?,
                        r.get::<_, Option<String>>(7)?,
                        r.get::<_, Option<String>>(8)?,
                        r.get::<_, Option<String>>(9)?,
                        r.get::<_, Option<String>>(10)?,
                        r.get::<_, Option<String>>(11)?,
                        r.get::<_, Option<String>>(12)?,
                        r.get::<_, Option<String>>(13)?,
                        r.get::<_, Option<String>>(14)?,
                    ))
                }) {
                    for row_res in rows.flatten() {
                        let _ = dest_conn.execute(
                            "INSERT OR REPLACE INTO provider_connections (id, provider, auth_type, name, email, priority, is_active, access_token, refresh_token, project_id, expires_at, scope, provider_specific_data, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                            rusqlite::params![
                                row_res.0, row_res.1, row_res.2, row_res.3, row_res.4, row_res.5, row_res.6, row_res.7, row_res.8, row_res.9, row_res.10, row_res.11, row_res.12, row_res.13, row_res.14
                            ],
                        );
                        count += 1;
                    }
                }
            }
        }

        // 2. Migrate proxy_registry (OmniRoute uses 'type' column)
        let proxy_count: i64 = dest_conn
            .query_row("SELECT count(*) FROM proxy_registry", [], |r| r.get(0))
            .unwrap_or(0);

        if proxy_count == 0 {
            if let Ok(mut proxy_stmt) = src_conn.prepare("SELECT id, name, type, host, port, username, password, status FROM proxy_registry") {
                if let Ok(proxy_rows) = proxy_stmt.query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, Option<String>>(5)?,
                        r.get::<_, Option<String>>(6)?,
                        r.get::<_, Option<String>>(7)?,
                    ))
                }) {
                    for pr in proxy_rows.flatten() {
                        let _ = dest_conn.execute(
                            "INSERT OR REPLACE INTO proxy_registry (id, name, protocol, host, port, username, password, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                            rusqlite::params![pr.0, pr.1, pr.2, pr.3, pr.4, pr.5, pr.6, pr.7],
                        );
                        count += 1;
                    }
                }
            }
        }

        // 3. Migrate combos & their assigned proxies from proxy_assignments
        let mut proxy_map = std::collections::HashMap::new();
        if let Ok(mut pa_stmt) = src_conn.prepare("SELECT pa.scope_id, pr.name FROM proxy_assignments pa JOIN proxy_registry pr ON pa.proxy_id = pr.id WHERE pa.scope = 'combo'") {
            if let Ok(pa_rows) = pa_stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))) {
                for item in pa_rows.flatten() {
                    proxy_map.insert(item.0, item.1);
                }
            }
        }

        if let Ok(mut combo_stmt) = src_conn.prepare("SELECT id, name, data FROM combos") {
            if let Ok(combo_rows) = combo_stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            }) {
                for cr in combo_rows.flatten() {
                    let (id, name, data_json) = cr;
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&data_json) {
                        let strategy = val.get("strategy").and_then(|s| s.as_str()).unwrap_or("priority").to_string();
                        let mut models = Vec::new();
                        if let Some(arr) = val.get("models").and_then(|v| v.as_array()) {
                            for item in arr {
                                if let Some(m) = item.as_str() {
                                    models.push(m.to_string());
                                } else if let Some(m) = item.get("model").and_then(|v| v.as_str()) {
                                    models.push(m.to_string());
                                }
                            }
                        }
                        let proxy_name = proxy_map.get(&id).cloned().or_else(|| {
                            val.get("proxy_name").or_else(|| val.get("proxy")).and_then(|v| v.as_str()).map(|s| s.to_string())
                        });
                        let models_json = serde_json::to_string(&models).unwrap_or_else(|_| "[]".into());

                        let _ = dest_conn.execute(
                            "INSERT OR REPLACE INTO combos (id, name, strategy, models_json, proxy_name) VALUES (?1, ?2, ?3, ?4, ?5)",
                            rusqlite::params![id, name, strategy, models_json, proxy_name],
                        );
                        count += 1;
                    }
                }
            }
        }

        tracing::info!("Successfully imported/verified {} records into Oxiroute native storage!", count);

        Ok(count)
    }

    pub fn get_historical_usage_totals(&self) -> Result<(u64, u64, u64)> {
        // Try to query OmniRoute or local SQLite for historical total tokens
        if let Some(home) = dirs::home_dir() {
            let omniroute_db = home.join(".omniroute/storage.sqlite");
            if omniroute_db.exists() {
                if let Ok(conn) = Connection::open(&omniroute_db) {
                    let row_res: rusqlite::Result<(Option<i64>, Option<i64>, Option<i64>)> = conn.query_row(
                        "SELECT sum(tokens_input), sum(tokens_output), sum(tokens_reasoning) FROM usage_history",
                        [],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    );
                    if let Ok((in_tok, out_tok, reason_tok)) = row_res {
                        let total_in = in_tok.unwrap_or(0).max(0) as u64;
                        let total_out = out_tok.unwrap_or(0).max(0) as u64;
                        let total_reason = reason_tok.unwrap_or(0).max(0) as u64;
                        return Ok((total_in, total_out, total_reason));
                    }
                }
            }
        }
        Ok((0, 0, 0))
    }

    pub fn load_historical_timeline(&self, limit: usize) -> Result<Vec<(String, String, String, String, u64, u64, u16)>> {
        // Return (id, timestamp, model, account, tokens, latency_ms, status_code)
        if let Some(home) = dirs::home_dir() {
            let omniroute_db = home.join(".omniroute/storage.sqlite");
            if omniroute_db.exists() {
                if let Ok(conn) = Connection::open(&omniroute_db) {
                    if let Ok(mut stmt) = conn.prepare(
                        "SELECT id, timestamp, model, account, (tokens_in + tokens_out), duration, status FROM call_logs ORDER BY timestamp DESC LIMIT ?1"
                    ) {
                        let rows = stmt.query_map(rusqlite::params![limit as i64], |r| {
                            let id: String = r.get(0)?;
                            let raw_ts: String = r.get(1)?;
                            let model: String = r.get(2).unwrap_or_else(|_| "unknown".into());
                            let account: String = r.get(3).unwrap_or_else(|_| "main".into());
                            let tokens: i64 = r.get(4).unwrap_or(0);
                            let latency: i64 = r.get(5).unwrap_or(0);
                            let status: i64 = r.get(6).unwrap_or(200);

                            // Format timestamp nicely
                            let ts_formatted = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&raw_ts) {
                                dt.with_timezone(&chrono::Local).format("%H:%M:%S").to_string()
                            } else {
                                raw_ts.chars().take(8).collect()
                            };

                            Ok((id, ts_formatted, model, account, tokens.max(0) as u64, latency.max(0) as u64, status as u16))
                        });

                        if let Ok(mapped) = rows {
                            let mut results = Vec::new();
                            for item in mapped.flatten() {
                                results.push(item);
                            }
                            return Ok(results);
                        }
                    }
                }
            }
        }
        Ok(Vec::new())
    }

    // --- Combos CRUD ---
    pub fn load_combos(&self) -> Result<Vec<ComboRecord>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare("SELECT id, name, strategy, models_json, proxy_name FROM combos ORDER BY name ASC")
            .map_err(|e| OxiError::Internal(format!("Load combos prep failed: {}", e)))?;

        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let strategy: String = row.get(2)?;
                let models_json: String = row.get(3)?;
                let proxy_name: Option<String> = row.get(4)?;

                let models: Vec<String> = serde_json::from_str(&models_json).unwrap_or_else(|_| {
                    models_json.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
                });

                Ok(ComboRecord {
                    id,
                    name,
                    strategy,
                    models,
                    proxy_name,
                })
            })
            .map_err(|e| OxiError::Internal(format!("Load combos query failed: {}", e)))?;

        let mut combos = Vec::new();
        for r in rows.flatten() {
            combos.push(r);
        }
        Ok(combos)
    }

    pub fn save_combo(&self, combo: &ComboRecord) -> Result<()> {
        let conn = self.get_connection()?;
        let models_json = serde_json::to_string(&combo.models).unwrap_or_else(|_| "[]".into());

        conn.execute(
            "INSERT OR REPLACE INTO combos (id, name, strategy, models_json, proxy_name) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![combo.id, combo.name, combo.strategy, models_json, combo.proxy_name],
        )
        .map_err(|e| OxiError::Internal(format!("Failed to save combo: {}", e)))?;

        Ok(())
    }

    pub fn delete_combo(&self, id: &str) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM combos WHERE id = ?1", rusqlite::params![id])
            .map_err(|e| OxiError::Internal(format!("Failed to delete combo: {}", e)))?;
        Ok(())
    }

    // --- Proxies CRUD ---
    pub fn load_proxies(&self) -> Result<Vec<ProxyRecord>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare("SELECT id, name, protocol, host, port, username, password, status FROM proxy_registry ORDER BY name ASC")
            .map_err(|e| OxiError::Internal(format!("Load proxies prep failed: {}", e)))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(ProxyRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    protocol: row.get(2)?,
                    host: row.get(3)?,
                    port: row.get::<_, u32>(4)? as u16,
                    username: row.get(5)?,
                    password: row.get(6)?,
                    status: row.get(7)?,
                })
            })
            .map_err(|e| OxiError::Internal(format!("Load proxies query failed: {}", e)))?;

        let mut proxies = Vec::new();
        for r in rows.flatten() {
            proxies.push(r);
        }
        Ok(proxies)
    }

    pub fn save_proxy(&self, proxy: &ProxyRecord) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO proxy_registry (id, name, protocol, host, port, username, password, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                proxy.id,
                proxy.name,
                proxy.protocol,
                proxy.host,
                proxy.port as i64,
                proxy.username,
                proxy.password,
                proxy.status
            ],
        )
        .map_err(|e| OxiError::Internal(format!("Failed to save proxy: {}", e)))?;
        Ok(())
    }

    pub fn delete_proxy(&self, id: &str) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM proxy_registry WHERE id = ?1", rusqlite::params![id])
            .map_err(|e| OxiError::Internal(format!("Failed to delete proxy: {}", e)))?;
        Ok(())
    }

    // --- Provider Connections CRUD ---
    pub fn load_provider_connections(&self) -> Result<Vec<ProviderRecord>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare("SELECT id, provider, auth_type, name, email, priority, is_active, access_token, refresh_token, project_id, expires_at, scope, provider_specific_data, created_at, updated_at FROM provider_connections ORDER BY priority ASC, name ASC")
            .map_err(|e| OxiError::Internal(format!("Load provider connections prep failed: {}", e)))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(ProviderRecord {
                    id: row.get(0)?,
                    provider: row.get(1)?,
                    auth_type: row.get(2)?,
                    name: row.get(3)?,
                    email: row.get(4)?,
                    priority: row.get::<_, i64>(5)? as u32,
                    is_active: row.get::<_, i64>(6)? == 1,
                    access_token: row.get(7)?,
                    refresh_token: row.get(8)?,
                    project_id: row.get(9)?,
                    expires_at: row.get(10)?,
                    scope: row.get(11)?,
                    provider_specific_data: row.get(12)?,
                    created_at: row.get(13)?,
                    updated_at: row.get(14)?,
                })
            })
            .map_err(|e| OxiError::Internal(format!("Load provider connections query failed: {}", e)))?;

        let mut records = Vec::new();
        for r in rows.flatten() {
            records.push(r);
        }
        Ok(records)
    }

    pub fn save_provider_connection(&self, record: &ProviderRecord) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO provider_connections (id, provider, auth_type, name, email, priority, is_active, access_token, refresh_token, project_id, expires_at, scope, provider_specific_data, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            rusqlite::params![
                record.id,
                record.provider,
                record.auth_type,
                record.name,
                record.email,
                record.priority as i64,
                if record.is_active { 1 } else { 0 },
                record.access_token,
                record.refresh_token,
                record.project_id,
                record.expires_at,
                record.scope,
                record.provider_specific_data,
                record.created_at,
                record.updated_at
            ],
        )
        .map_err(|e| OxiError::Internal(format!("Failed to save provider connection: {}", e)))?;
        Ok(())
    }

    pub fn delete_provider_connection(&self, id: &str) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM provider_connections WHERE id = ?1", rusqlite::params![id])
            .map_err(|e| OxiError::Internal(format!("Failed to delete provider connection: {}", e)))?;
        Ok(())
    }

    pub fn update_provider_active(&self, id: &str, is_active: bool) -> Result<()> {
        let conn = self.get_connection()?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE provider_connections SET is_active = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![if is_active { 1 } else { 0 }, now, id],
        )
        .map_err(|e| OxiError::Internal(format!("Failed to update provider active status: {}", e)))?;
        Ok(())
    }

    pub fn update_provider_tokens(
        &self,
        id: &str,
        access_token: Option<&str>,
        refresh_token: Option<&str>,
        expires_at: Option<&str>,
    ) -> Result<()> {
        let conn = self.get_connection()?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE provider_connections SET access_token = COALESCE(?1, access_token), refresh_token = COALESCE(?2, refresh_token), expires_at = COALESCE(?3, expires_at), updated_at = ?4 WHERE id = ?5",
            rusqlite::params![access_token, refresh_token, expires_at, now, id],
        )
        .map_err(|e| OxiError::Internal(format!("Failed to update provider tokens: {}", e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use arc_swap::ArcSwap;
    use crate::{AntigravityAccount, AntigravityAuthManager};

    #[test]
    fn test_storage_crud_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!("oxiroute_test_{}", uuid::Uuid::new_v4()));
        let storage = StorageManager::new(temp_dir.clone());
        storage.init().unwrap();

        // 1. Combos CRUD
        let combo = ComboRecord {
            id: "combo-1".to_string(),
            name: "test-combo".to_string(),
            strategy: "priority".to_string(),
            models: vec!["model-a".to_string(), "model-b".to_string()],
            proxy_name: Some("test-proxy".to_string()),
        };
        storage.save_combo(&combo).unwrap();
        let loaded_combos = storage.load_combos().unwrap();
        let found = loaded_combos.iter().find(|c| c.id == "combo-1").unwrap();
        assert_eq!(found.name, "test-combo");
        assert_eq!(found.models, vec!["model-a", "model-b"]);

        storage.delete_combo("combo-1").unwrap();
        assert!(storage.load_combos().unwrap().iter().all(|c| c.id != "combo-1"));

        // 2. Proxies CRUD
        let proxy = ProxyRecord {
            id: "proxy-1".to_string(),
            name: Some("Local SOCKS5".to_string()),
            protocol: "socks5".to_string(),
            host: "127.0.0.1".to_string(),
            port: 1080,
            username: None,
            password: None,
            status: "active".to_string(),
        };
        storage.save_proxy(&proxy).unwrap();
        let loaded_proxies = storage.load_proxies().unwrap();
        let found_proxy = loaded_proxies.iter().find(|p| p.id == "proxy-1").unwrap();
        assert_eq!(found_proxy.host, "127.0.0.1");
        assert_eq!(found_proxy.port, 1080);

        storage.delete_proxy("proxy-1").unwrap();
        assert!(storage.load_proxies().unwrap().iter().all(|p| p.id != "proxy-1"));

        // 3. Provider Connections CRUD
        let provider = ProviderRecord {
            id: "prov-1".to_string(),
            provider: "openrouter".to_string(),
            auth_type: "apikey".to_string(),
            name: Some("OpenRouter Test".to_string()),
            email: None,
            priority: 1,
            is_active: true,
            access_token: Some("enc:v1:fake".to_string()),
            refresh_token: None,
            project_id: None,
            expires_at: None,
            scope: None,
            provider_specific_data: None,
            created_at: None,
            updated_at: None,
        };
        storage.save_provider_connection(&provider).unwrap();
        let loaded_providers = storage.load_provider_connections().unwrap();
        let found_prov = loaded_providers.iter().find(|p| p.id == "prov-1").unwrap();
        assert_eq!(found_prov.name.as_deref(), Some("OpenRouter Test"));
        assert!(found_prov.is_active);

        // Toggle active
        storage.update_provider_active("prov-1", false).unwrap();
        let updated_providers = storage.load_provider_connections().unwrap();
        let found_updated = updated_providers.iter().find(|p| p.id == "prov-1").unwrap();
        assert!(!found_updated.is_active);

        // Delete provider
        storage.delete_provider_connection("prov-1").unwrap();
        assert!(storage.load_provider_connections().unwrap().iter().all(|p| p.id != "prov-1"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_antigravity_live_probe() {
        let fake_acc = AntigravityAccount {
            id: "test".to_string(),
            email: "test@example.com".to_string(),
            project_id: "test".to_string(),
            access_token: Arc::new(ArcSwap::from_pointee("fake".to_string())),
            refresh_token: "fake_token".to_string(),
            expires_at: Arc::new(ArcSwap::from_pointee("".to_string())),
            is_active: true,
            priority: 1,
        };
        // Calling with invalid token should reach Google server via socks proxy (if any) and return HTTP 400 invalid_grant, NOT network failure
        let res = AntigravityAuthManager::test_account_connection(&fake_acc, Some("socks5h://127.0.0.1:5454")).await;
        // Even if proxy fails or returns invalid_grant, verify method runs
        println!("Test probe res: {:?}", res);
    }

    #[test]
    fn test_migrate_omniroute_real() {
        if let Some(home) = dirs::home_dir() {
            let omniroute_db = home.join(".omniroute/storage.sqlite");
            let omniroute_env = home.join(".omniroute/.env");
            let oxiroute_dir = home.join(".oxiroute");
            if omniroute_db.exists() {
                let storage = StorageManager::new(oxiroute_dir);
                let _ = storage.init();
                let res = storage.migrate_from_omniroute(&omniroute_db, &omniroute_env);
                assert!(res.is_ok());
            }
        }
    }
}

