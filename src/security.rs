use crate::models::Permissions;
use anyhow::{bail, Result};

pub struct Security;

impl Security {
    pub fn validate_permissions(
        skill_name: &str,
        requested_action: &str,
        granted_permissions: &Permissions,
    ) -> Result<()> {
        match requested_action {
            "network" => {
                if !granted_permissions.network {
                    bail!(
                        "Security Violation: Skill '{}' denied network access.",
                        skill_name
                    );
                }
            }
            "filesystem_read" => {
                if !granted_permissions.filesystem_read {
                    bail!(
                        "Security Violation: Skill '{}' denied filesystem read access.",
                        skill_name
                    );
                }
            }
            "filesystem_write" => {
                if !granted_permissions.filesystem_write {
                    bail!(
                        "Security Violation: Skill '{}' denied filesystem write access.",
                        skill_name
                    );
                }
            }
            "process_exec" => {
                if !granted_permissions.process_exec {
                    bail!(
                        "Security Violation: Skill '{}' denied process execution.",
                        skill_name
                    );
                }
            }
            _ => bail!("Unknown permission requested: {}", requested_action),
        }
        Ok(())
    }
}
