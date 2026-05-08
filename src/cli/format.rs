pub fn fmt_hms(seconds: i64) -> String {
    let s = seconds.max(0);
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

pub fn fmt_hm(seconds: i64) -> String {
    let s = seconds.max(0);
    format!("{:>3}:{:02}", s / 3600, (s % 3600) / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hms_pads_correctly() {
        assert_eq!(fmt_hms(0), "00:00:00");
        assert_eq!(fmt_hms(3661), "01:01:01");
        assert_eq!(fmt_hms(-5), "00:00:00");
    }

    #[test]
    fn hm_aligns() {
        assert_eq!(fmt_hm(3600), "  1:00");
        assert_eq!(fmt_hm(45 * 3600 + 30 * 60), " 45:30");
    }
}
