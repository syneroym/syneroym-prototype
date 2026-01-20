pub mod capabilities;
pub mod runtime;
pub mod types;

pub use capabilities::HostCapabilities;
pub use runtime::WasmRuntime;
pub use types::*;

// Re-export generated bindings for use in transports
pub use runtime::Service;
