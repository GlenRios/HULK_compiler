// src/parser/lalr/mod.rs
pub mod first_follow;
pub mod item;
pub mod automaton;
pub mod parse_table;
pub mod table_builder;

// Re-exports principales para uso desde el engine
pub use parse_table::{ParseTable, Action};
pub use table_builder::TableBuilder;
pub use first_follow::FirstFollow;