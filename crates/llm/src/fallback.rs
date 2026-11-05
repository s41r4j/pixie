//! A primary provider with one backup: if the primary fails before it has said anything
//! (network down, rate limit, outage, bad model id), the same request goes to the backup.

use crate::{LlmError, OnEvent, Provider, Request, Response, StreamEvent};
use std::sync::Arc;

pub struct Fallback {
    pub primary: Arc<dyn Provider>,
    pub backup: Arc<dyn Provider>,
}

#[async_trait::async_trait]
impl Provider for Fallback {
    fn label(&self) -> String {
        format!("{} (fallback: {})", self.primary.label(), self.backup.label())
    }
    fn supports_images(&self) -> bool {
        self.primary.supports_images()
    }
    async fn complete(&self, req: &Request, on_event: OnEvent<'_>) -> Result<Response, LlmError> {
        let mut spoke = false;
        let r = {
            let mut fwd = |e: StreamEvent| {
                spoke = true;
                on_event(e);
            };
            self.primary.complete(req, &mut fwd).await
        };
        match r {
            Err(LlmError::Cancelled) => Err(LlmError::Cancelled),
            Err(e) if !spoke => {
                log::warn!("primary model failed ({e}); using the fallback");
                let mut req = req.clone();
                // The backup has its own model ids.
                req.model = None;
                if !self.backup.supports_images() {
                    strip_images(&mut req);
                }
                self.backup.complete(&req, on_event).await
            }
            other => other,
        }
    }
}

fn strip_images(req: &mut Request) {
    use crate::Content;
    for m in req.messages.iter_mut() {
        for c in m.content.iter_mut() {
            match c {
                Content::Image { .. } => *c = Content::text("[screenshot omitted]"),
                Content::ToolResult { content, .. } => {
                    for x in content.iter_mut() {
                        if matches!(x, Content::Image { .. }) {
                            *x = Content::text("[image omitted]");
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Content, Message, ScriptedProvider};

    struct Down;
    #[async_trait::async_trait]
    impl Provider for Down {
        fn label(&self) -> String {
            "down".into()
        }
        fn supports_images(&self) -> bool {
            true
        }
        async fn complete(&self, _: &Request, _: OnEvent<'_>) -> Result<Response, LlmError> {
            Err(LlmError::Api { status: 529, message: "overloaded".into() })
        }
    }

    #[tokio::test]
    async fn backup_answers_when_primary_fails() {
        let f = Fallback { primary: Arc::new(Down), backup: Arc::new(ScriptedProvider::new(vec![ScriptedProvider::reply(vec![Content::text("hi")])])) };
        let req = Request::new("s", vec![Message::user_text("x")]);
        let r = f.complete(&req, &mut |_| {}).await.unwrap();
        assert_eq!(r.text(), "hi");
    }
}
