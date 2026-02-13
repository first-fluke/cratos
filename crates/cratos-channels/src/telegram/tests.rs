//! Tests for telegram module

use super::*;
use crate::message::MessageButton;

#[test]
fn test_telegram_config() {
    let config = TelegramConfig::new("test_token")
        .with_allowed_users(vec![123, 456])
        .with_groups_mention_only(false);

    assert_eq!(config.bot_token, "test_token");
    assert_eq!(config.allowed_users, vec![123, 456]);
    assert!(!config.groups_mention_only);
}

#[test]
fn test_build_keyboard() {
    let buttons = vec![
        MessageButton::callback("Yes", "approve:yes"),
        MessageButton::callback("No", "approve:no"),
    ];

    let keyboard = TelegramAdapter::build_keyboard(&buttons);
    assert!(keyboard.is_some());
}

#[test]
fn test_user_allowed() {
    let config = TelegramConfig::new("token").with_allowed_users(vec![123, 456]);
    let adapter = TelegramAdapter::new(config);

    assert!(adapter.is_user_allowed(123));
    assert!(adapter.is_user_allowed(456));
    assert!(!adapter.is_user_allowed(789));
}

#[test]
fn test_empty_allowlist_allows_all() {
    let config = TelegramConfig::new("token");
    let adapter = TelegramAdapter::new(config);

    assert!(adapter.is_user_allowed(123));
    assert!(adapter.is_user_allowed(999999));
}

#[test]
fn test_dm_policy_default() {
    let config = TelegramConfig::new("token");
    assert_eq!(config.dm_policy, DmPolicy::Allowlist);
}

#[test]
fn test_dm_policy_builder() {
    let config = TelegramConfig::new("token").with_dm_policy(DmPolicy::Open);
    assert_eq!(config.dm_policy, DmPolicy::Open);
}

#[test]
fn test_dm_policy_disabled() {
    let config = TelegramConfig::new("token").with_dm_policy(DmPolicy::Disabled);
    assert_eq!(config.dm_policy, DmPolicy::Disabled);
}

// Note: mask_for_logging and sanitize_error_for_user tests are in util.rs
