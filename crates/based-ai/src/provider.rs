use futures::stream::BoxStream;

use crate::error::AiError;
use crate::types::{ChatRequest, Chunk};

pub type ChunkStream = BoxStream<'static, Result<Chunk, AiError>>;

/// A streaming chat backend. Object-safe so the app can hold `Arc<dyn Provider>`.
pub trait Provider: Send + Sync {
    fn name(&self) -> &'static str;

    /// Starts a streamed completion. The stream ends after `Chunk::Done`.
    fn stream(&self, request: ChatRequest) -> ChunkStream;
}

#[cfg(test)]
mod tests {
    use futures::{StreamExt, stream};

    use super::*;
    use crate::types::{Message, Usage};

    struct Echo;

    impl Provider for Echo {
        fn name(&self) -> &'static str {
            "echo"
        }

        fn stream(&self, request: ChatRequest) -> ChunkStream {
            let text = request.messages[0].content.clone();
            stream::iter([Ok(Chunk::Text(text)), Ok(Chunk::Done(Usage::default()))]).boxed()
        }
    }

    #[tokio::test]
    async fn dyn_provider_streams() {
        let p: Box<dyn Provider> = Box::new(Echo);
        let req = ChatRequest::new("m", vec![Message::user("hi")]);
        let chunks: Vec<_> = p.stream(req).collect().await;
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], Ok(Chunk::Text("hi".into())));
    }
}
