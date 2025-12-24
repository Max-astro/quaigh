//! Optimization of logic networks

mod infer_gates;
mod share_logic;
mod resubstitute;

pub use infer_gates::{infer_dffe, infer_xor_mux};
pub use share_logic::share_logic;
pub use resubstitute::substitute_node;
