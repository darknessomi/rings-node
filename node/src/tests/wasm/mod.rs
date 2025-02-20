pub mod browser;
pub mod processor;

use wasm_bindgen_test::wasm_bindgen_test_configure;

use crate::logging::browser::init_logging;
use crate::prelude::rings_core::ecc::SecretKey;
use crate::prelude::rings_core::prelude::uuid;
use crate::prelude::rings_core::storage::PersistenceStorage;
use crate::prelude::SessionSk;
use crate::processor::Processor;
use crate::processor::ProcessorBuilder;
use crate::processor::ProcessorConfig;

wasm_bindgen_test_configure!(run_in_browser);

pub fn setup_log() {
    init_logging(crate::logging::LogLevel::Info);
    tracing::debug!("test")
}

pub async fn prepare_processor() -> Processor {
    let key = SecretKey::random();
    let sm = SessionSk::new_with_seckey(&key).unwrap();

    let config = serde_yaml::to_string(&ProcessorConfig::new(
        "stun://stun.l.google.com:19302".to_string(),
        sm,
        200,
    ))
    .unwrap();

    let storage_path = uuid::Uuid::new_v4().to_simple().to_string();
    let storage = PersistenceStorage::new_with_cap_and_path(50000, storage_path.as_str())
        .await
        .unwrap();

    ProcessorBuilder::from_serialized(&config)
        .unwrap()
        .storage(storage)
        .build()
        .unwrap()
}
