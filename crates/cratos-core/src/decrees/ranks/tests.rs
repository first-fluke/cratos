
    use super::*;

    fn create_test_ranks() -> Ranks {
        Ranks {
            meta: RanksMeta {
                title: "Rank System".to_string(),
                motto: "Prove with achievements".to_string(),
            },
            ranks: vec![
                Rank {
                    level: RankLevel::Range { min: 1, max: 2 },
                    title: "Mortal".to_string(),
                    title_kr: "Human".to_string(),
                    requirements: vec!["Self-management".to_string()],
                    permissions: vec![],
                },
                Rank {
                    level: RankLevel::Single(3),
                    title: "Demigod".to_string(),
                    title_kr: "Demigod".to_string(),
                    requirements: vec!["Peer support".to_string()],
                    permissions: vec![],
                },
                Rank {
                    level: RankLevel::Single(255),
                    title: "Supreme".to_string(),
                    title_kr: "Transcendent".to_string(),
                    requirements: vec!["God Slayer".to_string()],
                    permissions: vec!["Absolute authority".to_string()],
                },
            ],
            promotion: PromotionRules {
                formula: "(Current Level + 1) × 5".to_string(),
                additional: vec![],
            },
        }
    }

    #[test]
    fn test_rank_level_contains() {
        let range = RankLevel::Range { min: 1, max: 2 };
        assert!(range.contains(1));
        assert!(range.contains(2));
        assert!(!range.contains(3));

        let single = RankLevel::Single(5);
        assert!(single.contains(5));
        assert!(!single.contains(4));
    }

    #[test]
    fn test_rank_level_display() {
        assert_eq!(RankLevel::Single(3).display(), "Lv3");
        assert_eq!(RankLevel::Single(255).display(), "∞");
        assert_eq!(RankLevel::Range { min: 1, max: 2 }.display(), "Lv1~2");
    }

    #[test]
    fn test_get_rank_for_level() {
        let ranks = create_test_ranks();

        let mortal = ranks.get_rank_for_level(1);
        assert!(mortal.is_some());
        assert_eq!(mortal.unwrap().title, "Mortal");

        let demigod = ranks.get_rank_for_level(3);
        assert!(demigod.is_some());
        assert_eq!(demigod.unwrap().title, "Demigod");

        let supreme = ranks.get_rank_for_level(255);
        assert!(supreme.is_some());
        assert_eq!(supreme.unwrap().title, "Supreme");
    }

    #[test]
    fn test_format_display() {
        let ranks = create_test_ranks();
        let output = ranks.format_display();

        assert!(output.contains("# Rank System"));
        assert!(output.contains("**Mortal**"));
        assert!(output.contains("Lv1~2"));
    }
