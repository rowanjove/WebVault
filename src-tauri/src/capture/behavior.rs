pub struct BrowserBehavior;

impl BrowserBehavior {
    pub fn autoscroll_script() -> &'static str {
        r#"
        (async () => {
            const totalHeight = document.body.scrollHeight;
            const step = window.innerHeight * 0.8;
            let current = 0;
            while (current < totalHeight && current < 20000) {
                window.scrollBy(0, step);
                current += step;
                await new Promise(r => setTimeout(r, 200));
            }
            window.scrollTo(0, 0);
        })()
        "#
    }
}

pub struct TextExtractor;

impl TextExtractor {
    pub fn extract_title(html: &str) -> Option<String> {
        let title_regex = regex::Regex::new(r"(?i)<title[^>]*>([^<]+)</title>").ok()?;
        title_regex.captures(html).and_then(|c| c.get(1)).map(|m| m.as_str().trim().to_string())
    }

    pub fn extract_clean_text(html: &str) -> String {
        let script_regex = match regex::Regex::new(r"(?is)<script[^>]*>.*?</script>") {
            Ok(r) => r,
            Err(_) => return html.to_string(),
        };
        let style_regex = match regex::Regex::new(r"(?is)<style[^>]*>.*?</style>") {
            Ok(r) => r,
            Err(_) => return html.to_string(),
        };
        let without_script = script_regex.replace_all(html, " ");
        let stripped = style_regex.replace_all(&without_script, " ");
        let html_tag_regex = match regex::Regex::new(r"<[^>]+>") {
            Ok(r) => r,
            Err(_) => return stripped.to_string(),
        };
        let text = html_tag_regex.replace_all(&stripped, " ");
        let space_regex = match regex::Regex::new(r"\s+") {
            Ok(r) => r,
            Err(_) => return text.to_string(),
        };
        space_regex.replace_all(&text, " ").trim().to_string()
    }
}

