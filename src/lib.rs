#![allow(
    clippy::wildcard_imports,
    clippy::non_ascii_literal,
    clippy::missing_errors_doc,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::module_name_repetitions
)]

pub mod args;
pub mod config;
pub mod db;
pub mod errors;
pub mod nice;
pub mod patterns;
pub mod scan;
pub mod schedule;
pub mod utils;
