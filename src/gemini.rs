use anyhow::{Context, Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use serde::{Deserialize, Serialize};

const MODEL: &str = "gemini-3.1-flash-image-preview";
const ENDPOINT: &str = "https://generativelanguage.googleapis.com/v1beta/models";

#[derive(Serialize)]
struct Request<'a> {
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
    let req = Request {
        contents: vec![Content {
            parts: vec![Part { text: &prompt }],
        }],
    };

    let resp = reqwest::Client::new()
        .post(&url)
        .json(&req)
        .send()
        .await
        .context("POST to Gemini failed")?;

    let status = resp.status();
    let body = resp.text().await.context("read Gemini response body")?;
    if !status.is_success() {
        return Err(anyhow!("Gemini returned {status}: {body}"));
    }

    let parsed: Response =
        serde_json::from_str(&body).context("parse Gemini JSON response")?;

    let parts = parsed
        .candidates
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("Gemini response had no candidates: {body}"))?
        .content
        .parts;

    let mut text_pieces = Vec::new();
    for part in parts {
        if let Some(inline) = part.inline_data {
            tracing::info!(mime = %inline.mime_type, "gemini returned image");
            return B64
                .decode(inline.data)
                .context("base64-decode Gemini image bytes");
        }
        if let Some(text) = part.text {
            text_pieces.push(text);
        }
    }

    Err(anyhow!(
        "Gemini returned no image; text-only response: {}",
        text_pieces.join("\n")
    ))
}
