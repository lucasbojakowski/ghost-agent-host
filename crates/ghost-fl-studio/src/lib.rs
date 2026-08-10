//! Experimental FL Studio adapter built on the native Gopher WebView bridge.
//!
//! The adapter deliberately owns a single-flight control link because the observed
//! `script_handler.runJson` callback surface has no usable correlation ID. Tool arguments
//! are canonicalized from the live MCP schema before every call because FL Studio 26.1.3's
//! Gopher dispatcher is order-sensitive in practice.

mod adapter;
mod codex_tools;
mod processor_tools;
mod transport;

pub use adapter::{
    AdapterError, CapabilityManifest, FlCapability, FlStudioAdapterConfig, GopherNativeAdapter,
    MutationRecord, NativeToolDefinition, NativeToolResult, VerifiedMutation, DEFAULT_DEBUG_PORT,
};
pub use codex_tools::{FlAgentToolPolicy, FlPluginWriteScope};
pub use processor_tools::register_codex_tools;
