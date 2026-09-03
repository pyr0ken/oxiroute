use std::path::{Path, PathBuf};
use oxiroute_core::{OxiError, Result};
use rusqlite::Connection;

pub fn default_oxiroute_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("OXI_DATA_DIR") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".oxiroute")
}

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
        let conn = Connection::open(&self.db_path)
            .map_err(|e| OxiError::Internal(format!("Failed to open sqlite: {}", e)))?;

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
                models_json TEXT NOT NULL
            );",
        )
        .map_err(|e| OxiError::Internal(format!("Failed to init sqlite schema: {}", e)))?;

        // 3. Auto-migrate from OmniRoute if Oxiroute db is empty and OmniRoute db exists
        let count: i64 = conn
            .query_row("SELECT count(*) FROM provider_connections", [], |r| r.get(0))
            .unwrap_or(0);

        if count == 0 {
            if let Some(home) = dirs::home_dir() {
                let omniroute_db = home.join(".omniroute/storage.sqlite");
                let omniroute_env = home.join(".omniroute/.env");

                if omniroute_db.exists() && omniroute_env.exists() {
                    tracing::info!("Found OmniRoute data. Auto-migrating into Oxiroute native storage...");
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

        let dest_conn = Connection::open(&self.db_path)
            .map_err(|e| OxiError::Internal(format!("Failed to open destination Oxiroute db: {}", e)))?;

        // 1. Migrate provider_connections
        let mut stmt = src_conn
            .prepare("SELECT id, provider, auth_type, name, email, priority, is_active, access_token, refresh_token, project_id, expires_at, scope, provider_specific_data, created_at, updated_at FROM provider_connections")
            .map_err(|e| OxiError::Internal(format!("Prepare failed: {}", e)))?;

        let rows = stmt
            .query_map([], |r| {
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
            })
            .map_err(|e| OxiError::Internal(format!("Query failed: {}", e)))?;

        let mut count = 0;
        for row_res in rows {
            if let Ok((id, provider, auth_type, name, email, priority, is_active, access_token, refresh_token, project_id, expires_at, scope, provider_specific_data, created_at, updated_at)) = row_res {
                let _ = dest_conn.execute(
                    "INSERT OR REPLACE INTO provider_connections (id, provider, auth_type, name, email, priority, is_active, access_token, refresh_token, project_id, expires_at, scope, provider_specific_data, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                    rusqlite::params![
                        id, provider, auth_type, name, email, priority, is_active, access_token, refresh_token, project_id, expires_at, scope, provider_specific_data, created_at, updated_at
                    ],
                );
                count += 1;
            }
        }

        // 2. Migrate proxy_registry
        if let Ok(mut proxy_stmt) = src_conn.prepare("SELECT id, name, protocol, host, port, username, password, status FROM proxy_registry") {
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
                }
            }
        }

        tracing::info!("Successfully imported {} provider connections and proxies into Oxiroute!", count);

        Ok(count)
    }
}
