//! A jsonrpc-server of rings-node.
/// [JSON-RPC]: `<https://www.jsonrpc.org/specification>`
// pub mod method;
//pub mod response;
pub mod server;
/// RpcMeta basic info struct
pub use server::RpcMeta;

/// MetaIoHandler add methods from `super::methods::*` with RpcMeta
pub mod handler;
#[cfg(feature = "node")]
pub use self::handler::build_handler;
pub use self::handler::HandlerType;
