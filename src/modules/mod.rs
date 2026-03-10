//! Configuration modules
//!
//! Each module handles a specific aspect of cloud-init configuration.
//! Modules are executed in a defined order during the config and final stages.

pub mod apt;
pub mod bootcmd;
pub mod groups;
pub mod hostname;
pub mod locale;
pub mod ntp;
pub mod packages;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frequency_variants() {
        let per_instance = Frequency::PerInstance;
        let per_once = Frequency::PerOnce;
        let per_boot = Frequency::PerBoot;
        let always = Frequency::Always;

        assert_eq!(per_instance, Frequency::PerInstance);
        assert_eq!(per_once, Frequency::PerOnce);
        assert_eq!(per_boot, Frequency::PerBoot);
        assert_eq!(always, Frequency::Always);
        assert_ne!(per_instance, Frequency::Always);
    }

    #[test]
    fn test_frequency_debug() {
        let f = Frequency::PerInstance;
        assert_eq!(format!("{f:?}"), "PerInstance");
    }

    #[test]
    fn test_frequency_clone() {
        let f = Frequency::PerBoot;
        let cloned = f;
        assert_eq!(f, cloned);
    }

    struct TestModule;
    impl Module for TestModule {
        fn name(&self) -> &'static str {
            "test_module"
        }
    }

    #[test]
    fn test_module_trait_default_frequency() {
        let m = TestModule;
        assert_eq!(m.name(), "test_module");
        assert_eq!(m.frequency(), Frequency::PerInstance);
    }

    struct CustomFreqModule;
    impl Module for CustomFreqModule {
        fn name(&self) -> &'static str {
            "custom"
        }
        fn frequency(&self) -> Frequency {
            Frequency::PerBoot
        }
    }

    #[test]
    fn test_module_trait_custom_frequency() {
        let m = CustomFreqModule;
        assert_eq!(m.frequency(), Frequency::PerBoot);
    }
}
