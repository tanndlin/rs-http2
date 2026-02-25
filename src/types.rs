use std::fmt::Display;

pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum ContentType {
    HTML,
    Text,
    JavaScript,
    CSS,
    Unknown,
}

impl ContentType {
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "html" => ContentType::HTML,
            "txt" => ContentType::Text,
            "js" => ContentType::JavaScript,
            "css" => ContentType::CSS,
            _ => ContentType::Unknown,
        }
    }
}

impl From<ContentType> for String {
    fn from(val: ContentType) -> Self {
        val.to_string()
    }
}

impl Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let content_type_str = match self {
            ContentType::HTML => "text/html",
            ContentType::Text => "text/plain",
            ContentType::JavaScript => "application/javascript",
            ContentType::CSS => "text/css",
            ContentType::Unknown => "application/octet-stream",
        };
        write!(f, "{}", content_type_str)
    }
}
