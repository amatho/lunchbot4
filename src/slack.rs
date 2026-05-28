use serde::{Deserialize, Serialize};
use serde_json::json;
use worker::wasm_bindgen::JsValue;
use worker::{Error, Fetch, Headers, Method, Request, RequestInit, Result};

const POST_MESSAGE: &str = "https://slack.com/api/chat.postMessage";

#[derive(Deserialize)]
struct PostResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    ts: Option<String>,
}

#[derive(Serialize)]
struct Body<'a> {
    channel: &'a str,
    text: String,
    blocks: serde_json::Value,
}

pub async fn post_image(
    bot_token: &str,
    channel: &str,
    image_url: &str,
    day_no: &str,
    date_iso: &str,
) -> Result<String> {
    let title = format!("Dagens meny — {day_no} {date_iso}");
    let body = Body {
        channel,
        text: title.clone(),
        blocks: json!([
            {
                "type": "section",
                "text": { "type": "mrkdwn", "text": format!("*Dagens meny* — {day_no} {date_iso}") }
            },
            {
                "type": "image",
                "image_url": image_url,
                "alt_text": "Dagens meny"
            }
        ]),
    };
    let json_body = serde_json::to_string(&body)
        .map_err(|e| Error::RustError(format!("serialize Slack body: {e}")))?;

    let mut headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {bot_token}"))?;
    headers.set("Content-Type", "application/json")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(&json_body)));
    let request = Request::new_with_init(POST_MESSAGE, &init)?;

    let mut resp = Fetch::Request(request).send().await?;
    let status = resp.status_code();
    let text = resp.text().await?;
    if !(200..300).contains(&status) {
        return Err(Error::RustError(format!(
            "non-2xx ({status}) from chat.postMessage: {text}"
        )));
    }

    let parsed: PostResponse = serde_json::from_str(&text)
        .map_err(|e| Error::RustError(format!("parse Slack response: {e}")))?;
    if !parsed.ok {
        return Err(Error::RustError(format!(
            "slack chat.postMessage failed: {}",
            parsed.error.unwrap_or_else(|| "unknown".into())
        )));
    }
    Ok(parsed.ts.unwrap_or_default())
}
