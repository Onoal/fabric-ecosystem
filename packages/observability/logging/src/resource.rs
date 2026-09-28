use crate::{LogError, LogRecord};

fabric::resource! {
    pub LogSink {
        id: "onoal.package.observability.logging.sink";

        api {
            async fn emit(&self, record: LogRecord) -> Result<(), LogError>;
        }
    }
}
