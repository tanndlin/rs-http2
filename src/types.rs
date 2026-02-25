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
