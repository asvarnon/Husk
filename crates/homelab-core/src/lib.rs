//! # Homelab Core
//!
//! Pure capabilities and data models for homelab automation.

pub mod client;
pub mod config;
pub mod error;
pub mod models;
pub mod tools;
pub mod utils;

pub use client::HomelabClient;
pub use config::Config;
pub use error::{HomelabError, Result};
pub use models::*;
pub use tools::*;
