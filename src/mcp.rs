use crate::{deleter, scanner};
use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

const DEFAULT_PROTOCOL_VERSION: &str = "2025-06-18";

pub fn run() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Value>(&line) {
            Ok(message) => handle_message(message),
            Err(err) => Some(error_response(
                Value::Null,
                -32700,
                "Parse error",
                Some(json!({
                    "message": err.to_string(),
                })),
            )),
        };

        if let Some(response) = response {
            serde_json::to_writer(&mut stdout, &response)?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
        }
    }

    Ok(())
}

fn handle_message(message: Value) -> Option<Value> {
    let id = message.get("id").cloned()?;

    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Some(error_response(id, -32600, "Invalid Request", None));
    };

    match method {
        "initialize" => Some(success_response(id, initialize_result(&message))),
        "ping" => Some(success_response(id, json!({}))),
        "tools/list" => Some(success_response(id, tools_list_result())),
        "tools/call" => Some(
            call_tool(&message)
                .map(|result| success_response(id.clone(), result))
                .unwrap_or_else(|err| error_response(id, -32602, &err, None)),
        ),
        _ => Some(error_response(id, -32601, "Method not found", None)),
    }
}

fn initialize_result(message: &Value) -> Value {
    let requested_protocol = message
        .get("params")
        .and_then(|params| params.get("protocolVersion"))
        .and_then(Value::as_str)
        .filter(|version| !version.trim().is_empty())
        .unwrap_or(DEFAULT_PROTOCOL_VERSION);

    json!({
        "protocolVersion": requested_protocol,
        "capabilities": {
            "tools": {
                "listChanged": false
            }
        },
        "serverInfo": {
            "name": "irona",
            "title": "irona",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": "Use scan_artifacts to find build artifact directories, then call clean_artifacts only for paths the user has approved for deletion."
    })
}

fn tools_list_result() -> Value {
    json!({
        "tools": [
            {
                "name": "scan_artifacts",
                "title": "Scan build artifacts",
                "description": "Scan a directory and return build artifact directories with sizes and detected language or source.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Directory to scan. Defaults to the current working directory."
                        }
                    },
                    "additionalProperties": false
                },
                "outputSchema": artifact_scan_output_schema(),
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false,
                    "idempotentHint": true,
                    "openWorldHint": false
                }
            },
            {
                "name": "clean_artifacts",
                "title": "Clean build artifacts",
                "description": "Delete the provided artifact directories and return a summary of freed space.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "paths": {
                            "type": "array",
                            "description": "Artifact directories to delete.",
                            "items": {
                                "type": "string"
                            },
                            "minItems": 1
                        }
                    },
                    "required": ["paths"],
                    "additionalProperties": false
                },
                "outputSchema": clean_output_schema(),
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": true,
                    "idempotentHint": true,
                    "openWorldHint": false
                }
            }
        ]
    })
}

fn call_tool(message: &Value) -> Result<Value, String> {
    let params = message
        .get("params")
        .and_then(Value::as_object)
        .ok_or_else(|| "tools/call requires object params".to_string())?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "tools/call requires a string tool name".to_string())?;
    let arguments = params.get("arguments").unwrap_or(&Value::Null);

    match name {
        "scan_artifacts" => scan_artifacts(arguments),
        "clean_artifacts" => clean_artifacts(arguments),
        _ => Err(format!("Unknown tool: {name}")),
    }
}

fn scan_artifacts(arguments: &Value) -> Result<Value, String> {
    let path = match arguments {
        Value::Null => PathBuf::from("."),
        Value::Object(args) => args
            .get("path")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".")),
        _ => return Err("scan_artifacts arguments must be an object".to_string()),
    };
    let root = path.canonicalize().unwrap_or(path);
    let artifacts = scanner::scan_artifacts(&root);
    let total_size_bytes: u64 = artifacts.iter().map(|entry| entry.size_bytes).sum();
    let entries: Vec<Value> = artifacts
        .into_iter()
        .map(|entry| {
            json!({
                "path": entry.path.to_string_lossy(),
                "language": entry.language.to_string(),
                "size_bytes": entry.size_bytes
            })
        })
        .collect();

    let structured = json!({
        "root": root.to_string_lossy(),
        "count": entries.len(),
        "total_size_bytes": total_size_bytes,
        "artifacts": entries
    });
    let count = entries_len(&structured);
    Ok(tool_result(
        structured,
        format!(
            "Found {} artifact director{} totaling {} bytes.",
            count,
            if count == 1 { "y" } else { "ies" },
            total_size_bytes
        ),
        false,
    ))
}

fn clean_artifacts(arguments: &Value) -> Result<Value, String> {
    let paths = arguments
        .get("paths")
        .and_then(Value::as_array)
        .ok_or_else(|| "clean_artifacts requires a paths array".to_string())?;
    if paths.is_empty() {
        return Err("clean_artifacts requires at least one path".to_string());
    }

    let mut delete_inputs = Vec::with_capacity(paths.len());
    let mut original_paths = Vec::with_capacity(paths.len());
    let mut sizes = Vec::with_capacity(paths.len());

    for (index, path) in paths.iter().enumerate() {
        let Some(path) = path.as_str() else {
            return Err("clean_artifacts paths must all be strings".to_string());
        };
        let path = PathBuf::from(path);
        sizes.push(scanner::dir_size(&path));
        original_paths.push(path.clone());
        delete_inputs.push((index, path));
    }

    let runtime = tokio::runtime::Runtime::new().map_err(|err| err.to_string())?;
    let delete_results = runtime.block_on(deleter::delete_all(delete_inputs));

    let mut total_freed_bytes = 0u64;
    let mut deleted_count = 0usize;
    let mut results = Vec::with_capacity(delete_results.len());

    for result in delete_results {
        let size_bytes = sizes.get(result.index).copied().unwrap_or_default();
        let path = original_paths
            .get(result.index)
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default();
        let elapsed_ms = millis_u64(result.elapsed);

        match result.outcome {
            Ok(()) => {
                total_freed_bytes = total_freed_bytes.saturating_add(size_bytes);
                deleted_count += 1;
                results.push(json!({
                    "path": path,
                    "deleted": true,
                    "size_bytes": size_bytes,
                    "elapsed_ms": elapsed_ms
                }));
            }
            Err(err) => {
                results.push(json!({
                    "path": path,
                    "deleted": false,
                    "size_bytes": size_bytes,
                    "elapsed_ms": elapsed_ms,
                    "error": err.to_string()
                }));
            }
        }
    }

    let structured = json!({
        "requested_count": paths.len(),
        "deleted_count": deleted_count,
        "total_freed_bytes": total_freed_bytes,
        "results": results
    });
    Ok(tool_result(
        structured,
        format!(
            "Deleted {deleted_count} of {} requested director{} and freed {total_freed_bytes} bytes.",
            paths.len(),
            if paths.len() == 1 { "y" } else { "ies" },
        ),
        false,
    ))
}

fn artifact_scan_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "root": { "type": "string" },
            "count": { "type": "integer" },
            "total_size_bytes": { "type": "integer" },
            "artifacts": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "language": { "type": "string" },
                        "size_bytes": { "type": "integer" }
                    },
                    "required": ["path", "language", "size_bytes"]
                }
            }
        },
        "required": ["root", "count", "total_size_bytes", "artifacts"]
    })
}

fn clean_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "requested_count": { "type": "integer" },
            "deleted_count": { "type": "integer" },
            "total_freed_bytes": { "type": "integer" },
            "results": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "deleted": { "type": "boolean" },
                        "size_bytes": { "type": "integer" },
                        "elapsed_ms": { "type": "integer" },
                        "error": { "type": "string" }
                    },
                    "required": ["path", "deleted", "size_bytes", "elapsed_ms"]
                }
            }
        },
        "required": ["requested_count", "deleted_count", "total_freed_bytes", "results"]
    })
}

fn tool_result(structured_content: Value, text: String, is_error: bool) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "structuredContent": structured_content,
        "isError": is_error
    })
}

fn success_response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn error_response(id: Value, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut error = json!({
        "code": code,
        "message": message
    });

    if let Some(data) = data {
        error["data"] = data;
    }

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": error
    })
}

fn entries_len(structured: &Value) -> usize {
    structured
        .get("count")
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize
}

fn millis_u64(duration: std::time::Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn lists_scan_and_clean_tools() {
        let response = handle_message(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list"
        }))
        .unwrap();

        let tools = response["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "scan_artifacts");
        assert_eq!(tools[1]["name"], "clean_artifacts");
    }

    #[test]
    fn scans_artifacts_with_structured_content() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(tmp.path().join("target")).unwrap();
        fs::write(tmp.path().join("target").join("artifact.bin"), "data").unwrap();

        let response = handle_message(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "scan_artifacts",
                "arguments": {
                    "path": tmp.path().to_string_lossy()
                }
            }
        }))
        .unwrap();

        let structured = &response["result"]["structuredContent"];
        assert_eq!(structured["count"], 1);
        assert_eq!(structured["total_size_bytes"], 4);
        assert_eq!(structured["artifacts"][0]["language"], "Rust");
    }

    #[test]
    fn cleans_requested_artifacts() {
        let tmp = TempDir::new().unwrap();
        let artifact = tmp.path().join("target");
        fs::create_dir(&artifact).unwrap();
        fs::write(artifact.join("artifact.bin"), "data").unwrap();

        let response = handle_message(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "clean_artifacts",
                "arguments": {
                    "paths": [artifact.to_string_lossy()]
                }
            }
        }))
        .unwrap();

        let structured = &response["result"]["structuredContent"];
        assert_eq!(structured["requested_count"], 1);
        assert_eq!(structured["deleted_count"], 1);
        assert_eq!(structured["total_freed_bytes"], 4);
        assert!(!artifact.exists());
    }
}
