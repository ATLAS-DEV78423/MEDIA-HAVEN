//! Named renderer command surface (ADR-0005).
//!
//! One command per backend renderer operation. There is deliberately no
//! generic `http_request` / `api_request` passthrough.

mod renderer;

pub use renderer::*;
