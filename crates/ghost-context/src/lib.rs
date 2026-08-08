//! Composable presentation of domain evidence to an agent runtime.
//!
//! Compilers own ordering, labels, serialization and the response contract. Agent runtimes receive
//! only [`CompiledContext`] and must not reconstruct or discriminate workflow-specific data.

mod compiler;
mod model;
mod recipe;

pub use compiler::*;
pub use model::*;
pub use recipe::*;
