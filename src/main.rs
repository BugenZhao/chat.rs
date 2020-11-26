#![allow(dead_code)]

#[macro_use]
extern crate serde;
// extern crate tokio;

mod client;
mod error;
mod message;
mod protocol;
mod server;
mod user;

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
    },
    Server {
        #[structopt(short, long, default_value = "30388")]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();
    match opt {
        Opt::Client { server, port } => {
            let client = client::Client::new("bugen", &server, port).await?;
            client.run().await?;
        }
        Opt::Server { port } => {
            let server = server::Server::new(port).await?;
            server.run().await?;
        }
    }
    Ok(())
}
