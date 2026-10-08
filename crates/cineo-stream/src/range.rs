#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Span {
    Full,
    Partial { start: u64, end: u64 },
    Unsatisfiable,
}

pub(crate) fn span(header: Option<&str>, len: u64) -> Span {
    let Some(spec) = header.and_then(|h| h.trim().strip_prefix("bytes=")) else {
        return Span::Full;
    };
    if spec.contains(',') {
        return Span::Full;
    }
    let Some((first, last)) = spec.trim().split_once('-') else {
        return Span::Full;
    };
    let parse = |s: &str| s.trim().parse::<u64>().ok();
    let (start, end) = match (first.trim().is_empty(), last.trim().is_empty()) {
        (true, false) => match parse(last) {
            None => return Span::Full,
            Some(0) => return Span::Unsatisfiable,
            Some(n) => (len.saturating_sub(n), len.saturating_sub(1)),
        },
        (false, true) => match parse(first) {
            Some(start) => (start, len.saturating_sub(1)),
            None => return Span::Full,
        },
        (false, false) => match (parse(first), parse(last)) {
            (Some(start), Some(end)) if start <= end => (start, end.min(len.saturating_sub(1))),
            _ => return Span::Full,
        },
        (true, true) => return Span::Full,
    };
    if len == 0 || start >= len {
        Span::Unsatisfiable
    } else {
        Span::Partial { start, end }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_or_foreign_ranges_mean_the_whole_file() {
        assert_eq!(span(None, 100), Span::Full);
        assert_eq!(span(Some("items=0-1"), 100), Span::Full);
        assert_eq!(span(Some("bytes=0-1,5-6"), 100), Span::Full);
        assert_eq!(span(Some("bytes=abc"), 100), Span::Full);
        assert_eq!(span(Some("bytes=5-2"), 100), Span::Full);
        assert_eq!(span(Some("bytes=-"), 100), Span::Full);
    }

    #[test]
    fn open_and_closed_ranges_are_clamped_to_the_file() {
        assert_eq!(
            span(Some("bytes=0-"), 100),
            Span::Partial { start: 0, end: 99 }
        );
        assert_eq!(
            span(Some("bytes=10-19"), 100),
            Span::Partial { start: 10, end: 19 }
        );
        assert_eq!(
            span(Some("bytes=90-500"), 100),
            Span::Partial { start: 90, end: 99 }
        );
    }

    #[test]
    fn suffix_ranges_take_the_last_bytes() {
        assert_eq!(
            span(Some("bytes=-10"), 100),
            Span::Partial { start: 90, end: 99 }
        );
        assert_eq!(
            span(Some("bytes=-500"), 100),
            Span::Partial { start: 0, end: 99 }
        );
    }

    #[test]
    fn ranges_past_the_end_are_unsatisfiable() {
        assert_eq!(span(Some("bytes=100-"), 100), Span::Unsatisfiable);
        assert_eq!(span(Some("bytes=0-"), 0), Span::Unsatisfiable);
        assert_eq!(span(Some("bytes=-0"), 100), Span::Unsatisfiable);
    }
}
