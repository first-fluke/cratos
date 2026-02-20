use super::super::config_manager::{ConfigManager, MacAddressGuide};
use super::types::{ConfigAction, ConfigInput};
use crate::error::{Error, Result};
use tracing::info;

pub async fn handle_wol(
    manager: &mut ConfigManager,
    params: &ConfigInput,
) -> Result<serde_json::Value> {
    match params.action {
        ConfigAction::Set => handle_wol_set(manager, params).await,
        ConfigAction::List => handle_wol_list(manager).await,
        ConfigAction::Get => handle_wol_get(manager, params).await,
        ConfigAction::Delete => handle_wol_delete(manager, params).await,
    }
}

async fn handle_wol_set(
    manager: &mut ConfigManager,
    params: &ConfigInput,
) -> Result<serde_json::Value> {
    let device_name = params.device_name.as_ref().ok_or_else(|| {
        Error::InvalidInput("device_name is required for WoL registration".to_string())
    })?;

    // If no MAC address, provide guidance
    let Some(mac_address) = &params.mac_address else {
        return Ok(serde_json::json!({
            "status": "needs_info",
            "target": "wol_device",
            "device_name": device_name,
            "message": format!("'{}'을(를) 등록하려면 MAC 주소가 필요해요.", device_name),
            "guidance": MacAddressGuide::instructions(),
            "next_step": "MAC 주소를 알려주시면 등록할게요. (예: AA:BB:CC:DD:EE:FF)"
        }));
    };

    // Register the device
    manager.register_wol_device(device_name, mac_address, None)?;

    info!(
        device = %device_name,
        mac = %mac_address,
        "WoL device registered via config tool"
    );

    Ok(serde_json::json!({
        "status": "success",
        "action": "set",
        "target": "wol_device",
        "device_name": device_name,
        "mac_address": mac_address,
        "message": format!("'{}' 디바이스가 등록되었어요! 이제 '{}' 켜줘 라고 말하면 됩니다.", device_name, device_name)
    }))
}

pub async fn handle_wol_list(manager: &ConfigManager) -> Result<serde_json::Value> {
    let devices = manager.list_wol_devices();

    if devices.is_empty() {
        return Ok(serde_json::json!({
            "status": "success",
            "action": "list",
            "target": "wol_device",
            "devices": [],
            "message": "등록된 WoL 디바이스가 없어요. 디바이스를 등록하려면 이름과 MAC 주소를 알려주세요."
        }));
    }

    let device_list: Vec<serde_json::Value> = devices
        .iter()
        .map(|(name, device)| {
            serde_json::json!({
                "name": name,
                "mac_address": device.mac_address,
                "description": device.description
            })
        })
        .collect();

    Ok(serde_json::json!({
        "status": "success",
        "action": "list",
        "target": "wol_device",
        "devices": device_list,
        "message": format!("등록된 WoL 디바이스 {}개", devices.len())
    }))
}

async fn handle_wol_get(
    manager: &ConfigManager,
    params: &ConfigInput,
) -> Result<serde_json::Value> {
    let device_name = params
        .device_name
        .as_ref()
        .ok_or_else(|| Error::InvalidInput("device_name is required".to_string()))?;

    let device = manager
        .get_wol_device(device_name)
        .ok_or_else(|| Error::NotFound(format!("Device '{}' not found", device_name)))?;

    Ok(serde_json::json!({
        "status": "success",
        "action": "get",
        "target": "wol_device",
        "device_name": device_name,
        "mac_address": device.mac_address,
        "description": device.description,
        "message": format!("'{}': {}", device_name, device.mac_address)
    }))
}

pub async fn handle_wol_delete(
    manager: &mut ConfigManager,
    params: &ConfigInput,
) -> Result<serde_json::Value> {
    let device_name = params
        .device_name
        .as_ref()
        .ok_or_else(|| Error::InvalidInput("device_name is required".to_string()))?;

    manager.delete_wol_device(device_name)?;

    info!(device = %device_name, "WoL device deleted");

    Ok(serde_json::json!({
        "status": "success",
        "action": "delete",
        "target": "wol_device",
        "device_name": device_name,
        "message": format!("'{}' 디바이스가 삭제되었어요.", device_name)
    }))
}
