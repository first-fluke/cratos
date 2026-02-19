use crate::error::{Error, Result};
use tracing::{info};
use super::types::{ConfigInput, ConfigAction, ConfigTarget};
use super::super::config_manager::{ConfigManager};
use super::wol;

pub async fn handle_set(manager: &mut ConfigManager, params: &ConfigInput) -> Result<serde_json::Value> {
    let value = params.value.as_ref().ok_or_else(|| {
        Error::InvalidInput(format!(
            "value is required for setting {}. Available options: {:?}",
            params.target.display_name(),
            params.target.available_options()
        ))
    })?;

    // Validate value against available options
    let options = params.target.available_options();
    if !options.is_empty() {
        let value_lower = value.to_lowercase();
        let matched = options
            .iter()
            .find(|&opt| opt.to_lowercase() == value_lower);

        if matched.is_none() {
            return Err(Error::InvalidInput(format!(
                "Invalid value '{}' for {}. Available: {:?}",
                value,
                params.target.display_name(),
                options
            )));
        }
    }

    // Update configuration via ConfigManager
    let config = manager.config_mut();

    match params.target {
        ConfigTarget::LlmProvider => config.llm.default_provider = value.clone(),
        ConfigTarget::LlmModel => config.llm.default_model = value.clone(),
        ConfigTarget::Language => config.language = value.clone(),
        ConfigTarget::Persona => config.persona = value.clone(),
        ConfigTarget::Scheduler => {
            config.scheduler.enabled = value == "enable" || value == "enabled";
        }
        ConfigTarget::Channel => config.channel = value.clone(),
        ConfigTarget::Theme => config.theme = value.clone(),
        _ => {}
    }

    manager.save()?;

    info!(
        target = ?params.target,
        value = %value,
        "Configuration updated"
    );

    Ok(serde_json::json!({
        "status": "success",
        "action": "set",
        "target": params.target.display_name(),
        "value": value,
        "message": format!("{} → {}", params.target.display_name(), value)
    }))
}

pub async fn handle_get(manager: &ConfigManager, params: &ConfigInput) -> Result<serde_json::Value> {
    let config = manager.config();

    let current_value = match params.target {
        ConfigTarget::LlmProvider => {
            if config.llm.default_provider.is_empty() {
                "auto".to_string()
            } else {
                config.llm.default_provider.clone()
            }
        }
        ConfigTarget::LlmModel => {
            if config.llm.default_model.is_empty() {
                "auto".to_string()
            } else {
                config.llm.default_model.clone()
            }
        }
        ConfigTarget::Language => {
            if config.language.is_empty() {
                "en".to_string()
            } else {
                config.language.clone()
            }
        }
        ConfigTarget::Persona => {
            if config.persona.is_empty() {
                "cratos".to_string()
            } else {
                config.persona.clone()
            }
        }
        ConfigTarget::Scheduler => {
            if config.scheduler.enabled {
                "enabled".to_string()
            } else {
                "disabled".to_string()
            }
        }
        ConfigTarget::Channel => {
            if config.channel.is_empty() {
                "telegram".to_string()
            } else {
                config.channel.clone()
            }
        }
        ConfigTarget::Theme => {
            if config.theme.is_empty() {
                "dark".to_string()
            } else {
                config.theme.clone()
            }
        }
        ConfigTarget::WolDevice => {
            return wol::handle_wol_list(manager).await;
        }
    };

    Ok(serde_json::json!({
        "status": "success",
        "action": "get",
        "target": params.target.display_name(),
        "current_value": current_value,
        "available_options": params.target.available_options(),
        "message": format!("{}: {}", params.target.display_name(), current_value)
    }))
}

pub async fn handle_list(manager: &ConfigManager, params: &ConfigInput) -> Result<serde_json::Value> {
    if params.target == ConfigTarget::WolDevice {
        return wol::handle_wol_list(manager).await;
    }

    let options = params.target.available_options();

    Ok(serde_json::json!({
        "status": "success",
        "action": "list",
        "target": params.target.display_name(),
        "options": options,
        "message": format!("{} 옵션: {}", params.target.display_name(), options.join(", "))
    }))
}

pub async fn handle_delete(manager: &mut ConfigManager, params: &ConfigInput) -> Result<serde_json::Value> {
    if params.target == ConfigTarget::WolDevice {
        return wol::handle_wol_delete(manager, params).await;
    }

    // For other targets, "delete" means "reset to default"
    let default_value = match params.target {
        ConfigTarget::LlmProvider => "auto",
        ConfigTarget::LlmModel => "auto",
        ConfigTarget::Language => "en",
        ConfigTarget::Persona => "cratos",
        ConfigTarget::Channel => "telegram",
        ConfigTarget::Theme => "system",
        ConfigTarget::Scheduler => "disabled",
        ConfigTarget::WolDevice => unreachable!(),
    };

    // Update via set with default value
    let reset_params = ConfigInput {
        action: ConfigAction::Set,
        target: params.target,
        value: Some(default_value.to_string()),
        device_name: None,
        mac_address: None,
    };

    let mut result = handle_set(manager, &reset_params).await?;
    if let Some(obj) = result.as_object_mut() {
        obj.insert("action".to_string(), serde_json::json!("delete"));
        obj.insert(
            "message".to_string(),
            serde_json::json!(format!(
                "{} 초기화됨 → {}",
                params.target.display_name(),
                default_value
            )),
        );
    }

    Ok(result)
}
