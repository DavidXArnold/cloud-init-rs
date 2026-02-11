//! Configuration modules
//!
//! Each module handles a specific aspect of cloud-init configuration.
//! Modules are executed in a defined order during the config and final stages.

pub mod groups;
pub mod hostname;
pub mod locale;
pub mod runcmd;
pub mod ssh_keys;
pub mod timezone;
pub mod users;
pub mod write_files;

/// Module execution frequency
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frequency {
    /// Run once per instance
    PerInstance,
    /// Run once ever (across reboots and re-imaging)
    PerOnce,
    /// Run on every boot
    PerBoot,
    /// Always run
    Always,
}

/// Trait for configuration modules
pub trait Module {
    /// Name of this module
    fn name(&self) -> &'static str;

    /// Execution frequency for this module
    fn frequency(&self) -> Frequency {
        Frequency::PerInstance
    }
}
