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

//! example for initiating a Rocksdb backend

use std::env;

use anyhow::Result;
use log::info;
use opendal::services::rocksdb;
use opendal::services::rocksdb::Builder;
use opendal::Operator;

#[tokio::main]
async fn main() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "debug");
    }
    env_logger::init();
    println!(
        r#"
        OpenDAL rocksdb example.

        Available Environment Variables:

        - OPENDAL_ROCKSDB_DATADIR: the path to the rocksdb data directory (required)
        - OPENDAL_ROCKSDB_ROOT:    working directory of opendal, default is "/"
        "#
    );

    // Create rocksdb backend builder
    let mut builder: Builder = rocksdb::Builder::default();

    // Set the root, all operations will happen under this directory, or prefix, more accurately.
    //
    // NOTE: the root must be absolute path
    builder.root(&env::var("OPENDAL_ROCKSDB_ROOT").unwrap_or_else(|_| "/".to_string()));

    // Set the path to the rocksdb data directory
    builder.datadir(
        &env::var("OPENDAL_ROCKSDB_DATADIR").expect("env OPENDAL_ROCKSDB_DATADIR is not set"),
    );

    // `Accessor` provides the low level APIs, we will use `Operator` normally.
    let op: Operator = Operator::new(builder.build()?);

    let path = uuid::Uuid::new_v4().to_string();

    // Create an object handle to start operation on object.
    info!("try to write file: {}", &path);
    op.object(&path).write("Hello, world!").await?;
    info!("write file successful!");

    info!("try to read file: {}", &path);
    let content = op.object(&path).read().await?;
    info!(
        "read file successful, content: {}",
        String::from_utf8_lossy(&content)
    );

    info!("try to get file metadata: {}", &path);
    let meta = op.object(&path).metadata().await?;
    info!(
        "get file metadata successful, size: {}B",
        meta.content_length()
    );

    info!("try to delete file: {}", &path);
    op.object(&path).delete().await?;
    info!("delete file successful");

    Ok(())
}
