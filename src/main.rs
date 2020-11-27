#![allow(dead_code)]

#[macro_use]
extern crate serde;
// extern crate tokio;

mod client;
mod error;
mod message;
mod protocol;
mod server;

use crate::error::*;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "chat", about = "A DNS utility by Bugen.")]
pub enum Opt {
    Client {
        #[structopt(short, long, default_value = "127.0.0.1")]
        server: String,
        #[structopt(short, long, default_value = "30388")]
        port: u16,
        #[structopt(short, long)]
        name: String,
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
        Opt::Client { server, port, name } => {
            let client = client::Client::new(&name, &server, port);
            client.run().await?;
        }
        Opt::Server { port } => {
            let server = server::Server::new(port).await?;
            server.run().await?;
        }
    }
    Ok(())
}
