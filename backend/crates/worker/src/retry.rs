pub(crate) fn retry_delay_seconds(base_seconds: u64, max_seconds: u64, attempt: i32) -> u64 {
    if attempt <= 1 {
        return base_seconds.min(max_seconds);
    }

    let exponent = u32::try_from(attempt.saturating_sub(1)).unwrap_or(u32::MAX);
    let capped_exponent = exponent.min(20);
    let multiplier = 1_u64 << capped_exponent;

    base_seconds.saturating_mul(multiplier).min(max_seconds)
}

#[cfg(test)]
mod tests {
    use super::retry_delay_seconds;

    #[test]
    fn retry_backoff_is_exponential_and_capped() {
        assert_eq!(retry_delay_seconds(30, 900, 1), 30);
        assert_eq!(retry_delay_seconds(30, 900, 2), 60);
        assert_eq!(retry_delay_seconds(30, 900, 3), 120);
        assert_eq!(retry_delay_seconds(30, 900, 10), 900);
    }
}
