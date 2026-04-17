/// Formats a byte count into a human-readable string (e.g. "1.5 GB").
/// Uses the largest unit where the value is >= 1.0.
pub fn human_readable_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let bytes = bytes as f64;
    let unit_index = (bytes.log10() / 1024f64.log10()).min(UNITS.len() as f64 - 1.0) as usize;
    let converted = bytes / 1024f64.powi(unit_index as i32);

    format!("{:.1} {}", converted, UNITS[unit_index])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero() {
        assert_eq!(human_readable_size(0), "0 B");
    }

    #[test]
    fn test_kilobytes() {
        assert_eq!(human_readable_size(1024), "1.0 KB");
    }

    #[test]
    fn test_megabytes() {
        assert_eq!(human_readable_size(1_048_576), "1.0 MB");
    }

    #[test]
    fn test_gigabytes() {
        assert_eq!(human_readable_size(1_073_741_824), "1.0 GB");
    }

    #[test]
    fn test_terabytes() {
        assert_eq!(human_readable_size(1_099_511_627_776), "1.0 TB");
    }

    #[test]
    fn test_fractional() {
        let result = human_readable_size(1_536 * 1024);
        assert!(result.contains("1.5") && result.contains("MB"));
    }
}
