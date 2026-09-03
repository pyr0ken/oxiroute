pub mod antigravity;

pub use antigravity::AntigravityExecutor;

use std::sync::Arc;
use dashmap::DashMap;
use oxiroute_core::ProviderConfig;

pub struct Engine {
    providers: Arc<DashMap<String, ProviderConfig>>,
    antigravity_executor: Option<Arc<AntigravityExecutor>>,
}

impl Engine {
    pub fn new(proxy_url: Option<&str>) -> Self {
        let antigravity_executor = AntigravityExecutor::new(proxy_url).ok().map(Arc::new);

        Self {
            providers: Arc::new(DashMap::new()),
            antigravity_executor,
        }
    }

    pub fn antigravity(&self) -> Option<Arc<AntigravityExecutor>> {
        self.antigravity_executor.clone()
    }

    pub fn register_provider(&self, config: ProviderConfig) {
        self.providers.insert(config.id.clone(), config);
    }

    pub fn get_provider(&self, id: &str) -> Option<ProviderConfig> {
        self.providers.get(id).map(|p| p.value().clone())
    }

    pub fn list_providers(&self) -> Vec<ProviderConfig> {
        self.providers.iter().map(|p| p.value().clone()).collect()
    }
}
