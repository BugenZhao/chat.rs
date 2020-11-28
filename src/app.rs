mod basic_app;
mod tui_app;

pub use basic_app::BasicApp;
pub use tui_app::TuiApp;

use crate::{client::ClientInput, error::Result, protocol::ServerCommand};

type Tx<T> = tokio::sync::mpsc::UnboundedSender<T>;
type Rx<T> = tokio::sync::mpsc::UnboundedReceiver<T>;

/// Collect user inputs to `Client` and show data from `Client`
pub trait App {
    /// This method will create and run async app tasks, and return immediately
    fn start(input_tx: Tx<ClientInput>, msg_rx: Rx<ServerCommand>, name: &str) -> Result<()>;
}
