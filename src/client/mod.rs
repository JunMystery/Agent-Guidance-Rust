pub mod cache;
pub mod http;
pub mod types;

#[cfg(test)]
mod tests;

pub use http::*;
pub use types::*;

use crate::config::current_config;

pub fn default_client() -> RemoteMlClient {
    RemoteMlClient::new(&current_config().server)
}
