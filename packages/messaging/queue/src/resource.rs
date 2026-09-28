use crate::{QueueMessage, QueueSendResult};

fabric::resource! {
    pub FifoQueue {
        id: "onoal.package.messaging.queue.fifo";

        api {
            async fn send(&self, payload: Vec<u8>) -> QueueSendResult;
            async fn try_receive(&self) -> Option<QueueMessage>;
            async fn depth(&self) -> usize;
        }
    }
}
