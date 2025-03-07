// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use http::Response;
use http::StatusCode;
use serde::Deserialize;
use serde_json::de;

use crate::raw::*;
use crate::Error;
use crate::ErrorKind;
use crate::Result;

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct GcsErrorResponse {
    error: GcsError,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct GcsError {
    code: usize,
    message: String,
    errors: Vec<GcsErrorDetail>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct GcsErrorDetail {
    domain: String,
    location: String,
    location_type: String,
    message: String,
    reason: String,
}

/// Parse error respons into Error.
pub async fn parse_error(resp: Response<IncomingAsyncBody>) -> Result<Error> {
    let (parts, body) = resp.into_parts();
    let bs = body.bytes().await?;

    let (kind, retryable) = match parts.status {
        StatusCode::NOT_FOUND => (ErrorKind::ObjectNotFound, false),
        StatusCode::FORBIDDEN => (ErrorKind::ObjectPermissionDenied, false),
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let message = match de::from_slice::<GcsErrorResponse>(&bs) {
        Ok(gcs_err) => format!("{:?}", gcs_err),
        Err(_) => String::from_utf8_lossy(&bs).into_owned(),
    };

    let mut err = Error::new(kind, &message).with_context("response", format!("{:?}", parts));

    if retryable {
        err = err.set_temporary();
    }

    Ok(err)
}

pub fn parse_json_deserialize_error(e: serde_json::Error) -> Error {
    Error::new(ErrorKind::Unexpected, "deserialize json").set_source(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error() {
        let bs = bytes::Bytes::from(
            r#"
{
"error": {
 "errors": [
  {
   "domain": "global",
   "reason": "required",
   "message": "Login Required",
   "locationType": "header",
   "location": "Authorization"
  }
 ],
 "code": 401,
 "message": "Login Required"
 }
}
"#,
        );

        let out: GcsErrorResponse = de::from_slice(&bs).expect("must success");
        println!("{:?}", out);

        assert_eq!(out.error.code, 401);
        assert_eq!(out.error.message, "Login Required");
        assert_eq!(out.error.errors[0].domain, "global");
        assert_eq!(out.error.errors[0].reason, "required");
        assert_eq!(out.error.errors[0].message, "Login Required");
        assert_eq!(out.error.errors[0].location_type, "header");
        assert_eq!(out.error.errors[0].location, "Authorization");
    }
}
