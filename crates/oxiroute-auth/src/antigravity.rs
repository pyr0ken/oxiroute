use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use arc_swap::ArcSwap;
use oxiroute_core::{OxiError, Result};
use rusqlite::Connection;
use crate::crypto::{decrypt_field, derive_key};

#[derive(Debug, Clone)]
pub struct AntigravityAccount {
    pub id: String,
    pub email: String,
    pub project_id: String,
    pub access_token: Arc<ArcSwap<String>>,
    pub refresh_token: String,
    pub expires_at: Arc<ArcSwap<String>>,
    pub is_active: bool,
    pub priority: u32,
}

pub struct AntigravityAuthManager {
    accounts: Vec<AntigravityAccount>,
    current_index: AtomicUsize,
}

impl AntigravityAuthManager {
    pub fn new(accounts: Vec<AntigravityAccount>) -> Self {
        Self {
            accounts,
            current_index: AtomicUsize::new(0),
        }
    }

    pub fn load_from_omniroute_sqlite<P: AsRef<Path>>(
        db_path: P,
        env_path: P,
    ) -> Result<Self> {
        let env_content = std::fs::read_to_string(env_path.as_ref())
            .map_err(|e| OxiError::Auth(format!("Could not read OmniRoute .env file: {}", e)))?;

        let mut encryption_secret = None;
        for line in env_content.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("STORAGE_ENCRYPTION_KEY=") {
                encryption_secret = Some(val.trim_matches(['\'', '"'].as_ref()).to_string());
                break;
            }
        }

        let secret = encryption_secret
            .ok_or_else(|| OxiError::Auth("STORAGE_ENCRYPTION_KEY not found in .env".to_string()))?;

        let key = derive_key(&secret)?;

        let conn = Connection::open(db_path.as_ref())
            .map_err(|e| OxiError::Auth(format!("Could not open OmniRoute sqlite db: {}", e)))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, email, access_token, refresh_token, project_id, expires_at, is_active, priority \
                 FROM provider_connections WHERE provider = 'antigravity' AND is_active = 1 \
                 ORDER BY priority ASC",
            )
            .map_err(|e| OxiError::Auth(format!("Sqlite query prep failed: {}", e)))?;

        let account_rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            })
            .map_err(|e| OxiError::Auth(format!("Query execution failed: {}", e)))?;

        let mut accounts = Vec::new();

        for row in account_rows {
            let (id, email, enc_acc, enc_ref, project_id, expires_at, is_active, priority) =
                row.map_err(|e| OxiError::Auth(format!("Row read error: {}", e)))?;

            let access_token = decrypt_field(&enc_acc, &key)?;
            let refresh_token = decrypt_field(&enc_ref, &key)?;

            tracing::info!(
                "Loaded Antigravity account: {} (Project: {}, priority: {})",
                email,
                project_id,
                priority
            );

            accounts.push(AntigravityAccount {
                id,
                email,
                project_id,
                access_token: Arc::new(ArcSwap::from_pointee(access_token)),
                refresh_token,
                expires_at: Arc::new(ArcSwap::from_pointee(expires_at)),
                is_active: is_active == 1,
                priority: priority as u32,
            });
        }

        if accounts.is_empty() {
            return Err(OxiError::Auth(
                "No active Antigravity accounts found in OmniRoute database".to_string(),
            ));
        }

        Ok(Self::new(accounts))
    }

    pub fn get_account(&self) -> Option<AntigravityAccount> {
        if self.accounts.is_empty() {
            return None;
        }
        let idx = self.current_index.fetch_add(1, Ordering::Relaxed) % self.accounts.len();
        Some(self.accounts[idx].clone())
    }

    pub fn list_accounts(&self) -> &[AntigravityAccount] {
        &self.accounts
    }
}
