//! # pleme-config
//!
//! Configuration management library for Pleme platform services.
//!
//! ## Philosophy
//!
//! This library implements 12-Factor App (Heroku) principles:
//! - Store config in the environment
//! - Strict separation of config from code
//! - Config varies between deployments (dev, staging, production)
//! - No config files in version control
//!
//! ## Usage
//!
//! ```rust
//! use pleme_config::{ConfigLoader, Result};
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct ServiceConfig {
//!     #[serde(alias = "DATABASE_URL")]
//!     database_url: String,
//!
//!     #[serde(default = "default_port")]
//!     port: u16,
//! }
//!
//! fn default_port() -> u16 { 8080 }
//!
//! let config = ServiceConfig::load()?;
//! ```
//!
//! ## Features
//!
//! - `multi-source` - Load from multiple sources (env, files, etc.)
//! - `validation` - Runtime configuration validation
//! - `formats` - Support for JSON, YAML, TOML files
//! - `secrets` - Secrets management integration
//! - `database` - Database URL parsing helpers
//! - `full` - All features enabled

use serde::de::DeserializeOwned;
use std::net::SocketAddr;
use std::str::FromStr;
use thiserror::Error;

/// Configuration error types
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Missing required environment variable
    #[error("Missing required environment variable: {0}")]
    MissingEnv(String),

    /// Invalid environment variable value
    #[error("Invalid value for {variable}: {message}")]
    InvalidValue {
        variable: String,
        message: String,
    },

    /// Configuration validation failed
    #[error("Configuration validation failed: {0}")]
    Validation(String),

    /// Failed to parse configuration
    #[error("Failed to parse configuration: {0}")]
    Parse(String),

    /// Failed to load configuration file
    #[error("Failed to load configuration file: {0}")]
    FileLoad(String),

    /// Secrets decryption failed
    #[error("Failed to decrypt secret: {0}")]
    SecretDecryption(String),
}

/// Result type for configuration operations
pub type Result<T> = std::result::Result<T, ConfigError>;

/// Trait for loading configuration from environment
pub trait ConfigLoader: DeserializeOwned + Sized {
    /// Load configuration from environment variables
    ///
    /// ## Example
    ///
    /// ```rust
    /// use pleme_config::{ConfigLoader, Result};
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct MyConfig {
    ///     #[serde(alias = "DATABASE_URL")]
    ///     database_url: String,
    /// }
    ///
    /// impl ConfigLoader for MyConfig {}
    ///
    /// let config = MyConfig::load()?;
    /// ```
    fn load() -> Result<Self> {
        envy::from_env::<Self>()
            .map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Load configuration with a prefix
    ///
    /// ## Example
    ///
    /// ```rust
    /// // Loads MYAPP_DATABASE_URL instead of DATABASE_URL
    /// let config = MyConfig::load_with_prefix("MYAPP")?;
    /// ```
    fn load_with_prefix(prefix: &str) -> Result<Self> {
        envy::prefixed(prefix).from_env::<Self>()
            .map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Validate configuration after loading
    ///
    /// Override this method to add custom validation logic.
    fn validate(&self) -> Result<()> {
        Ok(())
    }

    /// Load and validate configuration
    fn load_and_validate() -> Result<Self> {
        let config = Self::load()?;
        config.validate()?;
        Ok(config)
    }
}

/// Helper for creating multi-case configuration fields
///
/// Supports multiple naming conventions:
/// - `DATABASE_URL` (uppercase snake_case)
/// - `DATABASE__URL` (double underscore)
/// - `database__url` (lowercase double underscore)
///
/// ## Example
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Config {
///     #[serde(alias = "DATABASE_URL", alias = "DATABASE__URL", alias = "database__url")]
///     database_url: String,
/// }
/// ```
#[macro_export]
macro_rules! multicase_field {
    ($field:ident: $ty:ty) => {
        #[serde(
            alias = stringify!($field),
            alias = stringify!([<$field:upper>]),
            alias = stringify!([<$field:upper _>]),
            alias = stringify!([<$field:lower __>])
        )]
        $field: $ty
    };
}

/// Common configuration fields used across services
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CommonConfig {
    /// Service port (defaults to 8080)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Log level (defaults to "info")
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Git SHA for versioning
    #[serde(alias = "GIT_SHA", alias = "GIT__SHA", alias = "git__sha", default)]
    pub git_sha: String,

    /// Environment name (dev, staging, production)
    #[serde(alias = "ENVIRONMENT", alias = "ENV", default = "default_environment")]
    pub environment: String,
}

fn default_port() -> u16 {
    8080
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_environment() -> String {
    "development".to_string()
}

impl CommonConfig {
    /// Check if running in production
    pub fn is_production(&self) -> bool {
        self.environment == "production" || self.environment == "prod"
    }

    /// Check if running in development
    pub fn is_development(&self) -> bool {
        self.environment == "development" || self.environment == "dev"
    }

    /// Check if running in staging
    pub fn is_staging(&self) -> bool {
        self.environment == "staging" || self.environment == "stage"
    }
}

impl ConfigLoader for CommonConfig {}

/// Service run mode determines how the service executes
///
/// Different modes have different responsibilities and behaviors:
/// - **Api**: Serve HTTP/GraphQL API requests
/// - **Migrate**: Run database migrations and exit
/// - **Worker**: Process background jobs/tasks
/// - **Promote**: Promote schema changes (for zero-downtime deployments)
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    /// API server mode - serve HTTP/GraphQL requests
    Api,
    /// Migration mode - run database migrations and exit
    Migrate,
    /// Worker mode - process background jobs
    Worker,
    /// Promote mode - promote schema changes for zero-downtime deployments
    Promote,
}

impl Default for RunMode {
    fn default() -> Self {
        Self::Api
    }
}

impl FromStr for RunMode {
    type Err = ConfigError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "api" => Ok(Self::Api),
            "migrate" => Ok(Self::Migrate),
            "worker" => Ok(Self::Worker),
            "promote" => Ok(Self::Promote),
            _ => Err(ConfigError::InvalidValue {
                variable: "RUN_MODE".to_string(),
                message: format!("Invalid run mode '{}'. Valid values: api, migrate, worker, promote", s),
            }),
        }
    }
}

impl std::fmt::Display for RunMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Api => write!(f, "api"),
            Self::Migrate => write!(f, "migrate"),
            Self::Worker => write!(f, "worker"),
            Self::Promote => write!(f, "promote"),
        }
    }
}

/// Server configuration with network binding helpers
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ServerConfig {
    /// Server host (defaults to 0.0.0.0)
    #[serde(default = "default_host")]
    pub host: String,

    /// Server port (defaults to 8080)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Run mode (api, migrate, worker, promote)
    #[serde(default)]
    pub run_mode: RunMode,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

impl ServerConfig {
    /// Get socket address for binding
    ///
    /// # Example
    /// ```rust
    /// let config = ServerConfig { host: "127.0.0.1".into(), port: 8080, run_mode: RunMode::Api };
    /// let addr = config.socket_addr().unwrap();
    /// assert_eq!(addr.port(), 8080);
    /// ```
    pub fn socket_addr(&self) -> Result<SocketAddr> {
        let addr_str = format!("{}:{}", self.host, self.port);
        addr_str.parse().map_err(|e| ConfigError::InvalidValue {
            variable: "HOST/PORT".to_string(),
            message: format!("Failed to parse socket address: {}", e),
        })
    }

    /// Check if running in API mode
    pub fn is_api_mode(&self) -> bool {
        self.run_mode == RunMode::Api
    }

    /// Check if running in migration mode
    pub fn is_migrate_mode(&self) -> bool {
        self.run_mode == RunMode::Migrate
    }

    /// Check if running in worker mode
    pub fn is_worker_mode(&self) -> bool {
        self.run_mode == RunMode::Worker
    }

    /// Check if running in promote mode
    pub fn is_promote_mode(&self) -> bool {
        self.run_mode == RunMode::Promote
    }
}

impl ConfigLoader for ServerConfig {}

/// Product configuration for multi-tenant services
///
/// Pleme uses product-level isolation where each product (Lilitu, NovaSkyn, etc.)
/// has its own isolated data and configuration.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProductConfig {
    /// Product identifier (e.g., "lilitu", "novaskyn")
    #[serde(alias = "PRODUCT_ID", alias = "product_id")]
    pub product_id: String,

    /// Product display name
    #[serde(alias = "PRODUCT_NAME", alias = "product_name", default)]
    pub product_name: Option<String>,

    /// Product tenant ID for database scoping
    #[serde(alias = "TENANT_ID", alias = "tenant_id", default)]
    pub tenant_id: Option<String>,
}

impl ProductConfig {
    /// Get product display name or fallback to ID
    pub fn display_name(&self) -> &str {
        self.product_name.as_deref().unwrap_or(&self.product_id)
    }

    /// Get tenant ID or use product ID as default
    pub fn tenant_id(&self) -> &str {
        self.tenant_id.as_deref().unwrap_or(&self.product_id)
    }

    /// Validate product configuration
    pub fn validate(&self) -> Result<()> {
        if self.product_id.is_empty() {
            return Err(ConfigError::Validation(
                "Product ID cannot be empty".to_string(),
            ));
        }

        // Product IDs should be lowercase and alphanumeric with hyphens
        if !self.product_id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return Err(ConfigError::Validation(
                "Product ID must be lowercase alphanumeric with hyphens".to_string(),
            ));
        }

        Ok(())
    }
}

impl ConfigLoader for ProductConfig {}

/// Database configuration
#[cfg(feature = "database")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DatabaseConfig {
    /// PostgreSQL connection URL
    #[serde(alias = "DATABASE_URL", alias = "DATABASE__URL", alias = "database__url")]
    pub database_url: String,

    /// Maximum number of connections in pool
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout_secs: u64,
}

#[cfg(feature = "database")]
fn default_max_connections() -> u32 {
    10
}

#[cfg(feature = "database")]
fn default_connection_timeout() -> u64 {
    30
}

#[cfg(feature = "database")]
impl DatabaseConfig {
    /// Parse database URL components
    pub fn parse_url(&self) -> Result<url::Url> {
        url::Url::parse(&self.database_url)
            .map_err(|e| ConfigError::InvalidValue {
                variable: "DATABASE_URL".to_string(),
                message: e.to_string(),
            })
    }

    /// Get database host
    pub fn host(&self) -> Result<String> {
        Ok(self.parse_url()?
            .host_str()
            .ok_or_else(|| ConfigError::InvalidValue {
                variable: "DATABASE_URL".to_string(),
                message: "Missing host".to_string(),
            })?
            .to_string())
    }

    /// Get database port
    pub fn port(&self) -> Result<u16> {
        Ok(self.parse_url()?
            .port()
            .unwrap_or(5432))  // PostgreSQL default port
    }

    /// Get database name
    pub fn database_name(&self) -> Result<String> {
        let url = self.parse_url()?;
        let path = url.path().trim_start_matches('/');
        if path.is_empty() {
            Err(ConfigError::InvalidValue {
                variable: "DATABASE_URL".to_string(),
                message: "Missing database name".to_string(),
            })
        } else {
            Ok(path.to_string())
        }
    }
}

#[cfg(feature = "database")]
impl ConfigLoader for DatabaseConfig {}

/// Redis configuration
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RedisConfig {
    /// Redis connection URL
    #[serde(alias = "REDIS_URL", alias = "REDIS__URL", alias = "redis__url")]
    pub redis_url: String,

    /// Connection pool size
    #[serde(default = "default_redis_pool_size")]
    pub pool_size: u32,
}

fn default_redis_pool_size() -> u32 {
    10
}

impl ConfigLoader for RedisConfig {}

// ============================================================================
// Optional: Validation support
// ============================================================================

#[cfg(feature = "validation")]
pub use validator::{Validate, ValidationError};

#[cfg(feature = "validation")]
pub trait ValidatedConfig: ConfigLoader + Validate {
    fn load_and_validate() -> Result<Self> {
        let config = Self::load()?;
        Validate::validate(&config)
            .map_err(|e| ConfigError::Validation(e.to_string()))?;
        Ok(config)
    }
}

// ============================================================================
// Optional: Multi-source configuration
// ============================================================================

#[cfg(feature = "multi-source")]
pub use config::Config as ConfigBuilder;

#[cfg(feature = "multi-source")]
pub trait MultiSourceLoader: DeserializeOwned + Sized {
    /// Load configuration from multiple sources (files, environment, etc.)
    ///
    /// Priority order (highest to lowest):
    /// 1. Environment variables (highest priority)
    /// 2. Environment-specific YAML file (config.{environment}.yaml)
    /// 3. Base YAML file (config.yaml)
    /// 4. Defaults defined in code (lowest priority)
    ///
    /// ## Example
    ///
    /// ```rust
    /// use pleme_config::MultiSourceLoader;
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct MyConfig {
    ///     database_url: String,
    ///     port: u16,
    /// }
    ///
    /// impl MultiSourceLoader for MyConfig {}
    ///
    /// // Loads from:
    /// // 1. config.yaml (if exists)
    /// // 2. config.production.yaml (if ENVIRONMENT=production)
    /// // 3. Environment variables (DATABASE_URL, PORT, etc.)
    /// let config = MyConfig::load_from_sources()?;
    /// ```
    fn load_from_sources() -> Result<Self> {
        Self::load_from_sources_with_path("config")
    }

    /// Load configuration with custom config file path
    ///
    /// Useful for testing or non-standard config locations.
    fn load_from_sources_with_path(config_path: &str) -> Result<Self> {
        let environment = std::env::var("ENVIRONMENT")
            .or_else(|_| std::env::var("ENV"))
            .unwrap_or_else(|_| "development".to_string());

        let mut builder = config::Config::builder();

        // 1. Load base config.yaml (optional)
        builder = builder.add_source(
            config::File::with_name(config_path)
                .format(config::FileFormat::Yaml)
                .required(false)
        );

        // 2. Load environment-specific config (optional)
        // e.g., config.production.yaml, config.staging.yaml
        let env_config_path = format!("{}.{}", config_path, environment);
        builder = builder.add_source(
            config::File::with_name(&env_config_path)
                .format(config::FileFormat::Yaml)
                .required(false)
        );

        // 3. Environment variables override everything
        // Supports nested keys with double underscore:
        // DATABASE__URL=postgres://... becomes database.url
        builder = builder.add_source(
            config::Environment::default()
                .separator("__")
                .try_parsing(true)
        );

        let config = builder
            .build()
            .map_err(|e| ConfigError::Parse(format!("Failed to build config: {}", e)))?;

        config
            .try_deserialize()
            .map_err(|e| ConfigError::Parse(format!("Failed to deserialize config: {}", e)))
    }

    /// Load and validate configuration from multiple sources
    fn load_from_sources_and_validate() -> Result<Self> {
        Self::load_from_sources()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use temp_env::with_vars;

    #[derive(Debug, serde::Deserialize, PartialEq)]
    struct TestConfig {
        #[serde(alias = "DATABASE_URL")]
        database_url: String,

        #[serde(default = "default_port")]
        port: u16,
    }

    impl ConfigLoader for TestConfig {}

    #[test]
    fn test_load_from_env() {
        with_vars(
            vec![
                ("DATABASE_URL", Some("postgres://localhost/test")),
                ("PORT", Some("3000")),
            ],
            || {
                let config = TestConfig::load().unwrap();
                assert_eq!(config.database_url, "postgres://localhost/test");
                assert_eq!(config.port, 3000);
            },
        );
    }

    #[test]
    fn test_default_values() {
        with_vars(
            vec![("DATABASE_URL", Some("postgres://localhost/test"))],
            || {
                let config = TestConfig::load().unwrap();
                assert_eq!(config.port, 8080);  // Default value
            },
        );
    }

    #[test]
    fn test_missing_required_field() {
        with_vars::<&str, &str, _, _>(vec![], || {
            let result = TestConfig::load();
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_common_config() {
        with_vars(
            vec![
                ("PORT", Some("9000")),
                ("LOG_LEVEL", Some("debug")),
                ("GIT_SHA", Some("abc123")),
                ("ENVIRONMENT", Some("production")),
            ],
            || {
                let config = CommonConfig::load().unwrap();
                assert_eq!(config.port, 9000);
                assert_eq!(config.log_level, "debug");
                assert_eq!(config.git_sha, "abc123");
                assert!(config.is_production());
                assert!(!config.is_development());
            },
        );
    }

    #[cfg(feature = "database")]
    #[test]
    fn test_database_config_parsing() {
        with_vars(
            vec![("DATABASE_URL", Some("postgres://localhost:5433/mydb"))],
            || {
                let config = DatabaseConfig::load().unwrap();
                assert_eq!(config.host().unwrap(), "localhost");
                assert_eq!(config.port().unwrap(), 5433);
                assert_eq!(config.database_name().unwrap(), "mydb");
            },
        );
    }

    #[test]
    fn test_run_mode_parsing() {
        assert_eq!(RunMode::from_str("api").unwrap(), RunMode::Api);
        assert_eq!(RunMode::from_str("API").unwrap(), RunMode::Api);
        assert_eq!(RunMode::from_str("migrate").unwrap(), RunMode::Migrate);
        assert_eq!(RunMode::from_str("worker").unwrap(), RunMode::Worker);
        assert_eq!(RunMode::from_str("promote").unwrap(), RunMode::Promote);

        assert!(RunMode::from_str("invalid").is_err());
    }

    #[test]
    fn test_run_mode_display() {
        assert_eq!(RunMode::Api.to_string(), "api");
        assert_eq!(RunMode::Migrate.to_string(), "migrate");
        assert_eq!(RunMode::Worker.to_string(), "worker");
        assert_eq!(RunMode::Promote.to_string(), "promote");
    }

    #[test]
    fn test_server_config() {
        with_vars(
            vec![
                ("HOST", Some("127.0.0.1")),
                ("PORT", Some("3000")),
                ("RUN_MODE", Some("worker")),
            ],
            || {
                let config = ServerConfig::load().unwrap();
                assert_eq!(config.host, "127.0.0.1");
                assert_eq!(config.port, 3000);
                assert_eq!(config.run_mode, RunMode::Worker);
                assert!(config.is_worker_mode());
                assert!(!config.is_api_mode());

                let addr = config.socket_addr().unwrap();
                assert_eq!(addr.port(), 3000);
            },
        );
    }

    #[test]
    fn test_server_config_defaults() {
        with_vars::<&str, &str, _, _>(vec![], || {
            let config = ServerConfig::load().unwrap();
            assert_eq!(config.host, "0.0.0.0");
            assert_eq!(config.port, 8080);
            assert_eq!(config.run_mode, RunMode::Api);
        });
    }

    #[test]
    fn test_product_config() {
        with_vars(
            vec![
                ("PRODUCT_ID", Some("lilitu")),
                ("PRODUCT_NAME", Some("Lilitu")),
                ("TENANT_ID", Some("tenant-123")),
            ],
            || {
                let config = ProductConfig::load().unwrap();
                assert_eq!(config.product_id, "lilitu");
                assert_eq!(config.display_name(), "Lilitu");
                assert_eq!(config.tenant_id(), "tenant-123");

                config.validate().unwrap();
            },
        );
    }

    #[test]
    fn test_product_config_defaults() {
        with_vars(
            vec![("PRODUCT_ID", Some("novaskyn"))],
            || {
                let config = ProductConfig::load().unwrap();
                assert_eq!(config.product_id, "novaskyn");
                assert_eq!(config.display_name(), "novaskyn");  // Falls back to ID
                assert_eq!(config.tenant_id(), "novaskyn");  // Falls back to ID
            },
        );
    }

    #[test]
    fn test_product_config_validation() {
        let valid_config = ProductConfig {
            product_id: "my-product".to_string(),
            product_name: None,
            tenant_id: None,
        };
        assert!(valid_config.validate().is_ok());

        let empty_id = ProductConfig {
            product_id: "".to_string(),
            product_name: None,
            tenant_id: None,
        };
        assert!(empty_id.validate().is_err());

        let invalid_chars = ProductConfig {
            product_id: "My_Product".to_string(),
            product_name: None,
            tenant_id: None,
        };
        assert!(invalid_chars.validate().is_err());
    }
}
