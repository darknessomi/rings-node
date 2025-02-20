//! The main entity of this module is the [ConnectionInterface] trait, which provides an
//! interface for establishing connections with other nodes, send data channel message to it.
//!
//! There is also a [TransportInterface] trait, which is used to specify the management of all
//! [ConnectionInterface] objects.

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;

use crate::connection_ref::ConnectionRef;
use crate::core::callback::BoxedTransportCallback;

/// Wrapper for the data that is sent over the data channel.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum TransportMessage {
    /// The custom message is sent by an external invoker and
    /// should be handled by the on_message callback.
    Custom(Vec<u8>),
}

/// The state of the WebRTC connection.
/// This enum is used to define a same interface for all the platforms.
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum WebrtcConnectionState {
    /// Unspecified
    #[default]
    Unspecified,

    /// WebrtcConnectionState::New indicates that any of the ICETransports or
    /// DTLSTransports are in the "new" state and none of the transports are
    /// in the "connecting", "checking", "failed" or "disconnected" state, or
    /// all transports are in the "closed" state, or there are no transports.
    New,

    /// WebrtcConnectionState::Connecting indicates that any of the
    /// ICETransports or DTLSTransports are in the "connecting" or
    /// "checking" state and none of them is in the "failed" state.
    Connecting,

    /// WebrtcConnectionState::Connected indicates that all ICETransports and
    /// DTLSTransports are in the "connected", "completed" or "closed" state
    /// and at least one of them is in the "connected" or "completed" state.
    Connected,

    /// WebrtcConnectionState::Disconnected indicates that any of the
    /// ICETransports or DTLSTransports are in the "disconnected" state
    /// and none of them are in the "failed" or "connecting" or "checking" state.
    Disconnected,

    /// WebrtcConnectionState::Failed indicates that any of the ICETransports
    /// or DTLSTransports are in a "failed" state.
    Failed,

    /// WebrtcConnectionState::Closed indicates the peer connection is closed
    /// and the isClosed member variable of PeerConnection is true.
    Closed,
}

/// The [ConnectionInterface] trait defines how to
/// make webrtc ice handshake with a remote peer and then send data channel message to it.
#[cfg_attr(feature = "web-sys-webrtc", async_trait(?Send))]
#[cfg_attr(not(feature = "web-sys-webrtc"), async_trait)]
pub trait ConnectionInterface {
    /// Sdp is used to expose local and remote session descriptions when handshaking.
    type Sdp: Serialize + DeserializeOwned;
    /// The error type that is returned by connection.
    type Error: std::error::Error;

    /// Send a [TransportMessage] to the remote peer.
    async fn send_message(&self, msg: TransportMessage) -> Result<(), Self::Error>;

    /// Get current webrtc connection state.
    fn webrtc_connection_state(&self) -> WebrtcConnectionState;

    /// This is a debug method to dump the stats of webrtc connection.
    async fn get_stats(&self) -> Vec<String>;

    /// Create a webrtc offer to start handshake.
    async fn webrtc_create_offer(&self) -> Result<Self::Sdp, Self::Error>;

    /// Accept a webrtc offer from remote peer and give back an answer.
    async fn webrtc_answer_offer(&self, offer: Self::Sdp) -> Result<Self::Sdp, Self::Error>;

    /// Accept a webrtc answer from remote peer.
    async fn webrtc_accept_answer(&self, answer: Self::Sdp) -> Result<(), Self::Error>;

    /// Wait for the data channel to be opened after handshake.
    async fn webrtc_wait_for_data_channel_open(&self) -> Result<(), Self::Error>;

    /// Close the webrtc connection.
    async fn close(&self) -> Result<(), Self::Error>;

    /// Deprecated, should use `webrtc_connection_state`.
    fn ice_connection_state(&self) -> WebrtcConnectionState {
        self.webrtc_connection_state()
    }

    /// Deprecated, should check the state of `webrtc_connection_state`.
    async fn is_connected(&self) -> bool {
        self.webrtc_connection_state() == WebrtcConnectionState::Connected
    }

    /// Deprecated, should check the state of `webrtc_connection_state`.
    async fn is_disconnected(&self) -> bool {
        matches!(
            self.webrtc_connection_state(),
            WebrtcConnectionState::Disconnected
                | WebrtcConnectionState::Failed
                | WebrtcConnectionState::Closed
        )
    }
}

/// This trait specifies how to management [ConnectionInterface] objects.
/// Each platform must implement this trait for its own connection implementation.
/// See [connections](crate::connections) module for examples.
#[cfg_attr(feature = "web-sys-webrtc", async_trait(?Send))]
#[cfg_attr(not(feature = "web-sys-webrtc"), async_trait)]
pub trait TransportInterface {
    /// The connection type that is created by this trait.
    type Connection: ConnectionInterface<Error = Self::Error>;

    /// The error type that is returned by transport.
    type Error: std::error::Error;

    /// Used to create a new connection and register it in the transport.
    ///
    /// To avoid memory leak, this function will not return a connection object.
    /// Instead, user should use `connection` method of to get a [ConnectionRef](crate::connection_ref::ConnectionRef)
    /// after creation.
    ///
    /// See [connections](crate::connections) module for examples.
    async fn new_connection(
        &self,
        cid: &str,
        callback: BoxedTransportCallback,
    ) -> Result<(), Self::Error>;

    /// This method closes and releases the connection from transport.
    /// All references to this cid, created by `get_connection`, will be released.
    async fn close_connection(&self, cid: &str) -> Result<(), Self::Error>;

    /// Get a reference of the connection by its id.
    fn connection(&self, cid: &str) -> Result<ConnectionRef<Self::Connection>, Self::Error>;

    /// Get all the connections in the transport.
    fn connections(&self) -> Vec<(String, ConnectionRef<Self::Connection>)>;

    /// Get all the connection ids in the transport.
    fn connection_ids(&self) -> Vec<String>;
}

/// Used to store a boxed [TransportInterface] trait object.
#[cfg(not(feature = "web-sys-webrtc"))]
pub type BoxedTransport<C, E> =
    Box<dyn TransportInterface<Connection = C, Error = E> + Send + Sync>;

/// Used to store a boxed [TransportInterface] trait object.
#[cfg(feature = "web-sys-webrtc")]
pub type BoxedTransport<C, E> = Box<dyn TransportInterface<Connection = C, Error = E>>;
