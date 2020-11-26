#[derive(Serialize, Deserialize, Clone)]
pub enum Message {
    Text(String),
}
