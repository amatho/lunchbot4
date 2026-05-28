use worker::{Bucket, HttpMetadata, Result};

pub async fn upload(bucket: &Bucket, key: &str, bytes: Vec<u8>) -> Result<()> {
    bucket
        .put(key, bytes)
        .http_metadata(HttpMetadata {
            content_type: Some("image/png".into()),
            ..Default::default()
        })
        .execute()
        .await?;
    Ok(())
}

pub fn public_url(base: &str, key: &str) -> String {
    format!("{base}/{key}")
}
