use anyhow::{Context, Result, anyhow};
use regex::Regex;
use serde_json::Value;

const MENU_URL: &str = "https://tullin.munu.shop/meny";

pub async fn fetch_menu_html() -> Result<String> {
    let body = reqwest::get(MENU_URL)
        .await
        .context("GET tullin.munu.shop/meny failed")?
        .error_for_status()
        .context("non-2xx from tullin.munu.shop/meny")?
        .text()
        .await
        .context("read body from tullin.munu.shop/meny")?;
    extract_menu_html(&body)
}

fn extract_menu_html(page: &str) -> Result<String> {
    // The SPA state is embedded twice-encoded: a JSON string literal in the page
    // whose value is itself a JSON document. We locate the outer literal by
    // looking for one that contains all five Norwegian weekday names.
    let re = Regex::new(r#""(?:[^"\\]|\\.)*""#).unwrap();
    let menu_lit = re
        .find_iter(page)
        .filter(|m| {
            let s = m.as_str();
            s.contains("Mandag") && s.contains("Tirsdag") && s.contains("Fredag")
        })
        .max_by_key(|m| m.len())
        .ok_or_else(|| {
            anyhow!(
                "could not find a JSON string containing weekday markers in page \
                 ({} bytes); the menu encoding may have changed",
                page.len()
            )
        })?;

    let state_json: String = serde_json::from_str(menu_lit.as_str())
        .context("decode outer JSON string literal")?;
    let state: Value = serde_json::from_str(&state_json).context("parse SPA state JSON")?;

    // Within the state, the menu HTML lives in some string field. Find the
    // largest string value that contains all five weekday markers.
    let mut best = String::new();
    walk_strings(&state, &mut |s| {
        if s.contains("Mandag")
            && s.contains("Tirsdag")
            && s.contains("Onsdag")
            && s.contains("Torsdag")
            && s.contains("Fredag")
            && s.len() > best.len()
        {
            best = s.to_string();
        }
    });

    if best.is_empty() {
        return Err(anyhow!(
            "no string in SPA state contains all five weekday markers"
        ));
    }
    Ok(best)
}

fn walk_strings<'a>(v: &'a Value, on_string: &mut impl FnMut(&'a str)) {
    match v {
        Value::String(s) => on_string(s),
        Value::Array(a) => a.iter().for_each(|x| walk_strings(x, on_string)),
        Value::Object(m) => m.values().for_each(|x| walk_strings(x, on_string)),
        _ => {}
    }
}
