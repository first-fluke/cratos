
    use super::*;
    use tempfile::TempDir;

    fn create_test_chronicle(name: &str, level: u8) -> Chronicle {
        let mut chronicle = Chronicle::new(name);
        chronicle.level = level;
        chronicle.add_entry("Test task", Some("1"));
        chronicle
    }

    #[test]
    fn test_store_new() {
        let store = ChronicleStore::new();
        assert!(store.data_dir().ends_with("chronicles"));
    }

    #[test]
    fn test_store_with_path() {
        let store = ChronicleStore::with_path("/custom/path");
        assert_eq!(store.data_dir(), Path::new("/custom/path"));
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        let chronicle = create_test_chronicle("sindri", 1);
        let path = store.save(&chronicle).unwrap();

        assert!(path.exists());
        assert!(path.to_string_lossy().contains("sindri_lv1.json"));

        let loaded = store.load("sindri").unwrap();
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.persona_name, "sindri");
        assert_eq!(loaded.level, 1);
    }

    #[test]
    fn test_load_latest_level() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        // Save multiple levels
        store.save(&create_test_chronicle("athena", 1)).unwrap();
        store.save(&create_test_chronicle("athena", 2)).unwrap();
        store.save(&create_test_chronicle("athena", 3)).unwrap();

        let loaded = store.load("athena").unwrap().unwrap();
        assert_eq!(loaded.level, 3); // Latest level
    }

    #[test]
    fn test_load_specific_level() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        store.save(&create_test_chronicle("heimdall", 1)).unwrap();
        store.save(&create_test_chronicle("heimdall", 2)).unwrap();

        let loaded = store.load_level("heimdall", 1).unwrap().unwrap();
        assert_eq!(loaded.level, 1);
    }

    #[test]
    fn test_load_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        let loaded = store.load("nonexistent").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_load_all() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        store.save(&create_test_chronicle("sindri", 1)).unwrap();
        store.save(&create_test_chronicle("athena", 2)).unwrap();
        store.save(&create_test_chronicle("heimdall", 1)).unwrap();

        let all = store.load_all().unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_list_personas() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        store.save(&create_test_chronicle("sindri", 1)).unwrap();
        store.save(&create_test_chronicle("athena", 1)).unwrap();
        store.save(&create_test_chronicle("sindri", 2)).unwrap(); // duplicate

        let personas = store.list_personas().unwrap();
        assert_eq!(personas.len(), 2);
        assert!(personas.contains(&"sindri".to_string()));
        assert!(personas.contains(&"athena".to_string()));
    }

    #[test]
    fn test_delete() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        store.save(&create_test_chronicle("mimir", 1)).unwrap();
        assert!(store.exists("mimir"));

        let deleted = store.delete("mimir", 1).unwrap();
        assert!(deleted);
        assert!(!store.exists("mimir"));
    }

    #[test]
    fn test_exists() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        assert!(!store.exists("sindri"));

        store.save(&create_test_chronicle("sindri", 1)).unwrap();
        assert!(store.exists("sindri"));
        assert!(store.exists("SINDRI")); // case-insensitive
    }

    #[test]
    fn test_filename() {
        assert_eq!(ChronicleStore::filename("Sindri", 1), "sindri_lv1.json");
        assert_eq!(ChronicleStore::filename("ATHENA", 3), "athena_lv3.json");
    }
