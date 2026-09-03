pub mod circuit;
pub mod classification;

pub use circuit::{CircuitBreaker, CircuitState};
pub use classification::{ClassifiedFailure, FailureKind};
