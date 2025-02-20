use std::sync::Arc;

use crate::dht::Did;
use crate::dht::PeerRing;
use crate::ecc::SecretKey;
use crate::error::Result;
use crate::session::SessionSk;
use crate::storage::PersistenceStorage;
use crate::swarm::Swarm;
use crate::swarm::SwarmBuilder;

mod test_message_handler;
mod test_stabilization;

pub async fn prepare_node(key: SecretKey) -> (Arc<Swarm>, String) {
    let stun = "stun://stun.l.google.com:19302";
    let path = PersistenceStorage::random_path("./tmp");
    let storage = PersistenceStorage::new_with_path(path.as_str())
        .await
        .unwrap();

    let session_sk = SessionSk::new_with_seckey(&key).unwrap();
    let swarm = Arc::new(SwarmBuilder::new(stun, storage, session_sk).build());

    println!("key: {:?}", key.to_string());
    println!("did: {:?}", swarm.did());

    (swarm, path)
}

pub async fn gen_pure_dht(did: Did) -> Result<PeerRing> {
    let db_path = PersistenceStorage::random_path("./tmp");
    let db = PersistenceStorage::new_with_path(db_path.as_str()).await?;
    Ok(PeerRing::new_with_storage(did, 3, db))
}

pub async fn gen_sorted_dht(s: usize) -> Vec<PeerRing> {
    let mut keys: Vec<crate::ecc::SecretKey> = vec![];
    for _i in 0..s {
        keys.push(crate::ecc::SecretKey::random());
    }
    keys.sort_by_key(|a| a.address());

    #[allow(clippy::needless_collect)]
    let dids: Vec<crate::dht::Did> = keys
        .iter()
        .map(|sk| crate::dht::Did::from(sk.address()))
        .collect();

    let mut iter = dids.into_iter();
    let mut ret: Vec<crate::dht::PeerRing> = vec![];
    for _ in 0..s {
        ret.push(
            crate::tests::default::gen_pure_dht(iter.next().unwrap())
                .await
                .unwrap(),
        )
    }
    ret
}
