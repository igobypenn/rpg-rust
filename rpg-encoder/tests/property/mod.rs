//! Property-based tests using proptest
//!
//! These tests verify invariants and properties of the codebase
//! using randomly generated inputs.

mod generators;
mod graph_property_test;
mod parser_property_test;
mod node_property_test;
