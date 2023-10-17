mod cache;
mod config;

use crate::config::ConnectionType;
use crate::config::ConnectionType::{File, Http};
use crate::config::StepType::{Read, Run, Write};
use cache::Cache;

use clap::Parser;
use reqwest::header::HeaderMap;
use tokio::fs;
use tokio::process::Command;
use tokio::task;
use tokio_postgres::NoTls;

use error_chain::error_chain;

error_chain! {
    foreign_links {
        Io(std::io::Error);
        Postgres(tokio_postgres::Error);
    }
}

/// Automation tool for data ingestion
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Number of times to greet
    #[arg(short, long, default_value_t = String::from("./.dash/workflows/config.yml"))]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config = config::read_config(args.config);
    let cache = Cache::new();

    for step in config.steps {
        let cache = cache.clone();
        let task = task::spawn(async move {
            return match step.r#type {
                Read(read) => handle_read(read, cache).await,
                Write(write) => handle_write(write, cache).await,
                Run(run) => handle_run(run, cache).await,
            };
        });

        let result = task.await;
    }

    Ok(())
}

async fn handle_read(connection_type: ConnectionType, cache: Cache) {
    match connection_type {
        File(file_config) => {
            let data = fs::read(file_config.location).await;
            cache
                .data
                .lock()
                .unwrap()
                .insert("_".to_string(), data.unwrap().into());
        }
        Http(http_config) => {
            let headers: HeaderMap = match &http_config.headers {
                Some(headers) => headers.try_into().expect("valid headers"),
                None => HeaderMap::new(),
            };
            let client = reqwest::Client::new();
            let response = client.get(http_config.url).headers(headers).send().await;
            let body = response.expect("valid http response").text().await;
            cache
                .data
                .lock()
                .unwrap()
                .insert("_".to_string(), body.unwrap().into());
        }
        ConnectionType::Postgresql(postgresql_config) => {
            println!("{:?}", postgresql_config);
            let (client, connection) =
                tokio_postgres::connect(&postgresql_config.connection, NoTls)
                    .await
                    .unwrap();

            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("connection error: {}", e);
                }
            });

            let result = client.query(&postgresql_config.query, &[]).await.unwrap();

            println!("step3");

            for column in result[0].columns() {
                println!("{} ({})", column.name(), column.type_())
            }
            let value: String = result[0].get(0);

            cache
                .data
                .lock()
                .unwrap()
                .insert("_".to_string(), value.into());
        }
    }
}

async fn handle_write(connection_type: ConnectionType, cache: Cache) {
    match connection_type {
        File(file_config) => {
            let data = cache.data.lock().unwrap().get("_").unwrap().clone();
            fs::write(file_config.location, data).await;
        }
        Http(http_config) => {
            let headers: HeaderMap = match &http_config.headers {
                Some(headers) => headers.try_into().expect("valid headers"),
                None => HeaderMap::new(),
            };
            let client = reqwest::Client::new();

            let data = cache.data.lock().unwrap().get("generic").unwrap().clone();
            let response = client
                .get(http_config.url)
                .headers(headers)
                .body(data)
                .send()
                .await;
        }
        ConnectionType::Postgresql(_) => todo!(),
    }
}

async fn handle_run(command: String, cache: Cache) -> () {
    let output = Command::new(command).output().await;
    let stdout = output.unwrap().stdout.into();
    cache.data.lock().unwrap().insert("_".to_string(), stdout);
}
