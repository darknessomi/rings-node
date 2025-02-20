use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::backend::native::extension::ExtensionConfig;
use crate::backend::native::service::ServiceConfig;
use crate::backend::native::BackendConfig;
use crate::error::Error;
use crate::error::Result;
use crate::prelude::rings_core::ecc::SecretKey;
use crate::prelude::SessionSk;
use crate::processor::ProcessorConfig;
use crate::processor::ProcessorConfigSerialized;
use crate::util::ensure_parent_dir;
use crate::util::expand_home;

lazy_static::lazy_static! {
  static ref DEFAULT_DATA_STORAGE_CONFIG: StorageConfig = StorageConfig {
    path: get_storage_location(".rings", "data"),
    capacity: DEFAULT_STORAGE_CAPACITY,
  };
  static ref DEFAULT_MEASURE_STORAGE_CONFIG: StorageConfig = StorageConfig {
    path: get_storage_location(".rings", "measure"),
    capacity: DEFAULT_STORAGE_CAPACITY,
  };
}

pub const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:50000";
pub const DEFAULT_ENDPOINT_URL: &str = "http://127.0.0.1:50000";
pub const DEFAULT_ICE_SERVERS: &str = "stun://stun.l.google.com:19302";
pub const DEFAULT_STABILIZE_TIMEOUT: usize = 3;
pub const DEFAULT_STORAGE_CAPACITY: usize = 200000000;

pub fn get_storage_location<P>(prefix: P, path: P) -> String
where P: AsRef<std::path::Path> {
    let home_dir = env::var_os("HOME").map(PathBuf::from);
    let expect = match home_dir {
        Some(dir) => dir.join(prefix).join(path),
        None => std::path::Path::new("data").join(prefix).join(path),
    };
    expect.to_str().unwrap().to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ecdsa_key: Option<SecretKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_manager: Option<String>,
    pub session_sk: Option<String>,
    #[serde(rename = "bind")]
    pub http_addr: String,
    pub endpoint_url: String,
    pub ice_servers: String,
    pub stabilize_timeout: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_ip: Option<String>,
    /// When there is no configuration in the YAML file,
    /// its deserialization is equivalent to `vec![]` in Rust.
    #[serde(default)]
    pub services: Vec<ServiceConfig>,
    pub data_storage: StorageConfig,
    pub measure_storage: StorageConfig,
    /// When there is no configuration in the YAML file,
    /// its deserialization is equivalent to `ExtensionConfig(vec![])` in Rust.
    #[serde(default)]
    pub extension: ExtensionConfig,
}

impl TryFrom<Config> for ProcessorConfigSerialized {
    type Error = Error;
    fn try_from(config: Config) -> Result<Self> {
        // Support old version
        let session_sk: String = if let Some(sk) = config.ecdsa_key {
            tracing::warn!("Field `ecdsa_key` is deprecated, use `session_sk` instead.");
            SessionSk::new_with_seckey(&sk)
                .expect("create session sk failed")
                .dump()
                .expect("dump session sk failed")
        } else if let Some(ssk) = config.session_manager {
            tracing::warn!("Field `session_manager` is deprecated, use `session_sk` instead.");
            ssk
        } else {
            let ssk_file = config.session_sk.expect("session_sk is not set.");
            let ssk_file_expand_home = expand_home(&ssk_file)?;
            fs::read_to_string(ssk_file_expand_home).unwrap_or_else(|e| {
                tracing::warn!("Read session_sk file failed: {e:?}. Handling it as raw session_sk string. This mode is deprecated. please use a file path.");
                ssk_file
            })
        };

        if let Some(ext_ip) = config.external_ip {
            Ok(Self::new_with_ext_addr(
                config.ice_servers,
                session_sk,
                config.stabilize_timeout,
                ext_ip,
            ))
        } else {
            Ok(Self::new(
                config.ice_servers,
                session_sk,
                config.stabilize_timeout,
            ))
        }
    }
}

impl TryFrom<Config> for ProcessorConfig {
    type Error = Error;
    fn try_from(config: Config) -> Result<Self> {
        ProcessorConfigSerialized::try_from(config).and_then(Self::try_from)
    }
}

impl From<Config> for BackendConfig {
    fn from(config: Config) -> Self {
        Self {
            services: config.services,
            extensions: config.extension,
        }
    }
}

impl Config {
    pub fn new<P>(session_sk: P) -> Self
    where P: AsRef<std::path::Path> {
        let session_sk = session_sk.as_ref().to_string_lossy().to_string();
        Self {
            ecdsa_key: None,
            session_manager: None,
            session_sk: Some(session_sk),
            http_addr: DEFAULT_BIND_ADDRESS.to_string(),
            endpoint_url: DEFAULT_ENDPOINT_URL.to_string(),
            ice_servers: DEFAULT_ICE_SERVERS.to_string(),
            stabilize_timeout: DEFAULT_STABILIZE_TIMEOUT,
            external_ip: None,
            services: vec![],
            data_storage: DEFAULT_DATA_STORAGE_CONFIG.clone(),
            measure_storage: DEFAULT_MEASURE_STORAGE_CONFIG.clone(),
            extension: ExtensionConfig::default(),
        }
    }

    pub fn write_fs<P>(&self, path: P) -> Result<String>
    where P: AsRef<std::path::Path> {
        let path = expand_home(path)?;
        ensure_parent_dir(&path)?;
        let f =
            fs::File::create(path.as_path()).map_err(|e| Error::CreateFileError(e.to_string()))?;
        let f_writer = io::BufWriter::new(f);
        serde_yaml::to_writer(f_writer, self).map_err(|_| Error::EncodeError)?;
        Ok(path.to_str().unwrap().to_owned())
    }

    pub fn read_fs<P>(path: P) -> Result<Config>
    where P: AsRef<std::path::Path> {
        let path = expand_home(path)?;
        tracing::debug!("Read config from: {:?}", path);
        let f = fs::File::open(path).map_err(|e| Error::OpenFileError(e.to_string()))?;
        let f_rdr = io::BufReader::new(f);
        serde_yaml::from_reader(f_rdr).map_err(|_| Error::EncodeError)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub path: String,
    pub capacity: usize,
}

impl StorageConfig {
    pub fn new(path: &str, capacity: usize) -> Self {
        Self {
            path: path.to_string(),
            capacity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialization_with_missed_field() {
        let yaml = r#"
session_sk: session_sk
bind: 127.0.0.1:50000
endpoint_url: http://127.0.0.1:50000
ice_servers: stun://stun.l.google.com:19302
stabilize_timeout: 3
external_ip: null
data_storage:
  path: /Users/foo/.rings/data
  capacity: 200000000
measure_storage:
  path: /Users/foo/.rings/measure
  capacity: 200000000
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.extension, ExtensionConfig::default());
        assert_eq!(cfg.services, vec![]);
    }
}
