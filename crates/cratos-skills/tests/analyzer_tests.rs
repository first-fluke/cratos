use cratos_skills::analyzer::{AnalyzerConfig, PatternAnalyzer, PatternStatus};

#[test]
fn test_is_stop_word() {
    let analyzer = PatternAnalyzer::default();
    assert!(analyzer.is_stop_word("the"));
    assert!(analyzer.is_stop_word("The"));
    assert!(analyzer.is_stop_word("a"));
    assert!(!analyzer.is_stop_word("file"));
    assert!(!analyzer.is_stop_word("read"));
}

#[test]
fn test_find_frequent_ngrams() {
    let analyzer = PatternAnalyzer::with_config(AnalyzerConfig {
        min_occurrences: 2,
        ..Default::default()
    });

    let sequences = vec![
        vec!["file_read".to_string(), "git_commit".to_string()],
        vec!["file_read".to_string(), "git_commit".to_string()],
        vec!["file_read".to_string(), "git_commit".to_string()],
        vec!["exec".to_string()],
    ];

    let ngrams = analyzer.find_frequent_ngrams(&sequences, 2);

    assert_eq!(ngrams.len(), 1);
    assert_eq!(
        ngrams.get(&vec!["file_read".to_string(), "git_commit".to_string()]),
        Some(&3)
    );
}

#[test]
fn test_analyzer_config_default() {
    let config = AnalyzerConfig::default();
    assert_eq!(config.min_occurrences, 3);
    assert_eq!(config.min_confidence, 0.6);
    assert_eq!(config.max_sequence_length, 5);
    assert_eq!(config.analysis_window_days, 30);
}

#[test]
fn test_pattern_status() {
    assert_eq!(PatternStatus::Detected.as_str(), "detected");
    assert_eq!(PatternStatus::Converted.as_str(), "converted");

    let parsed: PatternStatus = "detected".parse().unwrap();
    assert_eq!(parsed, PatternStatus::Detected);
}
