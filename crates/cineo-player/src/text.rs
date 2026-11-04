pub fn plain_text(text: &str) -> String {
    text.replace(['\n', '\r'], " ").chars().take(60).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_text_is_one_short_line() {
        assert_eq!(plain_text("eng\r\nSDH"), "eng  SDH");
        assert_eq!(plain_text(&"é".repeat(80)).chars().count(), 60);
    }
}
