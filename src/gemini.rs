use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use serde::{Deserialize, Serialize};
use worker::wasm_bindgen::JsValue;
use worker::{Delay, Error, Fetch, Headers, Method, Request, RequestInit, Result, console_log};

const GATEWAY_BASE: &str = "https://gateway.ai.cloudflare.com/v1";
const TEXT_MODEL: &str = "gemini-2.5-flash";
const IMAGE_MODEL: &str = "gemini-2.5-flash-image";

const MAX_RETRIES: u32 = 10;
const BASE_BACKOFF: Duration = Duration::from_secs(2);

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
    #[serde(rename = "generationConfig", skip_serializing_if = "Option::is_none")]
    generation_config: Option<GenerationConfig>,
}

#[derive(Serialize)]
struct GenerationConfig {
    #[serde(rename = "responseModalities")]
    response_modalities: Vec<&'static str>,
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

#[derive(Clone, Copy)]
pub struct Gemini<'a> {
    pub api_key: &'a str,
    pub cf_account_id: &'a str,
    pub cf_ai_gateway_token: &'a str,
}

impl Gemini<'_> {
    pub async fn generate_image(&self, day_no: &str, menu_html: &str) -> Result<GeneratedImage> {
        let menu_text = self.generate_menu_text(day_no, menu_html).await?;
        console_log!("gemini returned menu text ({} chars)", menu_text.len());

        let idx = rand::random_range(0..STYLE_PROMPTS.len());
        let style = STYLE_PROMPTS[idx];
        let png = self.generate_menu_image(&menu_text, style).await?;

        Ok(GeneratedImage {
            png,
            style_prompt: style,
            menu_text,
        })
    }

    /// Calls `gemini-2.5-flash` to produce the Slack-formatted Norwegian menu summary.
    async fn generate_menu_text(&self, day_no: &str, menu_html: &str) -> Result<String> {
        let prompt = format!(
            "Today is {day_no}. The text below is a very loosely structured Norwegian HTML menu.\n\n\
            Today's menu generally consists of three parts: today's dish (\"Dagens\"), the \
            vegetarian option (\"Vegetar dagens\"), and a soup (\"Suppe\"). If a part is missing \
            from the source, omit it rather than inventing one.\n\n\
            Produce a clean Norwegian summary of today's dishes only, formatted as Slack mrkdwn. \
            For each part that is present, write the part name in bold on its own line \
            (`*Dagens*`, `*Vegetar dagens*`, `*Suppe*`) followed by a bullet line starting \
            with `• ` describing the dish. No headers, no commentary, no surrounding prose. \
            Fix obvious typos.\n\n\
            Menu HTML:\n{menu_html}"
        );

        let request = self.build_request(TEXT_MODEL, &prompt, None)?;
        post_with_retry(&request, |status, body| {
            if is_transient(status) {
                return Outcome::Retry;
            }
            if !(200..300).contains(&status) {
                return Outcome::Fatal(format!("Gemini returned {status}: {body}"));
            }

            let parts = match first_candidate_parts(body) {
                Ok(parts) => parts,
                Err(e) => return Outcome::Fatal(e.to_string()),
            };
            let menu_text = parts
                .into_iter()
                .filter_map(|p| p.text)
                .map(|t| t.trim().to_owned())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join("\n");

            if menu_text.is_empty() {
                Outcome::Fatal(format!("Gemini returned no menu text: {body}"))
            } else {
                Outcome::Done(menu_text)
            }
        })
        .await
    }

    /// Calls `gemini-2.5-flash-image` to illustrate the menu. A text-only response
    /// (no image) is treated as a transient failure and retried via the shared
    /// retry loop, so we keep trying until we actually get image bytes back.
    async fn generate_menu_image(&self, menu_text: &str, style: &str) -> Result<Vec<u8>> {
        let prompt = format!(
            "Create an illustrative image depicting ONLY the food of today's lunch menu. \
            The image must contain no text, no labels, no menu boards, no chalkboards, and no \
            signage of any kind — pure food imagery. Style: {style}\n\n\
            Today's menu:\n{menu_text}"
        );

        let request = self.build_request(
            IMAGE_MODEL,
            &prompt,
            Some(GenerationConfig {
                response_modalities: vec!["TEXT", "IMAGE"],
            }),
        )?;

        post_with_retry(&request, |status, body| {
            if is_transient(status) {
                return Outcome::Retry;
            }
            if !(200..300).contains(&status) {
                return Outcome::Fatal(format!("Gemini returned {status}: {body}"));
            }

            let parts = match first_candidate_parts(body) {
                Ok(parts) => parts,
                Err(e) => return Outcome::Fatal(e.to_string()),
            };
            for part in parts {
                if let Some(inline) = part.inline_data {
                    console_log!("gemini returned image (mime {})", inline.mime_type);
                    return match B64.decode(inline.data) {
                        Ok(bytes) => Outcome::Done(bytes),
                        Err(e) => Outcome::Fatal(format!("base64-decode Gemini image bytes: {e}")),
                    };
                }
            }

            // 2xx but only text, retry to actually get an image.
            Outcome::Retry
        })
        .await
    }

    fn build_request(
        &self,
        model: &str,
        prompt: &str,
        generation_config: Option<GenerationConfig>,
    ) -> Result<Request> {
        let url = format!(
            "{GATEWAY_BASE}/{}/default/google-ai-studio/v1beta/models/{model}:generateContent",
            self.cf_account_id
        );
        let req = GenRequest {
            contents: vec![Content {
                parts: vec![Part { text: prompt }],
            }],
            generation_config,
        };
        let json_body = serde_json::to_string(&req)
            .map_err(|e| Error::RustError(format!("serialize Gemini request: {e}")))?;

        let headers = Headers::new();
        headers.set("Content-Type", "application/json")?;
        headers.set("x-goog-api-key", self.api_key)?;
        headers.set(
            "cf-aig-authorization",
            &format!("Bearer {}", self.cf_ai_gateway_token),
        )?;
        let mut init = RequestInit::new();
        init.with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(JsValue::from_str(&json_body)));
        Request::new_with_init(&url, &init)
    }
}

fn first_candidate_parts(body: &str) -> Result<Vec<RespPart>> {
    let parsed: Response = serde_json::from_str(body)
        .map_err(|e| Error::RustError(format!("parse Gemini JSON response: {e}")))?;

    Ok(parsed
        .candidates
        .into_iter()
        .next()
        .ok_or_else(|| Error::RustError(format!("Gemini response had no candidates: {body}")))?
        .content
        .parts)
}

enum Outcome<T> {
    Done(T),
    Fatal(String),
    Retry,
}

async fn post_with_retry<T>(
    request: &Request,
    classify: impl Fn(u16, &str) -> Outcome<T>,
) -> Result<T> {
    let mut attempt = 1u32;
    loop {
        let mut resp = Fetch::Request(request.clone()?).send().await?;
        let status = resp.status_code();
        let body = resp.text().await?;

        match classify(status, &body) {
            Outcome::Done(value) => return Ok(value),
            Outcome::Fatal(msg) => return Err(Error::RustError(msg)),
            Outcome::Retry if attempt > MAX_RETRIES => {
                return Err(Error::RustError(format!(
                    "Gemini still failing after {MAX_RETRIES} retries (last status {status}): {body}"
                )));
            }
            Outcome::Retry => {
                let backoff = BASE_BACKOFF * 2u32.pow(attempt.saturating_sub(1));
                console_log!(
                    "Gemini attempt {attempt}/{MAX_RETRIES} unsuccessful (status {status}); \
                     retrying in {}ms",
                    backoff.as_millis()
                );
                Delay::from(backoff).await;
                attempt += 1;
            }
        }
    }
}

fn is_transient(status: u16) -> bool {
    status == 429 || (500..600).contains(&status)
}
