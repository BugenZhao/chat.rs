use std::fmt;

pub type User = String;
/// All possible kinds of normal messages
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Message {
    Text(String),
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Message::Text(text) => {
                write!(f, "{}", text.trim())
            }
        }
    }
}
