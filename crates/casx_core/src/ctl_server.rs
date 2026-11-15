//! Local loopback IPC control server for casx-ctl (JSON-RPC 2.0 / MCP Server).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// Engine telemetry state reported to casx-ctl
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineTelemetryState {
    pub in_game: bool,
    pub version: String,
    pub player_pos: [f64; 3],
    pub fps: f64,
    pub ping_ms: u32,
    pub loaded_chunks: usize,
}

impl Default for EngineTelemetryState {
    fn default() -> Self {
        Self {
            in_game: false,
            version: "1.12.2".to_string(),
            player_pos: [0.0, 0.0, 0.0],
            fps: 0.0,
            ping_ms: 0,
            loaded_chunks: 0,
        }
    }
}

/// Yeu cau JSON-RPC 2.0 theo dac ta Model Context Protocol (MCP)
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
    pub id: Option<Value>,
}

/// Phan giai va dieu phoi yeu cau JSON-RPC 2.0
pub fn dispatch_json_rpc(request_str: &str, state: &mut EngineTelemetryState) -> String {
    let req: JsonRpcRequest = match serde_json::from_str(request_str) {
        Ok(r) => r,
        Err(e) => {
            return json!({
                "jsonrpc": "2.0",
                "error": { "code": -32700, "message": format!("Parse error: {}", e) },
                "id": null
            })
            .to_string();
        }
    };

    let id = req.id.unwrap_or(Value::Null);

    match req.method.as_str() {
        "casx.status" => json!({
            "jsonrpc": "2.0",
            "result": {
                "in_game": state.in_game,
                "version": state.version,
                "pos": state.player_pos,
                "fps": state.fps,
                "ping_ms": state.ping_ms,
                "loaded_chunks": state.loaded_chunks
            },
            "id": id
        })
        .to_string(),
        "casx.connect" => {
            if let Some(params) = req.params {
                if let Some(ver) = params.get("version").and_then(|v| v.as_str()) {
                    state.version = ver.to_string();
                }
                state.in_game = true;
                json!({
                    "jsonrpc": "2.0",
                    "result": { "accepted": true },
                    "id": id
                })
                .to_string()
            } else {
                json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32602, "message": "Missing params for casx.connect" },
                    "id": id
                })
                .to_string()
            }
        }
        "casx.disconnect" => {
            state.in_game = false;
            json!({
                "jsonrpc": "2.0",
                "result": { "disconnected": true },
                "id": id
            })
            .to_string()
        }
        "casx.chat" => {
            if let Some(params) = req.params {
                if params.get("message").and_then(|m| m.as_str()).is_some() {
                    json!({
                        "jsonrpc": "2.0",
                        "result": { "sent": true },
                        "id": id
                    })
                    .to_string()
                } else {
                    json!({
                        "jsonrpc": "2.0",
                        "error": { "code": -32602, "message": "Missing 'message' string" },
                        "id": id
                    })
                    .to_string()
                }
            } else {
                json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32602, "message": "Missing params" },
                    "id": id
                })
                .to_string()
            }
        }
        "casx.click_slot" => {
            if let Some(params) = req.params {
                let slot = params.get("slot").and_then(|s| s.as_u64());
                if slot.is_some() {
                    json!({
                        "jsonrpc": "2.0",
                        "result": { "clicked": true },
                        "id": id
                    })
                    .to_string()
                } else {
                    json!({
                        "jsonrpc": "2.0",
                        "error": { "code": -32602, "message": "Missing 'slot' parameter" },
                        "id": id
                    })
                    .to_string()
                }
            } else {
                json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32602, "message": "Missing params" },
                    "id": id
                })
                .to_string()
            }
        }
        "casx.toggle_mod" => {
            if let Some(params) = req.params {
                let mod_id = params
                    .get("mod_id")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown");
                let enabled = params
                    .get("enabled")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false);
                json!({
                    "jsonrpc": "2.0",
                    "result": { "mod_id": mod_id, "enabled": enabled },
                    "id": id
                })
                .to_string()
            } else {
                json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32602, "message": "Missing params" },
                    "id": id
                })
                .to_string()
            }
        }
        unknown => json!({
            "jsonrpc": "2.0",
            "error": { "code": -32601, "message": format!("Method not found: {}", unknown) },
            "id": id
        })
        .to_string(),
    }
}

/// Helper executing IPC handling loop
pub struct CasxCtlServer {
    pub state: Arc<Mutex<EngineTelemetryState>>,
}

impl Default for CasxCtlServer {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(EngineTelemetryState::default())),
        }
    }
}

impl CasxCtlServer {
    pub fn handle_request(&self, req_str: &str) -> String {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        dispatch_json_rpc(req_str, &mut state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_casx_ctl_status_and_connect_flow() {
        let server = CasxCtlServer::default();

        // 1. Check initial casx.status
        let req_status = r#"{"jsonrpc":"2.0","method":"casx.status","id":1}"#;
        let res_status = server.handle_request(req_status);
        let val: Value = serde_json::from_str(&res_status).expect("Valid JSON response");
        assert_eq!(val["result"]["in_game"], false);
        assert_eq!(val["id"], 1);

        // 2. Dispatch casx.connect
        let req_connect = r#"{"jsonrpc":"2.0","method":"casx.connect","params":{"host":"127.0.0.1","port":25565,"version":"26.2"},"id":2}"#;
        let res_connect = server.handle_request(req_connect);
        let val_connect: Value = serde_json::from_str(&res_connect).expect("Valid JSON response");
        assert_eq!(val_connect["result"]["accepted"], true);

        // 3. Verify updated casx.status after connection
        let res_status2 = server.handle_request(req_status);
        let val2: Value = serde_json::from_str(&res_status2).expect("Valid JSON response");
        assert_eq!(val2["result"]["in_game"], true);
        assert_eq!(val2["result"]["version"], "26.2");
    }

    #[test]
    fn test_casx_ctl_unknown_method_error() {
        let server = CasxCtlServer::default();
        let req = r#"{"jsonrpc":"2.0","method":"invalid_method","id":99}"#;
        let res = server.handle_request(req);
        let val: Value = serde_json::from_str(&res).expect("Valid JSON response");
        assert_eq!(val["error"]["code"], -32601);
        assert_eq!(val["id"], 99);
    }
}
