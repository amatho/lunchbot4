mod config;
mod gemini;
mod r2;
mod scrape;
mod slack;

use chrono::{Datelike, Weekday};
use chrono_tz::Europe::Oslo;
use worker::*;

#[event(scheduled)]
async fn scheduled(event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    console_error_panic_hook::set_once();
    if let Err(e) = run(event, env).await {
        console_error!("lunchbot run failed: {e}");
    }
}

async fn run(event: ScheduledEvent, env: Env) -> Result<()> {
    let cfg = config::Config::from_env(&env)?;

    // Use the event's scheduled time rather than a wall clock — Workers have no
    // ambient system clock and this keeps the run deterministic relative to the
    // cron trigger.
    let now = chrono::DateTime::from_timestamp_millis(event.schedule() as i64)
        .ok_or_else(|| Error::RustError("invalid schedule timestamp".into()))?;
    let today = now.with_timezone(&Oslo).date_naive();
    let weekday = today.weekday();
    let date_iso = today.format("%Y-%m-%d").to_string();

    let day_no = match weekday {
        Weekday::Mon => "Mandag",
        Weekday::Tue => "Tirsdag",
        Weekday::Wed => "Onsdag",
        Weekday::Thu => "Torsdag",
        Weekday::Fri => "Fredag",
        Weekday::Sat | Weekday::Sun => {
            console_log!("{date_iso}: weekend ({weekday:?}) — no menu to post, exiting");
            return Ok(());
        }
    };

    console_log!("starting run for {day_no} {date_iso}");

    let menu_html = scrape::fetch_menu_html().await?;
    console_log!("fetched menu html ({} chars)", menu_html.len());

    let image = gemini::Gemini {
        api_key: &cfg.gemini_api_key,
        cf_account_id: &cfg.cf_account_id,
        cf_ai_gateway_token: &cfg.cf_ai_gateway_token,
    }
    .generate_image(day_no, &menu_html)
    .await?;
    console_log!("gemini returned image ({} bytes)", image.png.len());

    let key = format!("menu-{date_iso}.png");
    let bucket = env.bucket("BUCKET")?;
    r2::upload(&bucket, &key, image.png).await?;
    let url = r2::public_url(&cfg.r2_public_url_base, &key);
    console_log!("uploaded to r2: {key} -> {url}");

    let ts = slack::post_image(
        &cfg.slack_bot_token,
        &cfg.slack_channel_id,
        &url,
        day_no,
        &date_iso,
        &image.menu_text,
    )
    .await?;
    console_log!("posted to slack: {ts}");

    if let Err(e) = slack::post_thread_reply(
        &cfg.slack_bot_token,
        &cfg.slack_channel_id,
        &ts,
        &format!("Style prompt:\n```{}```", image.style_prompt),
    )
    .await
    {
        console_error!("failed to post style-prompt thread reply: {e}");
    }

    Ok(())
}
