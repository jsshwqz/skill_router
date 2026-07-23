use std::collections::HashSet;

use agent_client_protocol::schema::v1::{SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectOption};
use aion_router::config::{ai_model, candidate_ai_endpoints, AiEndpoint};
use anyhow::{bail, Result};

/// The result of resolving a model selected by an ACP client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelResolution {
    /// Let Forge choose from the enabled endpoint priority order.
    Auto,
    /// Use the endpoint whose model identifier exactly matches the selection.
    Exact(AiEndpoint),
}

/// Enabled AI endpoints exposed to ACP clients as model choices.
#[derive(Debug, Clone)]
pub struct ModelCatalog {
    endpoints: Vec<AiEndpoint>,
    default_model: String,
}

impl ModelCatalog {
    /// Build a catalog from configured endpoints, preserving the first endpoint for each model ID.
    pub fn from_endpoints(endpoints: Vec<AiEndpoint>, requested_default: Option<&str>) -> Self {
        let mut seen_models = HashSet::new();
        let endpoints: Vec<_> = endpoints
            .into_iter()
            .filter(|endpoint| !AiEndpoint::is_disabled(&endpoint.label))
            .filter(|endpoint| {
                !endpoint.model.trim().is_empty()
                    && !endpoint.base_url.trim().is_empty()
                    && !endpoint.api_key.trim().is_empty()
            })
            .filter(|endpoint| seen_models.insert(endpoint.model.clone()))
            .collect();

        let default_model = requested_default
            .filter(|requested| endpoints.iter().any(|endpoint| endpoint.model == **requested))
            .map(str::to_owned)
            .or_else(|| endpoints.first().map(|endpoint| endpoint.model.clone()))
            .unwrap_or_else(|| "auto".to_string());

        Self {
            endpoints,
            default_model,
        }
    }

    /// Build a model catalog from the current Forge AI environment configuration.
    pub fn from_environment() -> Self {
        let requested_default = ai_model();
        Self::from_endpoints(candidate_ai_endpoints(), Some(&requested_default))
    }

    /// Return the model selected by default for new ACP sessions.
    pub fn default_model(&self) -> &str {
        &self.default_model
    }

    /// Return enabled model identifiers in endpoint priority order.
    pub fn model_ids(&self) -> Vec<&str> {
        self.endpoints.iter().map(|endpoint| endpoint.model.as_str()).collect()
    }

    /// Resolve an ACP selection without silently substituting another model.
    pub fn resolve(&self, selected: &str) -> Result<ModelResolution> {
        if selected == "auto" {
            if self.endpoints.is_empty() {
                bail!("no enabled AI models are configured for Aion Forge");
            }
            return Ok(ModelResolution::Auto);
        }

        if let Some(endpoint) = self.endpoints.iter().find(|endpoint| endpoint.model == selected) {
            return Ok(ModelResolution::Exact(endpoint.clone()));
        }

        let available = self.model_ids();
        bail!(
            "unknown model '{selected}'; available models: {}",
            if available.is_empty() {
                "none".to_string()
            } else {
                available.join(", ")
            }
        )
    }

    /// Build the standard ACP model selector advertised to the client.
    pub fn session_config_option(&self) -> SessionConfigOption {
        self.session_config_option_for(&self.default_model)
    }

    /// Build the standard ACP model selector with an explicit current session value.
    pub fn session_config_option_for(&self, current_model: &str) -> SessionConfigOption {
        let mut options = vec![SessionConfigSelectOption::new("auto", "Auto")];
        options.extend(
            self.endpoints
                .iter()
                .map(|endpoint| SessionConfigSelectOption::new(endpoint.model.clone(), endpoint.model.clone())),
        );

        SessionConfigOption::select("model", "Model", current_model.to_string(), options)
            .category(SessionConfigOptionCategory::Model)
    }
}

#[cfg(test)]
mod tests {
    use aion_router::config::{AiEndpoint, AiProtocol};

    use super::{ModelCatalog, ModelResolution};

    fn endpoint(label: &str, base_url: &str, api_key: &str, model: &str) -> AiEndpoint {
        AiEndpoint {
            label: label.to_string(),
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            protocol: AiProtocol::OpenAiChat,
        }
    }

    #[test]
    fn filters_empty_entries_and_deduplicates_models_by_priority() {
        let catalog = ModelCatalog::from_endpoints(
            vec![
                endpoint("first", "https://first.example/v1", "key", "shared"),
                endpoint("duplicate", "https://second.example/v1", "key", "shared"),
                endpoint("empty-model", "https://example.com/v1", "key", ""),
                endpoint("empty-url", "", "key", "missing-url"),
                endpoint("empty-key", "https://example.com/v1", "", "missing-key"),
                endpoint("second", "https://second.example/v1", "key", "second"),
            ],
            None,
        );

        assert_eq!(catalog.model_ids(), vec!["shared", "second"]);
        assert!(matches!(
            catalog.resolve("shared"),
            Ok(ModelResolution::Exact(endpoint)) if endpoint.label == "first"
        ));
    }

    #[test]
    fn selects_requested_default_only_when_it_is_enabled() {
        let endpoints = vec![
            endpoint("first", "https://first.example/v1", "key", "first-model"),
            endpoint("second", "https://second.example/v1", "key", "second-model"),
        ];

        let requested = ModelCatalog::from_endpoints(endpoints.clone(), Some("second-model"));
        let missing = ModelCatalog::from_endpoints(endpoints, Some("missing-model"));

        assert_eq!(requested.default_model(), "second-model");
        assert_eq!(missing.default_model(), "first-model");
    }

    #[test]
    fn rejects_unknown_model_without_fallback() {
        let catalog = ModelCatalog::from_endpoints(
            vec![endpoint("deepseek", "https://api.example/v1", "key", "deepseek-chat")],
            None,
        );

        let error = catalog.resolve("missing-model").unwrap_err();

        assert!(error.to_string().contains("missing-model"));
        assert!(error.to_string().contains("deepseek-chat"));
    }

    #[test]
    fn auto_is_the_only_fallback_selection() {
        let catalog = ModelCatalog::from_endpoints(
            vec![endpoint("deepseek", "https://api.example/v1", "key", "deepseek-chat")],
            None,
        );

        assert!(matches!(catalog.resolve("auto"), Ok(ModelResolution::Auto)));
        assert!(matches!(
            catalog.resolve("deepseek-chat"),
            Ok(ModelResolution::Exact(endpoint)) if endpoint.model == "deepseek-chat"
        ));
    }

    #[test]
    fn empty_catalog_exposes_auto_but_cannot_execute_it() {
        let catalog = ModelCatalog::from_endpoints(Vec::new(), None);

        assert_eq!(catalog.default_model(), "auto");
        assert_eq!(catalog.model_ids(), Vec::<&str>::new());
        assert!(catalog
            .resolve("auto")
            .unwrap_err()
            .to_string()
            .contains("no enabled AI models"));
    }

    #[test]
    fn session_config_contains_only_auto_and_enabled_models() {
        let catalog = ModelCatalog::from_endpoints(
            vec![
                endpoint("first", "https://first.example/v1", "key", "first-model"),
                endpoint("second", "https://second.example/v1", "key", "second-model"),
            ],
            Some("second-model"),
        );

        let option = serde_json::to_value(catalog.session_config_option()).unwrap();

        assert_eq!(option["id"], "model");
        assert_eq!(option["name"], "Model");
        assert_eq!(option["category"], "model");
        assert_eq!(option["currentValue"], "second-model");
        assert_eq!(option["options"][0]["value"], "auto");
        assert_eq!(option["options"][1]["value"], "first-model");
        assert_eq!(option["options"][2]["value"], "second-model");
        assert_eq!(option["options"].as_array().unwrap().len(), 3);
    }
}
