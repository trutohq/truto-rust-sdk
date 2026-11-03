//! MCP (Model Context Protocol) servers exposed by Truto.

use reqwest::Method;
use serde::Serialize;
use serde_json::json;

use super::seg;
use crate::{Client, Record, Request};

/// JSON-RPC calls to a Truto MCP server (`POST /mcp/{token}`).
#[derive(Debug, Clone, Copy)]
pub struct Mcp<'a> {
    client: &'a Client,
}

impl<'a> Mcp<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Send any JSON-RPC payload, unchanged.
    pub fn call(&self, token: &str, payload: impl Serialize) -> Request<Record> {
        self.client
            .request(Method::POST, format!("mcp/{}", seg(token)))
            .body(payload)
    }

    /// Send the `initialize` handshake.
    pub fn initialize(
        &self,
        token: &str,
        client_name: &str,
        client_version: &str,
    ) -> Request<Record> {
        self.call(
            token,
            json!({
                "jsonrpc": "2.0",
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "clientInfo": { "name": client_name, "version": client_version },
                },
            }),
        )
    }

    /// List the server's tools (`tools/list`).
    pub fn list_tools(&self, token: &str) -> Request<Record> {
        self.call(
            token,
            json!({ "jsonrpc": "2.0", "method": "tools/list", "id": 1 }),
        )
    }

    /// Call a tool (`tools/call`).
    pub fn call_tool(&self, token: &str, name: &str, arguments: impl Serialize) -> Request<Record> {
        let arguments = match serde_json::to_value(arguments) {
            Ok(v) => v,
            Err(e) => {
                return self.call(token, ()).body(FailingSerialize(e.to_string()));
            }
        };
        self.call(
            token,
            json!({
                "jsonrpc": "2.0",
                "method": "tools/call",
                "id": 2,
                "params": { "name": name, "arguments": arguments },
            }),
        )
    }
}

/// Re-raises an argument serialisation error through `Request::body`.
struct FailingSerialize(String);

impl Serialize for FailingSerialize {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom(&self.0))
    }
}
