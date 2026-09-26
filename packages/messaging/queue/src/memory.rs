use std::collections::VecDeque;
use std::sync::Mutex;

use crate::{FifoQueue, QueueMessage, QueueSendResult};

#[derive(Default)]
pub(crate) struct InMemoryQueueState {
    messages: Mutex<VecDeque<QueueMessage>>,
}

fabric::adapter! {
    pub InMemoryQueue for FifoQueue {
        id: "onoal.package.messaging.queue.fifo.in-memory";

        config {
            capacity: usize;
        }

        state {
            InMemoryQueueState = InMemoryQueueState::default();
        }

        runtime {
            fn send(&self, payload: Vec<u8>) -> QueueSendResult {
                let mut messages = self
                    .state
                    .get()
                    .messages
                    .lock()
                    .expect("in-memory queue state");
                if messages.len() >= self.config.capacity {
                    QueueSendResult::Full {
                        capacity: self.config.capacity,
                    }
                } else {
                    messages.push_back(QueueMessage { payload });
                    QueueSendResult::Accepted
                }
            }

            fn try_receive(&self) -> Option<QueueMessage> {
                self.state
                    .get()
                    .messages
                    .lock()
                    .expect("in-memory queue state")
                    .pop_front()
            }

            fn depth(&self) -> usize {
                self.state
                    .get()
                    .messages
                    .lock()
                    .expect("in-memory queue state")
                    .len()
            }
        }
    }
}
