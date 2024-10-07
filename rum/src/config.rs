use aes::Aes128;
use aes_gcm_siv::{AesGcmSiv, Key};
use once_cell::sync::OnceCell;
use std::io::IsTerminal;
use time::Duration;

use crate::controller::{AllowAll, AuthHandler, MiddlewareSet};
use rand::{rngs::OsRng, RngCore};

static CONFIG: OnceCell<Config> = OnceCell::new();

/// Global configuration.
pub struct Config {
    pub aes_key: Key<AesGcmSiv<Aes128>>, // AES-128 key used for encryption.
    pub secure_id_key: Key<AesGcmSiv<Aes128>>,
    pub cookie_max_age: Duration,
    pub tty: bool,
    pub default_auth: AuthHandler,
    pub session_duration: Duration,
    pub default_middleware: MiddlewareSet,
    pub cache_templates: bool,
    pub websocket: Websocket,
    pub log_queries: bool,
}

pub struct Websocket {
    pub ping_interval: Duration,
    pub ping_timeout: Duration,
    pub ping_disconnect_count: i64,
}

impl Default for Websocket {
    fn default() -> Self {
        Self {
            ping_timeout: Duration::seconds(5),
            ping_interval: Duration::seconds(60),
            ping_disconnect_count: 3,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        // Generate a random AES key.
        let mut secret_key = [0u8; 256 / 8];
        OsRng.fill_bytes(&mut secret_key);

        let aes_key = Key::<AesGcmSiv<Aes128>>::clone_from_slice(&secret_key[0..128 / 8]);
        let secure_id_key = Key::<AesGcmSiv<Aes128>>::clone_from_slice(&secret_key[128 / 8..]);

        Self {
            aes_key,
            secure_id_key,
            cookie_max_age: Duration::days(30),
            tty: std::io::stderr().is_terminal(),
            default_auth: AuthHandler::new(AllowAll {}),
            session_duration: Duration::days(4),
            default_middleware: MiddlewareSet::default(),
            cache_templates: false,
            websocket: Websocket::default(),
            log_queries: std::env::var("RUM_LOG_QUERIES").is_ok(),
        }
    }
}

pub fn get_config() -> &'static Config {
    CONFIG.get_or_init(|| Config::default())
}
