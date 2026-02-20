
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_laws_toml() -> &'static str {
        r#"
[meta]
title = "Laws (LAWS)"
philosophy = "Test philosophy"
immutable = true

[[articles]]
id = 1
title = "Article 1"
rules = ["Rule 1", "Rule 2"]

[[articles]]
id = 2
title = "Article 2"
rules = ["Rule A"]
"#
    }

    #[test]
    fn test_loader_new() {
        let loader = DecreeLoader::new();
        assert!(loader.config_dir().ends_with("decrees"));
    }

    #[test]
    fn test_loader_with_path() {
        let loader = DecreeLoader::with_path("/custom/path");
        assert_eq!(loader.config_dir(), Path::new("/custom/path"));
    }

    #[test]
    fn test_load_laws() {
        let temp_dir = TempDir::new().unwrap();
        let loader = DecreeLoader::with_path(temp_dir.path());

        // When file doesn't exist
        assert!(loader.load_laws().is_err());

        // Create file
        fs::write(temp_dir.path().join("laws.toml"), create_test_laws_toml()).unwrap();

        let laws = loader.load_laws().unwrap();
        assert_eq!(laws.article_count(), 2);
        assert_eq!(laws.meta.title, "Laws (LAWS)");
    }

    #[test]
    fn test_exists_methods() {
        let temp_dir = TempDir::new().unwrap();
        let loader = DecreeLoader::with_path(temp_dir.path());

        assert!(!loader.laws_exists());
        assert!(!loader.ranks_exists());
        assert!(!loader.warfare_exists());

        fs::write(temp_dir.path().join("laws.toml"), "").unwrap();
        assert!(loader.laws_exists());
    }

    #[test]
    fn test_validate_all() {
        let temp_dir = TempDir::new().unwrap();
        let loader = DecreeLoader::with_path(temp_dir.path());

        // When file doesn't exist
        let result = loader.validate_all();
        assert!(!result.all_valid());
        assert!(result.laws_count.is_none());

        // Create file
        fs::write(temp_dir.path().join("laws.toml"), create_test_laws_toml()).unwrap();

        let result = loader.validate_all();
        assert_eq!(result.laws_count, Some(2));
        assert!(!result.laws_valid); // less than 10
    }

    #[test]
    fn test_validation_result() {
        let mut result = ValidationResult::default();

        assert!(!result.all_valid());
        assert!(!result.required_valid());

        result.laws_valid = true;
        assert!(result.required_valid());
        assert!(!result.all_valid());

        result.ranks_valid = true;
        result.warfare_valid = true;
        assert!(result.all_valid());
    }
