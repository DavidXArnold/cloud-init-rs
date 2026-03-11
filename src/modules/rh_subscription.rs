//! Red Hat subscription management module
//!
//! Registers a RHEL/CentOS system with the Red Hat subscription manager
//! (`subscription-manager`) using either username/password or an activation key.
//!
//! # Cloud-config example
//!
//! ```yaml
//! rh_subscription:
//!   username: user@example.com
//!   password: mypassword
//!   auto-attach: true
//!   service-level: self-support
//!   enable-repo:
//!     - rhel-7-server-optional-rpms
//!   disable-repo:
//!     - rhel-7-server-extras-rpms
//! ```
//!
//! Or with an activation key:
//!
//! ```yaml
//! rh_subscription:
//!   activation-key: myactivationkey
//!   org: "1234567"
//!   add-pool:
//!     - 8a85f9833e1d21f2013e1d21c6200011
//! ```

use crate::CloudInitError;
use crate::config::RhSubscriptionConfig;
use tracing::{debug, info, warn};

/// Register the system and apply subscription configuration.
///
/// This is the main entry point called from the config stage.  It validates
/// the supplied config, registers the system with RHSM, optionally attaches
/// pools, and enables/disables the requested repositories.
pub async fn configure_rh_subscription(
    config: &RhSubscriptionConfig,
) -> Result<(), CloudInitError> {
    info!("rh_subscription: starting Red Hat subscription configuration");

    // Validate that we have enough information to register
    validate_config(config)?;

    // Register (or verify already registered)
    register(config).await?;

    // Attach pools
    if !config.add_pool.is_empty() {
        attach_pools(&config.add_pool).await?;
    } else if config.auto_attach == Some(true) {
        auto_attach(config.service_level.as_deref()).await?;
    }

    // Enable repositories
    if !config.enable_repo.is_empty() {
        enable_repos(&config.enable_repo).await?;
    }

    // Disable repositories
    if !config.disable_repo.is_empty() {
        disable_repos(&config.disable_repo).await?;
    }

    info!("rh_subscription: subscription configuration complete");
    Ok(())
}

/// Validate that the config contains the minimum fields needed to register.
fn validate_config(config: &RhSubscriptionConfig) -> Result<(), CloudInitError> {
    let has_user_pass = config.username.is_some() && config.password.is_some();
    let has_key_org = config.activation_key.is_some() && config.org.is_some();

    if !has_user_pass && !has_key_org {
        return Err(CloudInitError::Module {
            module: "rh_subscription".to_string(),
            message: "rh_subscription requires either (username + password) \
                      or (activation-key + org)"
                .to_string(),
        });
    }

    Ok(())
}

/// Register the system with subscription-manager.
async fn register(config: &RhSubscriptionConfig) -> Result<(), CloudInitError> {
    let mut args: Vec<String> = vec!["register".to_string(), "--force".to_string()];

    // Optional server/RHSM overrides
    if let Some(ref hostname) = config.server_hostname {
        args.push(format!("--serverurl={}", hostname));
    }
    if let Some(ref baseurl) = config.rhsm_baseurl {
        args.push(format!("--baseurl={}", baseurl));
    }

    // Authentication: activation key takes precedence
    if let (Some(key), Some(org)) = (&config.activation_key, &config.org) {
        debug!("rh_subscription: registering with activation key");
        args.push(format!("--activationkey={}", key));
        args.push(format!("--org={}", org));
    } else if let (Some(user), Some(pass)) = (&config.username, &config.password) {
        debug!("rh_subscription: registering with username/password");
        args.push(format!("--username={}", user));
        args.push(format!("--password={}", pass));
    }

    run_subscription_manager(&args).await
}

/// Auto-attach the best matching subscription, optionally with a service level.
async fn auto_attach(service_level: Option<&str>) -> Result<(), CloudInitError> {
    let mut args = vec!["attach".to_string(), "--auto".to_string()];

    if let Some(level) = service_level {
        args.push(format!("--servicelevel={}", level));
    }

    info!("rh_subscription: auto-attaching subscription");
    run_subscription_manager(&args).await
}

/// Attach one or more pool IDs.
async fn attach_pools(pools: &[String]) -> Result<(), CloudInitError> {
    for pool in pools {
        info!("rh_subscription: attaching pool {}", pool);
        let args = vec!["attach".to_string(), format!("--pool={}", pool)];
        run_subscription_manager(&args).await?;
    }
    Ok(())
}

/// Enable one or more repositories via `subscription-manager repos --enable`.
async fn enable_repos(repos: &[String]) -> Result<(), CloudInitError> {
    let mut args = vec!["repos".to_string()];
    for repo in repos {
        args.push(format!("--enable={}", repo));
    }
    info!("rh_subscription: enabling {} repo(s)", repos.len());
    run_subscription_manager(&args).await
}

/// Disable one or more repositories via `subscription-manager repos --disable`.
async fn disable_repos(repos: &[String]) -> Result<(), CloudInitError> {
    let mut args = vec!["repos".to_string()];
    for repo in repos {
        args.push(format!("--disable={}", repo));
    }
    info!("rh_subscription: disabling {} repo(s)", repos.len());
    run_subscription_manager(&args).await
}

/// Execute `subscription-manager` with the given arguments.
async fn run_subscription_manager(args: &[String]) -> Result<(), CloudInitError> {
    debug!("subscription-manager {}", args.join(" "));

    let output = tokio::process::Command::new("subscription-manager")
        .args(args)
        .output()
        .await
        .map_err(|e| CloudInitError::Command(format!("subscription-manager: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        // subscription-manager sometimes writes errors to stdout
        let detail = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        warn!("subscription-manager failed: {}", detail);
        return Err(CloudInitError::Module {
            module: "rh_subscription".to_string(),
            message: format!("subscription-manager failed: {}", detail),
        });
    }

    Ok(())
}
