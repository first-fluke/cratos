use super::*;
use crate::message::OutgoingAttachment;
use base64::Engine as _;

#[test]
fn test_slack_config() {
    let config = SlackConfig::new("xoxb-test", "xapp-test", "signing-secret")
        .with_allowed_channels(vec!["C123".to_string()])
        .with_mentions_only(false);

    assert_eq!(config.bot_token, "xoxb-test");
    assert_eq!(config.allowed_channels, vec!["C123".to_string()]);
    assert!(!config.mentions_only);
}

#[test]
fn test_channel_allowed() {
    let config = SlackConfig::new("xoxb-test", "xapp-test", "secret")
        .with_allowed_channels(vec!["C123".to_string(), "C456".to_string()]);
    let adapter = SlackAdapter::new(config);

    assert!(adapter.is_channel_allowed("C123"));
    assert!(adapter.is_channel_allowed("C456"));
    assert!(!adapter.is_channel_allowed("C789"));
}

#[test]
fn test_empty_allowlist_allows_all() {
    let config = SlackConfig::new("xoxb-test", "xapp-test", "secret");
    let adapter = SlackAdapter::new(config);

    assert!(adapter.is_channel_allowed("C123"));
    assert!(adapter.is_channel_allowed("ANY_CHANNEL"));
}

#[test]
fn test_workspace_allowed() {
    let config = SlackConfig::new("xoxb-test", "xapp-test", "secret")
        .with_allowed_workspaces(vec!["T123".to_string()]);
    let adapter = SlackAdapter::new(config);

    assert!(adapter.is_workspace_allowed("T123"));
    assert!(!adapter.is_workspace_allowed("T999"));
}

#[test]
fn test_dm_channel_detection() {
    // DM channels in Slack start with 'D'
    assert!("D1234567890".starts_with('D'));
    assert!(!"C1234567890".starts_with('D'));
}

#[test]
fn test_constant_time_eq() {
    assert!(constant_time_eq(b"hello", b"hello"));
    assert!(!constant_time_eq(b"hello", b"world"));
    assert!(!constant_time_eq(b"hello", b"hell"));
    assert!(!constant_time_eq(b"a", b"ab"));
}

#[test]
fn test_signature_verification() {
    // Test with known values
    let config = SlackConfig::new("xoxb-test", "xapp-test", "8f742231b10e8888abcd99yyyzzz85a5");
    let adapter = SlackAdapter::new(config);

    // Use current timestamp to avoid replay protection rejection
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let timestamp = now.to_string();
    let body = "token=xyzz0WbapA4vBCDEFasx0q6G&team_id=T1DC2JH3J&team_domain=testteamnow&channel_id=G8PSS9T3V&channel_name=foobar&user_id=U2CERLKJA&user_name=roadrunner&command=%2Fwebhook-collect&text=&response_url=https%3A%2F%2Fhooks.slack.com%2Fcommands%2FT1DC2JH3J%2F397700885554%2F96rGlfmibIGlgcZRskXaIFfN&trigger_id=398738663015.47445629121.803a0bc887a14d10d2c447fce8b6703c";

    // Compute expected signature manually
    let sig_basestring = format!("v0:{}:{}", timestamp, body);
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(b"8f742231b10e8888abcd99yyyzzz85a5").unwrap();
    mac.update(sig_basestring.as_bytes());
    let expected = mac.finalize().into_bytes();
    let signature = format!("v0={}", hex::encode(expected));

    // Should verify successfully with correct signature
    assert!(adapter
        .verify_signature(&timestamp, body, &signature)
        .is_ok());

    // Should fail with incorrect signature
    assert!(adapter
        .verify_signature(&timestamp, body, "v0=invalid")
        .is_err());
}

#[test]
fn test_signature_replay_protection() {
    let config = SlackConfig::new("xoxb-test", "xapp-test", "secret");
    let adapter = SlackAdapter::new(config);

    // Very old timestamp should be rejected
    let result = adapter.verify_signature("1000000000", "body", "v0=sig");
    assert!(result.is_err());
}

#[test]
fn test_outgoing_attachment_struct() {
    use crate::message::AttachmentType;

    let attachment = OutgoingAttachment {
        filename: "test.pdf".to_string(),
        mime_type: "application/pdf".to_string(),
        data: base64::engine::general_purpose::STANDARD.encode(b"test data"),
        attachment_type: AttachmentType::Document,
        caption: Some("Test file".to_string()),
    };

    assert_eq!(attachment.filename, "test.pdf");
    assert_eq!(attachment.mime_type, "application/pdf");
    assert!(attachment.caption.is_some());
    assert!(matches!(
        attachment.attachment_type,
        AttachmentType::Document
    ));
}

#[test]
fn test_base64_decode_for_attachment() {
    use base64::Engine as _;

    let original_data = b"Hello, World!";
    let encoded = base64::engine::general_purpose::STANDARD.encode(original_data);
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&encoded)
        .unwrap();

    assert_eq!(decoded, original_data);
}

#[test]
fn test_invalid_base64_attachment() {
    use base64::Engine as _;

    let result = base64::engine::general_purpose::STANDARD.decode("not-valid-base64!!!");
    assert!(result.is_err());
}

#[test]
fn test_slack_file_upload_url_format() {
    // Verify Slack API URL formats used in send_attachment
    let upload_url = "https://slack.com/api/files.getUploadURLExternal";
    let complete_url = "https://slack.com/api/files.completeUploadExternal";

    assert!(upload_url.contains("files.getUploadURLExternal"));
    assert!(complete_url.contains("files.completeUploadExternal"));
}
