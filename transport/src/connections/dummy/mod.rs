use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashMap;
use lazy_static::lazy_static;
use rand::distributions::Distribution;

use crate::callback::InnerTransportCallback;
use crate::connection_ref::ConnectionRef;
use crate::core::callback::BoxedTransportCallback;
use crate::core::transport::ConnectionInterface;
use crate::core::transport::TransportInterface;
use crate::core::transport::TransportMessage;
use crate::core::transport::WebrtcConnectionState;
use crate::error::Error;
use crate::error::Result;
use crate::ice_server::IceServer;
use crate::notifier::Notifier;
use crate::pool::Pool;

/// Max delay in ms on sending message
const DUMMY_DELAY_MAX: u64 = 100;
/// Min delay in ms on sending message
const DUMMY_DELAY_MIN: u64 = 0;
/// Config random delay when send message
const SEND_MESSAGE_DELAY: bool = true;
/// Config random delay when channel opening
const CHANNEL_OPEN_DELAY: bool = false;

lazy_static! {
    static ref CBS: DashMap<String, Arc<InnerTransportCallback>> = DashMap::new();
    static ref CONNS: DashMap<String, Arc<DummyConnection>> = DashMap::new();
}

/// A dummy connection for local testing.
/// Implements the [ConnectionInterface] trait with no real network.
pub struct DummyConnection {
    pub(crate) rand_id: String,
    remote_rand_id: Arc<Mutex<Option<String>>>,
    webrtc_connection_state: Arc<Mutex<WebrtcConnectionState>>,
}

/// [DummyTransport] manages all the [DummyConnection] and
/// provides methods to create, get and close connections.
pub struct DummyTransport {
    pool: Pool<DummyConnection>,
}

impl DummyConnection {
    fn new() -> Self {
        Self {
            rand_id: random(0, 10000000000).to_string(),
            remote_rand_id: Arc::new(Mutex::new(None)),
            webrtc_connection_state: Arc::new(Mutex::new(WebrtcConnectionState::New)),
        }
    }

    fn callback(&self) -> Arc<InnerTransportCallback> {
        CBS.get(&self.rand_id).unwrap().clone()
    }

    fn remote_callback(&self) -> Arc<InnerTransportCallback> {
        let cid: String = { self.remote_rand_id.lock().unwrap() }.clone().unwrap();
        CBS.get(&cid)
            .expect(&format!("Failed to get cid {:?}", &cid))
            .clone()
    }

    fn remote_conn(&self) -> Arc<DummyConnection> {
        let cid = { self.remote_rand_id.lock().unwrap() }.clone().unwrap();
        CONNS.get(&cid).unwrap().clone()
    }

    fn set_remote_rand_id(&self, rand_id: String) {
        let mut remote_rand_id = self.remote_rand_id.lock().unwrap();
        *remote_rand_id = Some(rand_id);
    }

    async fn set_webrtc_connection_state(&self, state: WebrtcConnectionState) {
        {
            let mut webrtc_connection_state = self.webrtc_connection_state.lock().unwrap();

            if state == *webrtc_connection_state {
                return;
            }

            *webrtc_connection_state = state;
        }

        self.callback().on_peer_connection_state_change(state).await;
    }
}

impl DummyTransport {
    /// Create a new [DummyTransport] instance.
    pub fn new(ice_servers: &str, _external_address: Option<String>) -> Self {
        let _ice_servers = IceServer::vec_from_str(ice_servers).unwrap();

        Self { pool: Pool::new() }
    }
}

#[async_trait]
impl ConnectionInterface for DummyConnection {
    type Sdp = String;
    type Error = Error;

    async fn send_message(&self, msg: TransportMessage) -> Result<()> {
        if SEND_MESSAGE_DELAY {
            random_delay().await;
        }
        self.webrtc_wait_for_data_channel_open().await?;
        let data = bincode::serialize(&msg).map(Bytes::from)?;
        self.remote_callback().on_message(&data).await;
        Ok(())
    }

    fn webrtc_connection_state(&self) -> WebrtcConnectionState {
        *self.webrtc_connection_state.lock().unwrap()
    }

    async fn get_stats(&self) -> Vec<String> {
        Vec::new()
    }

    async fn webrtc_create_offer(&self) -> Result<Self::Sdp> {
        self.set_webrtc_connection_state(WebrtcConnectionState::Connecting)
            .await;
        Ok(self.rand_id.clone())
    }

    async fn webrtc_answer_offer(&self, offer: Self::Sdp) -> Result<Self::Sdp> {
        self.set_webrtc_connection_state(WebrtcConnectionState::Connecting)
            .await;
        self.set_remote_rand_id(offer);
        Ok(self.rand_id.clone())
    }

    async fn webrtc_accept_answer(&self, answer: Self::Sdp) -> Result<()> {
        self.set_webrtc_connection_state(WebrtcConnectionState::Connected)
            .await;
        self.set_remote_rand_id(answer);
        self.remote_conn()
            .set_webrtc_connection_state(WebrtcConnectionState::Connected)
            .await;
        Ok(())
    }

    async fn webrtc_wait_for_data_channel_open(&self) -> Result<()> {
        if CHANNEL_OPEN_DELAY {
            random_delay().await;
        }
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        self.set_webrtc_connection_state(WebrtcConnectionState::Closed)
            .await;

        // simulate remote closing if it's not closed
        if self.remote_conn().webrtc_connection_state() != WebrtcConnectionState::Closed {
            self.remote_conn()
                .set_webrtc_connection_state(WebrtcConnectionState::Disconnected)
                .await;
            self.remote_conn()
                .set_webrtc_connection_state(WebrtcConnectionState::Closed)
                .await;
        }

        Ok(())
    }
}

#[async_trait]
impl TransportInterface for DummyTransport {
    type Connection = DummyConnection;
    type Error = Error;

    async fn new_connection(&self, cid: &str, callback: BoxedTransportCallback) -> Result<()> {
        if let Ok(existed_conn) = self.pool.connection(cid) {
            if matches!(
                existed_conn.webrtc_connection_state(),
                WebrtcConnectionState::New
                    | WebrtcConnectionState::Connecting
                    | WebrtcConnectionState::Connected
            ) {
                return Err(Error::ConnectionAlreadyExists(cid.to_string()));
            }
        }

        let conn = DummyConnection::new();
        let conn_rand_id = conn.rand_id.clone();

        self.pool.safely_insert(cid, conn)?;
        CONNS.insert(conn_rand_id.clone(), self.connection(cid)?.upgrade()?);
        CBS.insert(
            conn_rand_id.clone(),
            Arc::new(InnerTransportCallback::new(
                cid,
                callback,
                Notifier::default(),
            )),
        );
        Ok(())
    }

    async fn close_connection(&self, cid: &str) -> Result<()> {
        self.pool.safely_remove(cid).await
    }

    fn connection(&self, cid: &str) -> Result<ConnectionRef<Self::Connection>> {
        self.pool.connection(cid)
    }

    fn connections(&self) -> Vec<(String, ConnectionRef<Self::Connection>)> {
        self.pool.connections()
    }

    fn connection_ids(&self) -> Vec<String> {
        self.pool.connection_ids()
    }
}

async fn random_delay() {
    tokio::time::sleep(Duration::from_millis(random(
        DUMMY_DELAY_MIN,
        DUMMY_DELAY_MAX,
    )))
    .await;
}

fn random(low: u64, high: u64) -> u64 {
    let range = rand::distributions::Uniform::new(low, high);
    let mut rng = rand::thread_rng();
    range.sample(&mut rng)
}
