/// Convert project.toml `query_timeout` (milliseconds) to preference seconds.
pub fn query_timeout_ms_to_secs(ms: u64) -> u32 {
    ((ms / 1000).max(1)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_project_timeout_ms_to_secs() {
        assert_eq!(query_timeout_ms_to_secs(30_000), 30);
        assert_eq!(query_timeout_ms_to_secs(500), 1);
    }
}
