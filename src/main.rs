#![allow(dead_code)]

#[macro_use]
extern crate serde;

mod app;
mod client;
mod error;
mod message;
mod protocol;
mod server;

use crate::error::*;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "chat",
    about = "An example of async chat client/server with tokio."
)]
pub enum Opt {
    Client {
        #[structopt(short, long, default_value = "127.0.0.1")]
        server: String,
        #[structopt(short, long, default_value = "30388")]
        port: u16,
        #[structopt(short, long)]
        name: String,
        #[structopt(short, long)]
        tui: bool,
    },
    Server {
        #[structopt(short, long, default_value = "30388")]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    match Opt::from_args() {
        Opt::Client {
            server,
            port,
            name,
            tui,
        } => {
            let client = client::Client::new(&name, &server, port, tui);
            client.run().await?;
        }
        Opt::Server { port } => {
            let server = server::Server::new(port).await?;
            server.run().await?;
        }
    }

    std::process::exit(0);
}
