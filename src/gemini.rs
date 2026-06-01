use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use serde::{Deserialize, Serialize};
use worker::wasm_bindgen::JsValue;
use worker::{Date, Error, Fetch, Headers, Method, Request, RequestInit, Result, console_log};

const MODEL: &str = "gemini-2.5-flash-image";
const ENDPOINT: &str = "https://generativelanguage.googleapis.com/v1beta/models";

const STYLE_PROMPTS: &[&str] = &[
    "Vibrant overhead food photography of the dishes plated on a wooden table in natural daylight, realistic textures, shallow depth of field.",
    "Warm, painterly oil-painting depiction of the dishes arranged on rustic ceramic plates, soft brushstrokes, gallery-art feel.",
    "Clean studio food photography of each dish individually plated on white ceramic, soft shadows, food-magazine quality.",
    "Cheerful hand-drawn cartoon illustration of the dishes with bright flat colors, bold outlines, and a playful storybook vibe.",
    "Moody, dramatic close-up of the dishes against a dark background, rich textures, glossy highlights, fine-dining plating.",
];

#[derive(Serialize)]
struct GenRequest<'a> {
    contents: Vec<Content<'a>>,
}

#[derive(Serialize)]
struct Content<'a> {
    parts: Vec<Part<'a>>,
}

#[derive(Serialize)]
struct Part<'a> {
    text: &'a str,
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: CandidateContent,
}

#[derive(Deserialize)]
struct CandidateContent {
    #[serde(default)]
    parts: Vec<RespPart>,
}

#[derive(Deserialize)]
struct RespPart {
    #[serde(default)]
    text: Option<String>,
    #[serde(default, rename = "inlineData")]
    inline_data: Option<InlineData>,
}

#[derive(Deserialize)]
struct InlineData {
    #[serde(rename = "mimeType")]
    mime_type: String,
    data: String,
}

pub struct GeneratedImage {
    pub png: Vec<u8>,
    pub style_prompt: &'static str,
    pub menu_text: String,
}

pub async fn generate_image(
    api_key: &str,
    day_no: &str,
    menu_html: &str,
) -> Result<GeneratedImage> {
    let idx = (Date::now().as_millis() as usize) % STYLE_PROMPTS.len();
    let style = STYLE_PROMPTS[idx];
    let prompt = format!(
        "Today is {day_no}. The text below is a very loosely structured Norwegian HTML menu.\n\n\
         Today's menu generally consists of three parts: today's dish (\"Dagens\"), the \
         vegetarian option (\"Vegetar dagens\"), and a soup (\"Suppe\"). If a part is missing \
         from the source, omit it rather than inventing one.\n\n\
         Produce two outputs, in this exact order:\n\
         1. A clean Norwegian summary of today's dishes only, formatted as Slack mrkdwn. \
            For each part that is present, write the part name in bold on its own line \
            (`*Dagens*`, `*Vegetar dagens*`, `*Suppe*`) followed by a bullet line starting \
            with `• ` describing the dish. No headers, no commentary, no surrounding prose. \
            Fix obvious typos.\n\
         2. An illustrative image depicting ONLY the food of today's menu. The image must \
            contain no text, no labels, no menu boards, no chalkboards, and no signage of any \
            kind — pure food imagery. Style: {style}\n\n\
         Menu HTML:\n{menu_html}"
    );

    let url = format!("{ENDPOINT}/{MODEL}:generateContent?key={api_key}");
    let req = GenRequest {
        contents: vec![Content {
            parts: vec![Part { text: &prompt }],
        }],
    };
    let json_body = serde_json::to_string(&req)
        .map_err(|e| Error::RustError(format!("serialize Gemini request: {e}")))?;

    let headers = Headers::new();
    headers.set("Content-Type", "application/json")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(&json_body)));
    let request = Request::new_with_init(&url, &init)?;

    let mut resp = Fetch::Request(request).send().await?;
    let status = resp.status_code();
    let body = resp.text().await?;
    if !(200..300).contains(&status) {
        return Err(Error::RustError(format!(
            "Gemini returned {status}: {body}"
        )));
    }

    let parsed: Response = serde_json::from_str(&body)
        .map_err(|e| Error::RustError(format!("parse Gemini JSON response: {e}")))?;

    let parts = parsed
        .candidates
        .into_iter()
        .next()
        .ok_or_else(|| Error::RustError(format!("Gemini response had no candidates: {body}")))?
        .content
        .parts;

    let mut text_pieces: Vec<String> = Vec::new();
    let mut png: Option<Vec<u8>> = None;
    for part in parts {
        if let Some(text) = part.text {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                text_pieces.push(trimmed.to_owned());
            }
        }
        if let Some(inline) = part.inline_data {
            console_log!("gemini returned image (mime {})", inline.mime_type);
            png =
                Some(B64.decode(inline.data).map_err(|e| {
                    Error::RustError(format!("base64-decode Gemini image bytes: {e}"))
                })?);
        }
    }

    let menu_text = text_pieces.join("\n");
    let png = png.ok_or_else(|| {
        Error::RustError(format!(
            "Gemini returned no image; text-only response: {menu_text}"
        ))
    })?;

    Ok(GeneratedImage {
        png,
        style_prompt: style,
        menu_text,
    })
}
