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

use bytes::Buf;
use http::Response;
use http::StatusCode;
use quick_xml::de;
use serde::Deserialize;

use crate::raw::*;
use crate::Error;
use crate::ErrorKind;
use crate::Result;

/// ObsError is the error returned by obs service.
#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct ObsError {
    code: String,
    message: String,
    resource: String,
    request_id: String,
    host_id: String,
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
        // OBS could return `520 Origin Error` errors which should be retried.
        v if v.as_u16() == 520 => (ErrorKind::Unexpected, true),

        _ => (ErrorKind::Unexpected, false),
    };

    let message = match de::from_reader::<_, ObsError>(bs.clone().reader()) {
        Ok(obs_error) => format!("{:?}", obs_error),
        Err(_) => String::from_utf8_lossy(&bs).into_owned(),
    };

    let mut err = Error::new(kind, &message).with_context("response", format!("{:?}", parts));

    if retryable {
        err = err.set_temporary();
    }

    Ok(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error() {
        let bs = bytes::Bytes::from(
            r#"
<?xml version="1.0" encoding="UTF-8"?>
<Error>
<Code>NoSuchKey</Code>
<Message>The resource you requested does not exist</Message>
<Resource>/example-bucket/object</Resource>
<RequestId>001B21A61C6C0000013402C4616D5285</RequestId>
<HostId>RkRCRDJENDc5MzdGQkQ4OUY3MTI4NTQ3NDk2Mjg0M0FBQUFBQUFBYmJiYmJiYmJD</HostId>
</Error>
"#,
        );

        let out: ObsError = de::from_reader(bs.reader()).expect("must success");
        println!("{:?}", out);

        assert_eq!(out.code, "NoSuchKey");
        assert_eq!(out.message, "The resource you requested does not exist");
        assert_eq!(out.resource, "/example-bucket/object");
        assert_eq!(out.request_id, "001B21A61C6C0000013402C4616D5285");
        assert_eq!(
            out.host_id,
            "RkRCRDJENDc5MzdGQkQ4OUY3MTI4NTQ3NDk2Mjg0M0FBQUFBQUFBYmJiYmJiYmJD"
        );
    }
}
