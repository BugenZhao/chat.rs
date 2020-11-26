#![allow(dead_code)]

mod client;
mod server;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "dnser", about = "A DNS utility by Bugen.")]
enum Opt {
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

fn main() {
    println!("Hello, world!");
}
