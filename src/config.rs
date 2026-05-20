use anyhow::{Context, Result};

pub struct Config {
    pub gemini_api_key: String,
    pub slack_bot_token: String,
    pub slack_channel_id: String,
    pub r2_account_id: String,
    pub r2_access_key_id: String,
    pub r2_secret_access_key: String,
    pub r2_bucket: String,
    pub r2_public_url_base: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            gemini_api_key: required("GEMINI_API_KEY")?,
            slack_bot_token: required("SLACK_BOT_TOKEN")?,
            slack_channel_id: required("SLACK_CHANNEL_ID")?,
            r2_account_id: required("R2_ACCOUNT_ID")?,
            r2_access_key_id: required("R2_ACCESS_KEY_ID")?,
            r2_secret_access_key: required("R2_SECRET_ACCESS_KEY")?,
            r2_bucket: required("R2_BUCKET")?,
            r2_public_url_base: required("R2_PUBLIC_URL_BASE")?
                .trim_end_matches('/')
                .to_string(),
        })
    }
}

fn required(name: &str) -> Result<String> {
    std::env::var(name).with_context(|| format!("missing required env var {name}"))
}
