use tokio::sync::mpsc;

use crate::error::*;

type Tx<T> = mpsc::UnboundedSender<T>;
type Rx<T> = mpsc::UnboundedReceiver<T>;

pub trait App {
    fn start(input_tx: Tx<String>, msg_rx: Rx<String>) -> Result<()>;
}

#[derive(Default)]
pub struct BasicApp {
    input: String,
    messages: Vec<String>,
}

impl App for BasicApp {
    fn start(input_tx: Tx<String>, mut msg_rx: Rx<String>) -> Result<()> {
        let mut _app = Self::default();

        tokio::spawn(async move {
            loop {
                let input = {
                    let mut buf = String::new();
                    std::io::stdin().read_line(&mut buf).unwrap();
                    buf
                };
                input_tx.send(input).unwrap();
            }
        });

        tokio::spawn(async move {
            while let Some(content) = msg_rx.recv().await {
                println!("{}", content);
            }
        });

        Ok(())
    }
}
