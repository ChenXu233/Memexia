use regex::Regex;
use std::collections::HashSet;

#[derive(Debug)]
pub struct ParsedDoc {
    pub title: Option<String>,
    pub links: HashSet<String>,
    pub tags: HashSet<String>,
}

pub fn parse_content(content: &str) -> ParsedDoc {
    let mut links = HashSet::new();
    let mut tags = HashSet::new();
    
    // Regex for [[wiki links]]
    // Matches [[link]] or [[link|alias]]
    let link_re = Regex::new(r"\[\[([^\]\|]+)(?:\|[^\]]+)?\]\]").unwrap();
    for cap in link_re.captures_iter(content) {
        if let Some(link) = cap.get(1) {
            links.insert(link.as_str().trim().to_string());
        }
    }

    // Regex for #tags
    let tag_re = Regex::new(r"(?:\s|^)#(\w+)").unwrap();
    for cap in tag_re.captures_iter(content) {
        if let Some(tag) = cap.get(1) {
            tags.insert(tag.as_str().to_string());
        }
    }

    // Simple title extraction (first H1)
    let title_re = Regex::new(r"(?m)^#\s+(.+)").unwrap();
    let title = title_re.captures(content)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().trim().to_string());

    ParsedDoc {
        title,
        links,
        tags,
    }
}
