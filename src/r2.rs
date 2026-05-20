use anyhow::{Context, Result};
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::{Client, Config};

pub fn make_client(account_id: &str, access_key: &str, secret_key: &str) -> Client {
    let creds = Credentials::from_keys(access_key, secret_key, None);
    let endpoint = format!("https://{account_id}.r2.cloudflarestorage.com");
    let config = Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("auto"))
        .endpoint_url(endpoint)
        .credentials_provider(creds)
        .build();
    Client::from_conf(config)
}

pub async fn upload(client: &Client, bucket: &str, key: &str, bytes: Vec<u8>) -> Result<()> {
    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(ByteStream::from(bytes))
        .content_type("image/png")
        .send()
        .await
        .with_context(|| format!("put_object to r2://{bucket}/{key}"))?;
    Ok(())
}

pub fn public_url(base: &str, key: &str) -> String {
    format!("{base}/{key}")
}
