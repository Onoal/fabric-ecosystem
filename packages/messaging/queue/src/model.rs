#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueMessage {
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueSendResult {
    Accepted,
    Full { capacity: usize },
}
