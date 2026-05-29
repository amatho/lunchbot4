use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use serde::{Deserialize, Serialize};
use worker::wasm_bindgen::JsValue;
use worker::{Error, Fetch, Headers, Method, Request, RequestInit, Result, console_log};

const MODEL: &str = "gemini-2.5-flash-image";
const ENDPOINT: &str = "https://generativelanguage.googleapis.com/v1beta/models";

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

pub async fn generate_image(api_key: &str, day_no: &str, menu_html: &str) -> Result<Vec<u8>> {
    let prompt = format!(
        "Create a picture of today's menu (today is {day_no}) from the following menu, \
         which is in a very loosely structured HTML format in Norwegian. Use Norwegian \
         for any text in the image:\n{menu_html}"
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

    let mut text_pieces = Vec::new();
    for part in parts {
        if let Some(inline) = part.inline_data {
            console_log!("gemini returned image (mime {})", inline.mime_type);
            return B64
                .decode(inline.data)
                .map_err(|e| Error::RustError(format!("base64-decode Gemini image bytes: {e}")));
        }
        if let Some(text) = part.text {
            text_pieces.push(text);
        }
    }

    Err(Error::RustError(format!(
        "Gemini returned no image; text-only response: {}",
        text_pieces.join("\n")
    )))
}
