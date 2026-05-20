mod config;
mod gemini;
mod r2;
mod scrape;
mod slack;

use anyhow::Result;
use chrono::{Datelike, Weekday};
use chrono_tz::Europe::Oslo;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = config::Config::from_env()?;

    let today = chrono::Utc::now().with_timezone(&Oslo).date_naive();
    let weekday = today.weekday();
    let date_iso = today.format("%Y-%m-%d").to_string();

    let day_no = match weekday {
        Weekday::Mon => "Mandag",
        Weekday::Tue => "Tirsdag",
        Weekday::Wed => "Onsdag",
        Weekday::Thu => "Torsdag",
        Weekday::Fri => "Fredag",
        Weekday::Sat | Weekday::Sun => {
            tracing::info!(%date_iso, ?weekday, "weekend — no menu to post, exiting");
            return Ok(());
        }
    };

    tracing::info!(%date_iso, %day_no, "starting run");

    let menu_html = scrape::fetch_menu_html().await?;
    tracing::info!(chars = menu_html.len(), "fetched menu html");

    let png = gemini::generate_image(&cfg.gemini_api_key, day_no, &menu_html).await?;
    tracing::info!(bytes = png.len(), "gemini returned image");

    let key = format!("menu-{date_iso}.png");
    let client = r2::make_client(
        &cfg.r2_account_id,
        &cfg.r2_access_key_id,
        &cfg.r2_secret_access_key,
    );
    r2::upload(&client, &cfg.r2_bucket, &key, png).await?;
    let url = r2::public_url(&cfg.r2_public_url_base, &key);
    tracing::info!(%key, %url, "uploaded to r2");

    let ts = slack::post_image(
        &cfg.slack_bot_token,
        &cfg.slack_channel_id,
        &url,
        day_no,
        &date_iso,
    )
    .await?;
    tracing::info!(%ts, "posted to slack");

    Ok(())
}
