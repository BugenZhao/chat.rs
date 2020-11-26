#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Message {
    Text(String),
}
