pub mod setup;
pub(crate) mod setup_targets;
pub(crate) mod setup_cli;
pub mod verify;
pub mod clients;
pub mod rules;
pub(crate) mod rules_cleaner;
pub mod uninstall;
pub mod upgrade;

pub use setup::run_setup;
pub use verify::run_verify_setup;
pub use uninstall::run_uninstall;
pub use upgrade::run_upgrade;
pub use rules::replace_or_append_tagged_section;

