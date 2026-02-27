use std::fmt::Display;

#[derive(PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum ImageType {
    PNG,
    JPEG,
    GIF,
    Icon,
}

#[derive(PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum ContentType {
    HTML,
    Text,
    JavaScript,
    CSS,
    JSON,
    Image(ImageType),
    Unknown,
}

impl ContentType {
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "html" => ContentType::HTML,
            "txt" => ContentType::Text,
            "js" => ContentType::JavaScript,
            "css" => ContentType::CSS,
            "json" => ContentType::JSON,
            "png" => ContentType::Image(ImageType::PNG),
            "jpg" | "jpeg" => ContentType::Image(ImageType::JPEG),
            "ico" => ContentType::Image(ImageType::Icon),
            "gif" => ContentType::Image(ImageType::GIF),
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
            ContentType::JSON => "application/json",
            ContentType::Image(ImageType::PNG) => "image/png",
            ContentType::Image(ImageType::JPEG) => "image/jpeg",
            ContentType::Image(ImageType::GIF) => "image/gif",
            ContentType::Image(ImageType::Icon) => "image/x-icon",
            ContentType::Unknown => "application/octet-stream",
        };
        write!(f, "{}", content_type_str)
    }
}
