
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("active_persona");
        let state = ActivePersonaState::with_path(&state_file);

        // Initially empty
        assert_eq!(state.load().unwrap(), None);

        // Save
        state.save("sindri").unwrap();
        assert_eq!(state.load().unwrap(), Some("sindri".to_string()));

        // Overwrite
        state.save("athena").unwrap();
        assert_eq!(state.load().unwrap(), Some("athena".to_string()));
    }

    #[test]
    fn test_clear() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("active_persona");
        let state = ActivePersonaState::with_path(&state_file);

        state.save("sindri").unwrap();
        assert!(state.load().unwrap().is_some());

        state.clear().unwrap();
        assert_eq!(state.load().unwrap(), None);
    }

    #[test]
    fn test_clear_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("active_persona");
        let state = ActivePersonaState::with_path(&state_file);

        // Should not error
        state.clear().unwrap();
    }

    #[test]
    fn test_save_lowercases() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("active_persona");
        let state = ActivePersonaState::with_path(&state_file);

        state.save("SINDRI").unwrap();
        assert_eq!(state.load().unwrap(), Some("sindri".to_string()));
    }
