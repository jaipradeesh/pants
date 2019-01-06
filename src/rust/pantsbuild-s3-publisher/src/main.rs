use futures::Future;
use lambda_runtime::{error::HandlerError, lambda, Context};
use pantsbuild_s3_publisher::*;
use rusoto_core::Region;
use rusoto_core::RusotoFuture;
use rusoto_s3::{
    DeleteObjectError, DeleteObjectOutput, DeleteObjectRequest, GetObjectRequest,
    PutObjectAclError, PutObjectAclOutput, PutObjectAclRequest, S3Client, S3,
};
use serde_derive::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::read_to_end;

fn main() {
    env_logger::init();

    lambda!(handle);
}

#[derive(Deserialize)]
struct Event {
    #[serde(rename = "Records")]
    records: Vec<Record>,
}

#[derive(Deserialize)]
struct Record {
    s3: S3Record,
    #[serde(rename = "awsRegion")]
    aws_region: String,
}

#[derive(Deserialize)]
struct S3Record {
    bucket: S3Bucket,
    object: S3Object,
}

#[derive(Deserialize)]
struct S3Bucket {
    name: String,
}

#[derive(Deserialize)]
struct S3Object {
    key: String,
    size: usize,
}

#[derive(Serialize)]
struct Output {
    message: String,
}

fn handle(event: Event, context: Context) -> impl Future<Item = Output, Error = HandlerError> {
    let record = &event.records[0];
    let s3 = Arc::new(S3Client::new(
        Region::from_str(&record.aws_region).expect("TODO: Region"),
    ));
    let bucket = record.s3.bucket.name.clone();
    let key = record.s3.object.key.clone();
    let mut request = GetObjectRequest::default();
    request.bucket = bucket.clone();
    request.key = key.clone();
    let size = record.s3.object.size;
    let buf = Vec::with_capacity(size);
    s3.get_object(request)
        .map_err(|err| format!("Error getting S3 object handle: {}", err))
        .and_then(|resp| resp.body.ok_or_else(|| ("No body".to_owned())))
        .and_then(move |body| {
            read_to_end(body.into_async_read(), buf)
                .map_err(|err| format!("Error reading file contents: {}", err))
        })
        .map(|(_body, bytes)| bytes)
        .and_then(move |bytes| match validate(&key, &bytes) {
            Validation::Good => Box::new(
                open_acl(&s3, bucket.clone(), key.clone())
                    .map(|_| "Opened up ACL".to_owned())
                    .map_err(move |err| {
                        format!("Error opening up ACL for {} {}: {}", bucket, key, err)
                    }),
            ) as Box<dyn Future<Item = _, Error = _> + Send>,
            Validation::Bad => Box::new(
                delete(&s3, bucket.clone(), key.clone())
                    .map(|_| "Deleted".to_owned())
                    .map_err(move |err| format!("Error deleting {} {}: {}", bucket, key, err)),
            ) as Box<dyn Future<Item = _, Error = _> + Send>,
        })
        .map(|message| Output { message })
        .map_err(move |err| context.new_error(&err))
}

fn open_acl(
    s3: &Arc<S3Client>,
    bucket: String,
    key: String,
) -> RusotoFuture<PutObjectAclOutput, PutObjectAclError> {
    let mut request = PutObjectAclRequest::default();
    request.bucket = bucket;
    request.key = key;
    request.acl = Some("public-read".to_owned());
    s3.put_object_acl(request)
}

fn delete(
    s3: &Arc<S3Client>,
    bucket: String,
    key: String,
) -> RusotoFuture<DeleteObjectOutput, DeleteObjectError> {
    let mut request = DeleteObjectRequest::default();
    request.bucket = bucket;
    request.key = key;
    s3.delete_object(request)
}
