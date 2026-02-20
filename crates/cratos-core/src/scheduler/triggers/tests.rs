
    use super::*;

    #[test]
    fn test_trigger_type_cron() {
        let trigger = TriggerType::cron("0 9 * * *");
        if let TriggerType::Cron(cron) = trigger {
            assert_eq!(cron.expression, "0 9 * * *");
            assert!(cron.timezone.is_none());
        } else {
            panic!("Expected Cron trigger");
        }
    }

    #[test]
    fn test_trigger_type_interval() {
        let trigger = TriggerType::interval(3600);
        if let TriggerType::Interval(interval) = trigger {
            assert_eq!(interval.seconds, 3600);
            assert!(!interval.immediate);
        } else {
            panic!("Expected Interval trigger");
        }
    }

    #[test]
    fn test_trigger_type_one_time() {
        let now = Utc::now();
        let trigger = TriggerType::one_time(now);
        if let TriggerType::OneTime(one_time) = trigger {
            assert_eq!(one_time.at, now);
        } else {
            panic!("Expected OneTime trigger");
        }
    }

    #[test]
    fn test_trigger_type_file() {
        let trigger = TriggerType::file("/tmp/test.txt");
        if let TriggerType::File(file) = trigger {
            assert_eq!(file.path, "/tmp/test.txt");
            assert_eq!(file.debounce_ms, 500);
        } else {
            panic!("Expected File trigger");
        }
    }

    #[test]
    fn test_trigger_type_cpu_threshold() {
        let trigger = TriggerType::cpu_threshold(80.0);
        if let TriggerType::System(system) = trigger {
            assert_eq!(system.metric, SystemMetric::CpuUsage);
            assert_eq!(system.threshold, 80.0);
            assert_eq!(system.comparison, Comparison::GreaterThan);
        } else {
            panic!("Expected System trigger");
        }
    }

    #[test]
    fn test_comparison_check() {
        assert!(Comparison::GreaterThan.check(90.0, 80.0));
        assert!(!Comparison::GreaterThan.check(70.0, 80.0));
        assert!(Comparison::LessThan.check(70.0, 80.0));
        assert!(!Comparison::LessThan.check(90.0, 80.0));
        assert!(Comparison::Equal.check(80.0, 80.0));
        assert!(!Comparison::Equal.check(81.0, 80.0));
    }

    #[test]
    fn test_trigger_serialization() {
        let trigger = TriggerType::cron("0 9 * * *");
        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("cron"));
        assert!(json.contains("0 9 * * *"));

        let deserialized: TriggerType = serde_json::from_str(&json).unwrap();
        if let TriggerType::Cron(cron) = deserialized {
            assert_eq!(cron.expression, "0 9 * * *");
        } else {
            panic!("Deserialization failed");
        }
    }
