//! rings-rpc client

use rings_core::session::SessionSk;
use serde_json::json;
use serde_json::Value;

use crate::error::Error;
use crate::error::Result;
use crate::jsonrpc_client::SimpleClient;
use crate::method::Method;
use crate::prelude::jsonrpc_core::Params;
use crate::prelude::*;
use crate::response;
use crate::response::Peer;

/// Wrap json_client send request between nodes or browsers.
pub struct Client {
    client: SimpleClient,
}

impl Client {
    /// Creates a new Client instance with the specified endpoint URL
    pub fn new(endpoint_url: &str, delegated_sk: Option<SessionSk>) -> Self {
        Self {
            client: SimpleClient::new(endpoint_url, delegated_sk),
        }
    }

    /// Establishes a WebRTC connection with a remote peer using HTTP as the signaling channel.
    ///
    /// This function allows two peers to establish a WebRTC connection using HTTP,
    /// which can be useful in scenarios where a direct peer-to-peer connection is not possible due to firewall restrictions or other network issues.
    /// The function sends ICE candidates and Session Description Protocol (SDP) messages over HTTP as a form of signaling to establish the connection.
    ///
    /// Takes a URL for an HTTP server that will be used as the signaling channel to exchange ICE candidates and SDP with the remote peer.
    /// Returns a Did that can be used to refer to this connection in subsequent WebRTC operations.
    pub async fn connect_peer_via_http(&mut self, http_url: &str) -> Result<String> {
        let resp = self
            .client
            .call_method(
                Method::ConnectPeerViaHttp.as_str(),
                Params::Array(vec![Value::String(http_url.to_owned())]),
            )
            .await
            .map_err(Error::RpcError)?;

        let did = resp.as_str().ok_or(Error::DecodeError)?;
        Ok(did.to_string())
    }

    /// Attempts to connect to a peer using a seed file located at the specified source path.
    pub async fn connect_with_seed(&mut self, seeds: &[serde_json::Value]) -> Result<()> {
        self.client
            .call_method(
                Method::ConnectWithSeed.as_str(),
                Params::Array(seeds.to_vec()),
            )
            .await
            .map_err(Error::RpcError)?;
        Ok(())
    }

    /// Attempts to connect to a peer using a DID stored in a Distributed Hash Table (DHT).
    pub async fn connect_with_did(&mut self, did: &str) -> Result<()> {
        self.client
            .call_method(
                Method::ConnectWithDid.as_str(),
                Params::Array(vec![Value::String(did.to_owned())]),
            )
            .await
            .map_err(Error::RpcError)?;
        Ok(())
    }

    /// Lists all connected peers and their status.
    ///
    /// Returns an Output containing a formatted string representation of the list of peers if successful, or an anyhow::Error if an error occurred.
    pub async fn list_peers(&mut self) -> Result<Vec<Peer>> {
        let resp = self
            .client
            .call_method(Method::ListPeers.as_str(), Params::Array(vec![]))
            .await
            .map_err(Error::RpcError)?;

        let peers: Vec<Peer> = serde_json::from_value(resp).map_err(|_| Error::DecodeError)?;
        Ok(peers)
    }

    /// Disconnects from the peer with the specified DID.
    pub async fn disconnect(&mut self, did: &str) -> Result<()> {
        self.client
            .call_method(Method::Disconnect.as_str(), Params::Array(vec![json!(did)]))
            .await
            .map_err(Error::RpcError)?;

        Ok(())
    }

    /// Sends a message to the specified peer.
    pub async fn send_message(
        &self,
        did: &str,
        text: &str,
    ) -> Result<response::SendMessageResponse> {
        let mut params = serde_json::Map::new();
        params.insert("destination".to_owned(), json!(did));
        params.insert("text".to_owned(), json!(text));
        let result = self
            .client
            .call_method(Method::SendTo.as_str(), Params::Map(params))
            .await
            .map_err(Error::RpcError)?;
        serde_json::from_value(result).map_err(|_| Error::DecodeError)
    }

    /// Sends a custom message to the specified peer.
    pub async fn send_custom_message(
        &self,
        did: &str,
        data_b64: &str,
    ) -> Result<response::SendMessageResponse> {
        let result = self
            .client
            .call_method(
                Method::SendCustomMessage.as_str(),
                Params::Array(vec![json!(did), json!(data_b64)]),
            )
            .await
            .map_err(Error::RpcError)?;
        serde_json::from_value(result).map_err(|_| Error::DecodeError)
    }

    /// Registers a new service with the given name.
    pub async fn register_service(&self, name: &str) -> Result<()> {
        self.client
            .call_method(
                Method::RegisterService.as_str(),
                Params::Array(vec![json!(name)]),
            )
            .await
            .map_err(Error::RpcError)?;
        Ok(())
    }

    /// Looks up the DIDs of services registered with the given name.
    pub async fn lookup_service(&self, name: &str) -> Result<Vec<String>> {
        let resp = self
            .client
            .call_method(
                Method::LookupService.as_str(),
                Params::Array(vec![json!(name)]),
            )
            .await
            .map_err(Error::RpcError)?;

        serde_json::from_value(resp).map_err(|_| Error::DecodeError)
    }

    /// Publishes a message to the specified topic.
    pub async fn publish_message_to_topic(&self, topic: &str, data: &str) -> Result<()> {
        self.client
            .call_method(
                Method::PublishMessageToTopic.as_str(),
                Params::Array(vec![json!(topic), json!(data)]),
            )
            .await
            .map_err(Error::RpcError)?;
        Ok(())
    }

    pub async fn fetch_topic_messages(&self, topic: &str, index: usize) -> Result<Vec<String>> {
        let resp = self
            .client
            .call_method(
                Method::FetchMessagesOfTopic.as_str(),
                Params::Array(vec![json!(topic), json!(index)]),
            )
            .await
            .map_err(Error::RpcError)?;

        serde_json::from_value(resp).map_err(|_| Error::DecodeError)
    }

    /// Query for swarm inspect info.
    pub async fn inspect(&self) -> Result<response::NodeInfo> {
        let resp = self
            .client
            .call_method(Method::NodeInfo.as_str(), Params::None)
            .await
            .map_err(Error::RpcError)?;
        serde_json::from_value(resp).map_err(|_| Error::DecodeError)
    }
}
