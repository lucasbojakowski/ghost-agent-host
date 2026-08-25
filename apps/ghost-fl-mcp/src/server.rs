use std::{borrow::Cow, sync::Arc};

use ghost_fl_studio::{
    AdapterError, FlStudioManifest, GopherNativeAdapter, NativeToolDefinition, NativeToolResult,
};
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorCode,
        Implementation, JsonObject, ListToolsResult, PaginatedRequestParams, ProtocolVersion,
        ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
    ErrorData, RoleServer, ServerHandler,
};
use serde_json::Value;
use thiserror::Error;

pub const MCP_PROTOCOL_VERSION: &str = "2026-07-28";

#[derive(Debug, Error)]
pub enum McpEdgeError {
    #[error("live Gopher tool `{tool}` input schema is not a JSON object")]
    InvalidToolSchema { tool: String },
}

trait NativeToolCaller: Send + Sync {
    fn call_native(&self, tool: &str, arguments: Value) -> Result<NativeToolResult, AdapterError>;
}

impl NativeToolCaller for GopherNativeAdapter {
    fn call_native(&self, tool: &str, arguments: Value) -> Result<NativeToolResult, AdapterError> {
        GopherNativeAdapter::call_native(self, tool, arguments)
    }
}

#[derive(Debug, Error)]
enum DispatchError {
    #[error("MCP tool `{0}` is not present in the live Gopher manifest")]
    UnknownTool(String),
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    #[error("FL Studio tool worker failed: {0}")]
    Worker(String),
}

#[derive(Clone)]
pub struct FlMcpServer {
    caller: Arc<dyn NativeToolCaller>,
    tools: Arc<Vec<Tool>>,
}

impl FlMcpServer {
    pub fn from_gopher(
        manifest: &FlStudioManifest,
        adapter: Arc<GopherNativeAdapter>,
    ) -> Result<Self, McpEdgeError> {
        Self::new_with_caller(manifest, adapter)
    }

    fn new_with_caller<C>(manifest: &FlStudioManifest, caller: Arc<C>) -> Result<Self, McpEdgeError>
    where
        C: NativeToolCaller + 'static,
    {
        Ok(Self {
            caller,
            tools: Arc::new(mcp_tools_from_manifest(manifest)?),
        })
    }

    fn find_tool(&self, name: &str) -> Option<&Tool> {
        self.tools
            .binary_search_by(|tool| tool.name.as_ref().cmp(name))
            .ok()
            .map(|index| &self.tools[index])
    }

    async fn dispatch(
        &self,
        name: String,
        arguments: JsonObject,
    ) -> Result<NativeToolResult, DispatchError> {
        if self.find_tool(&name).is_none() {
            return Err(DispatchError::UnknownTool(name));
        }
        let caller = Arc::clone(&self.caller);
        tokio::task::spawn_blocking(move || caller.call_native(&name, Value::Object(arguments)))
            .await
            .map_err(|error| DispatchError::Worker(error.to_string()))?
            .map_err(DispatchError::Adapter)
    }
}

impl ServerHandler for FlMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::V_2026_07_28)
            .with_server_info(Implementation::new(
                "ghost-fl-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Raw live FL Studio/Gopher capability surface. FL Studio is authoritative; tool names, descriptions and input schemas come from the live Gopher manifest.",
            )
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Owned(vec![ProtocolVersion::V_2026_07_28])
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: self.tools.as_ref().clone(),
            ..Default::default()
        })
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.find_tool(name).cloned()
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let name = request.name.into_owned();
        let arguments = request.arguments.unwrap_or_default();
        match self.dispatch(name, arguments).await {
            Ok(result) => Ok(native_result_to_mcp(result).into()),
            Err(DispatchError::UnknownTool(name))
            | Err(DispatchError::Adapter(AdapterError::UnknownTool(name))) => Err(ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                format!("FL Studio tool `{name}` is not present in the live Gopher manifest"),
                None,
            )),
            Err(DispatchError::Adapter(error)) => Ok(adapter_error_to_mcp(error).into()),
            Err(DispatchError::Worker(message)) => Err(ErrorData::internal_error(message, None)),
        }
    }
}

fn mcp_tools_from_manifest(manifest: &FlStudioManifest) -> Result<Vec<Tool>, McpEdgeError> {
    let mut tools = manifest
        .tools
        .iter()
        .map(mcp_tool_from_native)
        .collect::<Result<Vec<_>, _>>()?;
    tools.sort_by(|left, right| left.name.as_ref().cmp(right.name.as_ref()));
    Ok(tools)
}

fn mcp_tool_from_native(definition: &NativeToolDefinition) -> Result<Tool, McpEdgeError> {
    let Value::Object(input_schema) = &definition.input_schema else {
        return Err(McpEdgeError::InvalidToolSchema {
            tool: definition.name.clone(),
        });
    };
    Ok(Tool::new(
        definition.name.clone(),
        definition.description.clone(),
        input_schema.clone(),
    ))
}

fn native_result_to_mcp(result: NativeToolResult) -> CallToolResult {
    let NativeToolResult {
        raw, content_text, ..
    } = result;
    let mut content = content_text
        .into_iter()
        .map(ContentBlock::text)
        .collect::<Vec<_>>();
    if content.is_empty() {
        content.push(ContentBlock::text(raw.to_string()));
    }
    let mut mapped = CallToolResult::success(content);
    mapped.structured_content = Some(raw);
    mapped
}

fn adapter_error_to_mcp(error: AdapterError) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(error.to_string())])
}

#[cfg(test)]
mod tests;
