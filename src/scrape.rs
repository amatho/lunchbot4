use regex::Regex;
use serde_json::Value;
use worker::{Error, Fetch, Method, Request, Result};

const MENU_URL: &str = "https://tullin.munu.shop/meny";

pub async fn fetch_menu_html() -> Result<String> {
    let req = Request::new(MENU_URL, Method::Get)?;
    let mut resp = Fetch::Request(req).send().await?;
    let status = resp.status_code();
    if !(200..300).contains(&status) {
        return Err(Error::RustError(format!(
            "non-2xx ({status}) from tullin.munu.shop/meny"
        )));
    }
    let body = resp.text().await?;
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
            Error::RustError(format!(
                "could not find a JSON string containing weekday markers in page \
                 ({} bytes); the menu encoding may have changed",
                page.len()
            ))
        })?;

    let state_json: String = serde_json::from_str(menu_lit.as_str())
        .map_err(|e| Error::RustError(format!("decode outer JSON string literal: {e}")))?;
    let state: Value = serde_json::from_str(&state_json)
        .map_err(|e| Error::RustError(format!("parse SPA state JSON: {e}")))?;

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
        return Err(Error::RustError(
            "no string in SPA state contains all five weekday markers".into(),
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
