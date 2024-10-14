use tokio::sync::mpsc;
use tower_lsp::lsp_types::MessageType;
use tower_lsp::Client;
use tracing::Subscriber;
use tracing_subscriber::{fmt::format::Writer, Layer};

pub struct Logger {
    sender: mpsc::Sender<(MessageType, String)>,
}

impl Logger {
    pub fn new(client: Client) -> Self {
        let (sender, mut receiver) = mpsc::channel(256);

        // Spawn a long-running task to handle logging
        tokio::spawn(async move {
            while let Some((message_type, log_message)) = receiver.recv().await {
                client.log_message(message_type, log_message).await;
            }
        });

        Self { sender }
    }
}

impl<S> Layer<S> for Logger
where
    S: Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let level = metadata.level();

        let message_type = match *level {
            tracing::Level::ERROR => MessageType::ERROR,
            tracing::Level::WARN => MessageType::WARNING,
            tracing::Level::INFO => MessageType::INFO,
            tracing::Level::DEBUG | tracing::Level::TRACE => MessageType::LOG,
        };

        let mut message = String::with_capacity(128);
        let writer = Writer::new(&mut message);
        let mut visitor = tracing_subscriber::fmt::format::DefaultVisitor::new(
            writer,
            event.fields().count() == 0,
        );
        event.record(&mut visitor);

        let log_message = format!("[{}] {}: {}", level, metadata.target(), message.trim());

        if let Err(e) = self.sender.try_send((message_type, log_message)) {
            eprintln!("Failed to send log message: {}", e);
        }
    }
}
