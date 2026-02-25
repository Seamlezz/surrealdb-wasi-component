#[derive(Debug, Clone)]
pub enum Auth {
    Root { username: String, password: String },
    Namespace { username: String, password: String },
    Database { username: String, password: String },
}

impl Default for Auth {
    fn default() -> Self {
        Self::Root {
            username: "root".to_string(),
            password: "root".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SurrealHostConfig {
    pub url: String,
    pub namespace: String,
    pub database: String,
    pub auth: Auth,
}

impl SurrealHostConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.url.trim().is_empty(), "url must not be empty");
        anyhow::ensure!(
            !self.namespace.trim().is_empty(),
            "namespace must not be empty"
        );
        anyhow::ensure!(
            !self.database.trim().is_empty(),
            "database must not be empty"
        );
        Ok(())
    }
}
