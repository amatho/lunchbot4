use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::json;

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

    let resp = reqwest::Client::new()
        .post(POST_MESSAGE)
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .context("POST chat.postMessage failed")?
        .error_for_status()
        .context("non-2xx from chat.postMessage")?;

    let parsed: PostResponse = resp.json().await.context("parse Slack response")?;
    if !parsed.ok {
        return Err(anyhow!(
            "slack chat.postMessage failed: {}",
            parsed.error.unwrap_or_else(|| "unknown".into())
        ));
    }
    Ok(parsed.ts.unwrap_or_default())
}
