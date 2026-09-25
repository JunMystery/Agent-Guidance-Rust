pub mod schema;
pub mod store;
pub mod watcher;

#[cfg(test)]
mod tests;

pub use schema::*;
pub use store::*;
pub use watcher::*;

use std::sync::Arc;

pub fn current_config() -> Arc<AppConfig> {
    ConfigWatcher::global().current()
}

pub fn is_remote_mode() -> bool {
    current_config().server.is_remote()
}
