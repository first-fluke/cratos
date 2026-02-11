//! Device pairing CLI commands
//!
//! `cratos pair start`   — Generate PIN for pairing
//! `cratos pair devices`  — List paired devices
//! `cratos pair unpair`   — Remove a paired device

use super::PairCommands;
use anyhow::Result;
use cratos_core::pairing::PairingManager;

/// Run pair command
pub async fn run(cmd: PairCommands) -> Result<()> {
    let manager = PairingManager::new();

    match cmd {
        PairCommands::Start => start_pairing(&manager).await,
        PairCommands::Devices => list_devices(&manager).await,
        PairCommands::Unpair { device_id } => unpair_device(&manager, &device_id).await,
    }
}

/// Start pairing: generate and display PIN
async fn start_pairing(manager: &PairingManager) -> Result<()> {
    let pin = manager.start_pairing().await;

    println!();
    println!("  Device Pairing");
    println!("  {}", "-".repeat(40));
    println!();
    println!("  PIN:  {}", pin);
    println!();
    println!("  Enter this PIN on your mobile device");
    println!("  within 5 minutes to complete pairing.");
    println!();
    println!("  The device will also need the server's");
    println!("  address (use mDNS or enter manually).");
    println!();

    Ok(())
}

/// List all paired devices
async fn list_devices(manager: &PairingManager) -> Result<()> {
    let devices = manager.list_devices().await;

    if devices.is_empty() {
        println!("\nNo paired devices.");
        println!("Run `cratos pair start` to pair a device.\n");
        return Ok(());
    }

    println!("\nPaired Devices ({})\n{}", devices.len(), "-".repeat(60));

    for device in &devices {
        let pk_hex: String = device
            .public_key
            .iter()
            .take(4)
            .map(|b| format!("{:02x}", b))
            .collect();
        println!(
            "  {} — {} (paired: {}, key: {}...)",
            device.device_id,
            device.device_name,
            device.paired_at.format("%Y-%m-%d %H:%M"),
            pk_hex,
        );
    }
    println!();

    Ok(())
}

/// Unpair a device by ID
async fn unpair_device(manager: &PairingManager, device_id: &str) -> Result<()> {
    if manager.unpair_device(device_id).await {
        println!("Device {} unpaired.", device_id);
    } else {
        println!("Device {} not found.", device_id);
    }
    Ok(())
}
