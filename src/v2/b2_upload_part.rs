use reqwest::Body;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

use crate::header_serializer::HeadersFrom;

use super::{
    server_side_encryption::EncryptionAlgorithm, Error, FileId, JsonErrorObj, Md5, PartNumber,
    ServerSideEncryption, ServerSideEncryptionCustomerKey, Sha1, TimeStamp,
    UploadPartUrlParameters,
};

#[derive(Debug, Serialize, TypedBuilder)]
pub struct UploadPartParameters {
    #[serde(rename = "X-Bz-Part-Number")]
    /// A number from 1 to 10000. The parts uploaded for one file must have contiguous numbers, starting with 1.
    part_number: PartNumber,

    #[serde(rename = "Content-Length")]
    /// The number of bytes in the file being uploaded. Note that this header is required; you cannot leave it out and just use chunked encoding.
    /// The minimum size of every part but the last one is 5MB.
    /// When sending the SHA1 checksum at the end, the Content-Length should be set to the size of the file plus the 40 bytes of hex checksum.
    content_length: u64,

    #[serde(rename = "X-Bz-Content-Sha1")]
    /// The SHA1 checksum of the this part of the file. B2 will check this when the part is uploaded, to make sure that the data arrived correctly.
    /// The same SHA1 checksum must be passed to b2_finish_large_file.
    /// You may optionally provide the SHA1 at the end of the upload.
    content_sha1: Sha1,

    #[serde(rename = "X-Bz-Server-Side-Encryption-Customer-Algorithm")]
    #[builder(default, setter(strip_option))]
    /// This header is required if b2_start_large_file was called with parameters specifying Server-Side Encryption with Customer-Managed Keys (SSE-C), in which case its value must match the serverSideEncryption algorithm requested via b2_start_large_file.
    server_side_encryption_algorithm: Option<EncryptionAlgorithm>,

    #[serde(rename = "X-Bz-Server-Side-Encryption-Customer-Key")]
    #[builder(default, setter(strip_option))]
    /// This header is required if b2_start_large_file was called with parameters specifying Server-Side Encryption with Customer-Managed Keys (SSE-C), in which case its value must match the serverSideEncryption customerKey requested via b2_start_large_file.
    server_side_encryption_customer_key: Option<ServerSideEncryptionCustomerKey>,

    #[serde(rename = "X-Bz-Server-Side-Encryption-Customer-Key-Md5")]
    #[builder(default, setter(strip_option))]
    /// This header is required if b2_start_large_file was called with parameters specifying Server-Side Encryption with Customer-Managed Keys (SSE-C), in which case its value must match the serverSideEncryption customerKeyMd5 requested via b2_start_large_file.
    server_side_encryption_customer_key_md5: Option<Md5>,
}

#[derive(Debug)]
pub enum UploadPartError {
    BadRequest {
        raw_error: JsonErrorObj,
    },
    Unauthorized {
        raw_error: JsonErrorObj,
    },
    /// acc. to documentaion: Call b2_get_upload_part_url again to get a new auth token
    BadAuthToken {
        raw_error: JsonErrorObj,
    },
    /// acc. to documentaion: Call b2_get_upload_part_url again to get a new auth token
    ExpiredAuthToken {
        raw_error: JsonErrorObj,
    },
    // Method Not allowed listed in documentation, but skipped here, as the request method forces POST
    RequestTimeout {
        raw_error: JsonErrorObj,
    },
    /// acc. to documentaion: Call b2_get_upload_part_url again to get a new auth token
    ServiceUnavailable {
        raw_error: JsonErrorObj,
    },
    Unexpected {
        raw_error: Error,
    },
}

impl From<reqwest::Error> for UploadPartError {
    fn from(e: reqwest::Error) -> Self {
        //TODO separate error for network / timeouts??
        Self::Unexpected {
            raw_error: Error::ReqwestError(e),
        }
    }
}

impl From<JsonErrorObj> for UploadPartError {
    fn from(e: JsonErrorObj) -> Self {
        match (e.status as usize, e.code.as_str()) {
            (400, "bad_request") => Self::BadRequest { raw_error: e },
            (401, "unauthorized") => Self::Unauthorized { raw_error: e },
            (401, "bad_auth_token") => Self::BadAuthToken { raw_error: e },
            (401, "expired_auth_token") => Self::ExpiredAuthToken { raw_error: e },
            (408, "request_timeout") => Self::RequestTimeout { raw_error: e },
            (503, "service_unavailable") => Self::ServiceUnavailable { raw_error: e },
            _ => Self::Unexpected {
                raw_error: Error::JsonError(e),
            },
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadPartOk {
    file_id: FileId,
    part_number: PartNumber,
    content_length: u64,
    content_sha1: Sha1,
    content_md5: Option<Md5>,
    server_side_encryption: Option<ServerSideEncryption>,
    upload_timestamp: TimeStamp,
}

pub async fn b2_upload_part<T: Into<Body>>(
    uploader_params: &mut UploadPartUrlParameters,
    upload_part_params: &UploadPartParameters,
    file_contents: T,
) -> Result<UploadPartOk, UploadPartError> {
    let resp = reqwest::Client::new()
        .post(uploader_params.upload_url().as_str())
        .header(
            "Authorization",
            uploader_params.authorization_token().as_str(),
        )
        .headers_from(upload_part_params)
        .body(file_contents)
        .send()
        .await
        .map_err(UploadPartError::from)?;
    if resp.status().as_u16() == http_types::StatusCode::Ok as u16 {
        Ok(resp.json().await.map_err(UploadPartError::from)?)
    } else {
        let raw_error: JsonErrorObj = resp.json().await.map_err(UploadPartError::from)?;
        Err(raw_error.into())
    }
}