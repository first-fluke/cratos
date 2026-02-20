
    use super::*;

    fn create_test_laws() -> Laws {
        Laws {
            meta: LawsMeta {
                title: "Laws (LAWS)".to_string(),
                philosophy: "Test philosophy".to_string(),
                immutable: true,
            },
            articles: vec![
                Article {
                    id: 1,
                    title: "Planning and Design".to_string(),
                    rules: vec!["Rule 1".to_string(), "Rule 2".to_string()],
                },
                Article {
                    id: 2,
                    title: "Development Guidelines".to_string(),
                    rules: vec!["Rule A".to_string()],
                },
            ],
        }
    }

    #[test]
    fn test_article_count() {
        let laws = create_test_laws();
        assert_eq!(laws.article_count(), 2);
    }

    #[test]
    fn test_get_article() {
        let laws = create_test_laws();

        let article = laws.get_article(1);
        assert!(article.is_some());
        assert_eq!(article.unwrap().title, "Planning and Design");

        assert!(laws.get_article(99).is_none());
    }

    #[test]
    fn test_is_valid() {
        let laws = create_test_laws();
        assert!(!laws.is_valid()); // Only 2 articles
    }

    #[test]
    fn test_article_reference() {
        let article = Article {
            id: 2,
            title: "Development Guidelines".to_string(),
            rules: vec![],
        };
        assert_eq!(article.reference(), "Article 2");
    }

    #[test]
    fn test_format_display() {
        let laws = create_test_laws();
        let output = laws.format_display();

        assert!(output.contains("# Laws (LAWS)"));
        assert!(output.contains("Test philosophy"));
        assert!(output.contains("## Article 1 (Planning and Design)"));
        assert!(output.contains("- Rule 1"));
    }
