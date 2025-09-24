use anyhow::Result;
use std::sync::Arc;
use crate::database::Database;

pub struct ConfigManager {
    database: Arc<Database>,
}

impl ConfigManager {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    pub async fn get(&self, key: &str) -> Result<String> {
        self.database.get_config(key).await
    }

    pub async fn set(&self, key: &str, value: &str) -> Result<()> {
        self.database.set_config(key, value).await
    }
}