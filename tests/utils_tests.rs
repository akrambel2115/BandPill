use bandpill::utils::{format_speed, format_usage, to_wide_chars};

#[test]
fn test_format_speed_scales() {
    assert_eq!(format_speed(0.0), "0 B/s");
    assert_eq!(format_speed(512.0), "512 B/s");
    assert_eq!(format_speed(1023.0), "1023 B/s");
    assert_eq!(format_speed(1024.0), "1.0 KB/s");
    assert_eq!(format_speed(1536.0), "1.5 KB/s");
    assert_eq!(format_speed(1024.0 * 1024.0), "1.0 MB/s");
    assert_eq!(format_speed(4.8 * 1024.0 * 1024.0), "4.8 MB/s");
}

#[test]
fn test_format_usage_mb_mode() {
    let (num, unit, full) = format_usage(230.4, "MB");
    assert_eq!(num, "230");
    assert_eq!(unit, "MB");
    assert_eq!(full, "230 MB");

    let (num, unit, full) = format_usage(1500.0, "MB");
    assert_eq!(num, "1500");
    assert_eq!(unit, "MB");
    assert_eq!(full, "1500 MB");
}

#[test]
fn test_format_usage_compact_mode() {
    let (num, unit, full) = format_usage(230.0, "MB_COMPACT");
    assert_eq!(num, "230");
    assert_eq!(unit, "MB");
    assert_eq!(full, "230MB");
}

#[test]
fn test_format_usage_auto_mode() {
    // Under 1024 MB -> format as MB
    let (num, unit, full) = format_usage(850.2, "AUTO");
    assert_eq!(num, "850");
    assert_eq!(unit, "MB");
    assert_eq!(full, "850 MB");

    // Over 1024 MB (< 10 GB) -> format as X.X GB
    let (num, unit, full) = format_usage(1536.0, "AUTO");
    assert_eq!(num, "1.5");
    assert_eq!(unit, "GB");
    assert_eq!(full, "1.5 GB");

    // Over 10 GB -> format as rounded integer GB
    let (num, unit, full) = format_usage(12.4 * 1024.0, "AUTO");
    assert_eq!(num, "12");
    assert_eq!(unit, "GB");
    assert_eq!(full, "12 GB");
}

#[test]
fn test_to_wide_chars_null_termination() {
    let wide = to_wide_chars("Hello");
    assert_eq!(wide.len(), 6);
    assert_eq!(wide[5], 0);
    assert_eq!(wide[0], 'H' as u16);
    assert_eq!(wide[4], 'o' as u16);
}
