use crate::{QueueMessage, QueueSendResult};

fabric::resource! {
    pub FifoQueue {
        id: "onoal.package.messaging.queue.fifo";

        api {
            fn send(&self, payload: Vec<u8>) -> QueueSendResult;
            fn try_receive(&self) -> Option<QueueMessage>;
            fn depth(&self) -> usize;
        }
    }
}
