#[cfg(test)]
mod tests {
    use crate::viewer::types::{truncate, ReplayOptions};

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn test_replay_options_builder() {
        let opts = ReplayOptions::dry_run()
            .with_range(Some(1), Some(10))
            .skip(vec!["exec".to_string()]);

        assert!(opts.dry_run);
        assert_eq!(opts.from_sequence, Some(1));
        assert_eq!(opts.to_sequence, Some(10));
        assert_eq!(opts.skip_tools, vec!["exec"]);
    }
}
