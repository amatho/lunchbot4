use worker::{Env, Result};

pub struct Config {
    pub gemini_api_key: String,
    pub cf_account_id: String,
    pub cf_ai_gateway_token: String,
    pub slack_bot_token: String,
    pub slack_channel_id: String,
    pub r2_public_url_base: String,
}

impl Config {
    pub fn from_env(env: &Env) -> Result<Self> {
        Ok(Self {
            gemini_api_key: secret(env, "GEMINI_API_KEY")?,
            cf_account_id: var(env, "CF_ACCOUNT_ID")?,
            cf_ai_gateway_token: secret(env, "AI_GATEWAY_TOKEN")?,
            slack_bot_token: secret(env, "SLACK_BOT_TOKEN")?,
            slack_channel_id: secret(env, "SLACK_CHANNEL_ID")?,
            r2_public_url_base: var(env, "R2_PUBLIC_URL_BASE")?
                .trim_end_matches('/')
                .to_string(),
        })
    }
}

fn secret(env: &Env, name: &str) -> Result<String> {
    Ok(env.secret(name)?.to_string())
}

fn var(env: &Env, name: &str) -> Result<String> {
    Ok(env.var(name)?.to_string())
}
