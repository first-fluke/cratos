    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_toml() -> &'static str {
        r#"
[persona]
name = "TestPersona"
title = "Test Title"
domain = "DEV"

[traits]
core = "Test core trait"
philosophy = "Test philosophy"
communication_style = ["clarity", "conciseness"]

[principles]
1 = "First principle"

[skills]
default = ["skill1"]

[level]
level = 1
title = "Mortal"
"#
    }

    #[test]
    fn test_loader_new() {
        let loader = PersonaLoader::new();
        assert_eq!(loader.config_dir(), Path::new(DEFAULT_PANTHEON_DIR));
    }

    #[test]
    fn test_loader_with_path() {
        let loader = PersonaLoader::with_path("/custom/path");
        assert_eq!(loader.config_dir(), Path::new("/custom/path"));
    }

    #[test]
    fn test_loader_nonexistent_dir() {
        let loader = PersonaLoader::with_path("/nonexistent/path");
        let result = loader.load_all().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_loader_load_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.toml");

        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(create_test_toml().as_bytes()).unwrap();

        let loader = PersonaLoader::with_path(temp_dir.path());
        let preset = loader.load("test").unwrap();

        assert_eq!(preset.persona.name, "TestPersona");
        assert_eq!(preset.persona.title, "Test Title");
        assert_eq!(preset.level.level, 1);
    }

    #[test]
    fn test_loader_load_all() {
        let temp_dir = TempDir::new().unwrap();

        // Create two test files
        for name in ["alpha", "beta"] {
            let file_path = temp_dir.path().join(format!("{name}.toml"));
            let content = create_test_toml().replace("TestPersona", &format!("{name}Persona"));
            std::fs::write(file_path, content).unwrap();
        }

        let loader = PersonaLoader::with_path(temp_dir.path());
        let presets = loader.load_all().unwrap();

        assert_eq!(presets.len(), 2);
    }

    #[test]
    fn test_loader_exists() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("exists.toml");
        std::fs::write(file_path, create_test_toml()).unwrap();

        let loader = PersonaLoader::with_path(temp_dir.path());

        assert!(loader.exists("exists"));
        assert!(loader.exists("EXISTS")); // case-insensitive
        assert!(!loader.exists("notexists"));
    }

    #[test]
    fn test_loader_list_names() {
        let temp_dir = TempDir::new().unwrap();

        for name in ["alpha", "beta", "gamma"] {
            let file_path = temp_dir.path().join(format!("{name}.toml"));
            std::fs::write(file_path, create_test_toml()).unwrap();
        }

        let loader = PersonaLoader::with_path(temp_dir.path());
        let names = loader.list_names().unwrap();

        assert_eq!(names.len(), 3);
        assert!(names.contains(&"alpha".to_string()));
        assert!(names.contains(&"beta".to_string()));
        assert!(names.contains(&"gamma".to_string()));
    }

    #[test]
    fn test_loader_load_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let loader = PersonaLoader::with_path(temp_dir.path());

        let result = loader.load("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_loader_invalid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("invalid.toml");
        std::fs::write(file_path, "invalid toml content {{{").unwrap();

        let loader = PersonaLoader::with_path(temp_dir.path());
        let result = loader.load("invalid");

        assert!(result.is_err());
    }
