
    use super::*;

    #[test]
    fn test_generate_keypair_and_verify() {
        let (signing_key, verifying_key) = generate_device_keypair();
        let challenge = generate_challenge();
        let signature = sign_challenge(&signing_key, &challenge);

        let result = verify_signature(verifying_key.as_bytes(), &challenge, &signature);
        assert!(result.is_ok());
    }

    #[test]
    fn test_wrong_signature_fails() {
        let (signing_key, _) = generate_device_keypair();
        let (_, other_verifying_key) = generate_device_keypair();

        let challenge = generate_challenge();
        let signature = sign_challenge(&signing_key, &challenge);

        // Wrong public key
        let result = verify_signature(other_verifying_key.as_bytes(), &challenge, &signature);
        assert_eq!(result, Err(DeviceAuthError::VerificationFailed));
    }

    #[test]
    fn test_wrong_message_fails() {
        let (signing_key, verifying_key) = generate_device_keypair();
        let challenge = generate_challenge();
        let signature = sign_challenge(&signing_key, &challenge);

        // Different message
        let other_challenge = generate_challenge();
        let result = verify_signature(verifying_key.as_bytes(), &other_challenge, &signature);
        assert_eq!(result, Err(DeviceAuthError::VerificationFailed));
    }

    #[test]
    fn test_invalid_public_key() {
        let result = verify_signature(&[0u8; 16], &[0u8; 32], &[0u8; 64]);
        assert_eq!(result, Err(DeviceAuthError::InvalidPublicKey));
    }

    #[test]
    fn test_invalid_signature_length() {
        let (_, verifying_key) = generate_device_keypair();
        let result = verify_signature(
            verifying_key.as_bytes(),
            &[0u8; 32],
            &[0u8; 32], // Wrong length
        );
        assert_eq!(result, Err(DeviceAuthError::InvalidSignature));
    }

    #[tokio::test]
    async fn test_challenge_store_issue_and_verify() {
        let store = ChallengeStore::new();
        let challenge = store.issue("device-1").await;

        let result = store.verify("device-1", &challenge).await;
        assert!(result.is_ok());

        // Second verify should fail (one-time use)
        let result = store.verify("device-1", &challenge).await;
        assert_eq!(result, Err(DeviceAuthError::ChallengeNotFound));
    }

    #[tokio::test]
    async fn test_challenge_store_expired() {
        let store = ChallengeStore::with_ttl_secs(0); // Immediate expiry
        let challenge = store.issue("device-1").await;

        // Sleep to ensure expiry
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let result = store.verify("device-1", &challenge).await;
        assert_eq!(result, Err(DeviceAuthError::ChallengeExpired));
    }

    #[tokio::test]
    async fn test_challenge_store_wrong_device() {
        let store = ChallengeStore::new();
        let challenge = store.issue("device-1").await;

        let result = store.verify("device-2", &challenge).await;
        assert_eq!(result, Err(DeviceAuthError::ChallengeNotFound));
    }

    #[tokio::test]
    async fn test_challenge_store_cleanup() {
        let store = ChallengeStore::with_ttl_secs(0);
        store.issue("d1").await;
        store.issue("d2").await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let cleaned = store.cleanup().await;
        assert_eq!(cleaned, 2);
    }

    #[test]
    fn test_full_challenge_response_flow() {
        // Simulate the full Happy Coder pattern:
        // 1. Node generates keypair
        let (node_signing_key, node_verifying_key) = generate_device_keypair();

        // 2. Server generates challenge
        let challenge = generate_challenge();

        // 3. Node signs challenge
        let signature = sign_challenge(&node_signing_key, &challenge);

        // 4. Server verifies with stored public key
        let result = verify_signature(node_verifying_key.as_bytes(), &challenge, &signature);
        assert!(result.is_ok());
    }
