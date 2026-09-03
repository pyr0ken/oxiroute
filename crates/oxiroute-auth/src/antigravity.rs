use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use arc_swap::ArcSwap;
use oxiroute_core::{OxiError, Result};
use rusqlite::Connection;
use crate::crypto::{decrypt_field, derive_key};

pub const GOOGLE_OAUTH_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

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

        let secret = encryption_secret.ok_or_else(|| {
            OxiError::Auth("STORAGE_ENCRYPTION_KEY not found in .env".to_string())
        })?;

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

    pub fn get_accounts_ordered(&self) -> Vec<AntigravityAccount> {
        let n = self.accounts.len();
        if n == 0 {
            return Vec::new();
        }

        let start_idx = self.current_index.fetch_add(1, Ordering::Relaxed) % n;
        let mut ordered = Vec::with_capacity(n);

        for i in 0..n {
            let idx = (start_idx + i) % n;
            ordered.push(self.accounts[idx].clone());
        }

        ordered
    }

    pub fn list_accounts(&self) -> &[AntigravityAccount] {
        &self.accounts
    }

    pub async fn test_account_connection(
        account: &AntigravityAccount,
        proxy_url: Option<&str>,
    ) -> std::result::Result<u64, String> {
        let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(8));
        if let Some(p) = proxy_url {
            if let Ok(proxy) = reqwest::Proxy::all(p) {
                builder = builder.proxy(proxy);
            }
        }
        let client = builder.build().map_err(|e| format!("Client build error: {}", e))?;

        let start = Instant::now();
        // Probe Google OAuth endpoint with refresh token
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", &account.refresh_token),
        ];

        let res = client
            .post(GOOGLE_OAUTH_TOKEN_URL)
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Network probe failed (proxy/route error): {}", e))?;

        let latency_ms = start.elapsed().as_millis() as u64;

        if res.status().is_success() {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(tok) = json.get("access_token").and_then(|t| t.as_str()) {
                    account.access_token.store(Arc::new(tok.to_string()));
                }
            }
            Ok(latency_ms)
        } else {
            let status = res.status().as_u16();
            Err(format!("HTTP status {}", status))
        }
    }

    pub async fn refresh_account_token(
        account: &AntigravityAccount,
        client: &reqwest::Client,
    ) -> Result<String> {
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", &account.refresh_token),
        ];

        let res = client
            .post(GOOGLE_OAUTH_TOKEN_URL)
            .form(&params)
            .send()
            .await
            .map_err(|e| OxiError::Auth(format!("Token refresh HTTP dispatch failed: {}", e)))?;

        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = res.text().await.unwrap_or_default();
            return Err(OxiError::Auth(format!(
                "Token refresh rejected (status {}): {}",
                status, body
            )));
        }

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| OxiError::Auth(format!("Token refresh JSON parse error: {}", e)))?;

        let new_token = json
            .get("access_token")
            .and_then(|t| t.as_str())
            .ok_or_else(|| OxiError::Auth("Missing access_token in refresh response".to_string()))?;

        account
            .access_token
            .store(Arc::new(new_token.to_string()));

        tracing::info!(
            "Successfully refreshed access token for Antigravity account: {}",
            account.email
        );

        Ok(new_token.to_string())
    }

    pub fn spawn_background_refresher(self: Arc<Self>, proxy_url: Option<String>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 minutes
            let mut builder = reqwest::Client::builder();
            if let Some(ref p) = proxy_url {
                if let Ok(proxy) = reqwest::Proxy::all(p) {
                    builder = builder.proxy(proxy);
                }
            }
            let client = builder.build().unwrap_or_default();

            loop {
                interval.tick().await;
                for account in self.list_accounts() {
                    // Refresh token proactively
                    let _ = Self::refresh_account_token(account, &client).await;
                }
            }
        });
    }
}
