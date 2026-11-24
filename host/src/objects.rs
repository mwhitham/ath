//! Attachment files in an S3-compatible bucket (D259). Every request is a short-lived
//! signed URL, so any service that speaks the S3 protocol works: Amazon S3,
//! Cloudflare R2, Railway buckets, Supabase Storage, MinIO, and others. Keys are
//! `<resource-id>/attachments/<stored name>`. The stored name contains the record ID,
//! so different files never share a key.

use std::time::Duration;

use rusty_s3::actions::{DeleteObject, GetObject, PutObject};
use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};

use crate::config::{Config, S3Config};
use crate::error::{ApiError, ApiResult};

const SIGNED_FOR: Duration = Duration::from_secs(300);

#[derive(Clone)]
pub struct ObjectStore {
    bucket: Bucket,
    credentials: Credentials,
}

impl ObjectStore {
    pub fn new(config: &S3Config) -> Result<ObjectStore, String> {
        let endpoint = config.endpoint.parse().map_err(|error| format!("the bucket endpoint is not a URL: {error}"))?;
        let style = if config.path_style { UrlStyle::Path } else { UrlStyle::VirtualHost };
        let bucket = Bucket::new(endpoint, style, config.bucket.clone(), config.region.clone())
            .map_err(|error| format!("the bucket settings are not usable: {error}"))?;
        let credentials =
            Credentials::new(Config::secret(&config.access_key_env)?, Config::secret(&config.secret_key_env)?);
        Ok(ObjectStore { bucket, credentials })
    }

    pub fn key(resource_id: &str, stored_name: &str) -> String {
        format!("{resource_id}/attachments/{stored_name}")
    }

    pub async fn put(&self, key: &str, bytes: Vec<u8>) -> ApiResult<()> {
        let url = PutObject::new(&self.bucket, Some(&self.credentials), key).sign(SIGNED_FOR);
        blocking("storing an attachment", move || {
            ureq::put(url.as_str())
                .timeout(Duration::from_secs(120))
                .send_bytes(&bytes)
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
        .await
    }

    /// The object's bytes, or `None` when there is no such object.
    pub async fn get(&self, key: &str) -> ApiResult<Option<Vec<u8>>> {
        let url = GetObject::new(&self.bucket, Some(&self.credentials), key).sign(SIGNED_FOR);
        blocking("reading an attachment", move || {
            match ureq::get(url.as_str()).timeout(Duration::from_secs(120)).call() {
                Ok(response) => {
                    let mut bytes = Vec::new();
                    std::io::Read::read_to_end(&mut response.into_reader(), &mut bytes)
                        .map_err(|error| error.to_string())?;
                    Ok(Some(bytes))
                }
                Err(ureq::Error::Status(404, _)) => Ok(None),
                Err(error) => Err(error.to_string()),
            }
        })
        .await
    }

    pub async fn delete(&self, key: &str) -> ApiResult<()> {
        let url = DeleteObject::new(&self.bucket, Some(&self.credentials), key).sign(SIGNED_FOR);
        blocking("removing an attachment", move || {
            match ureq::delete(url.as_str()).timeout(Duration::from_secs(60)).call() {
                Ok(_) | Err(ureq::Error::Status(404, _)) => Ok(()),
                Err(error) => Err(error.to_string()),
            }
        })
        .await
    }

    /// A short-lived link a client can download the object from. Issue it only after
    /// the permission check (D259).
    pub fn download_link(&self, key: &str) -> String {
        GetObject::new(&self.bucket, Some(&self.credentials), key).sign(SIGNED_FOR).to_string()
    }
}

async fn blocking<T: Send + 'static>(
    context: &'static str,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> ApiResult<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| ApiError::internal(context, error))?
        .map_err(|error| ApiError::internal(context, error))
}
