//! Captured code mail with bounded accepted-then-failed/delayed delivery faults.
use irongate::email::{EmailDeliveryError, RenderedEmail, VerificationEmailSender};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

#[derive(Default)]
pub struct Mail {
    messages: Mutex<Vec<(String, RenderedEmail)>>,
    fail: bool,
    gate: Option<(Arc<Notify>, Arc<Notify>)>,
}

#[async_trait::async_trait]
impl VerificationEmailSender for Mail {
    async fn send_verification_email(
        &self,
        to: &str,
        message: RenderedEmail,
    ) -> Result<String, EmailDeliveryError> {
        self.messages.lock().unwrap().push((to.into(), message));
        if let Some((entered, release)) = &self.gate {
            entered.notify_one();
            tokio::time::timeout(std::time::Duration::from_secs(5), release.notified())
                .await
                .expect("release delivery");
        }
        if self.fail {
            Err(EmailDeliveryError::ProviderStatus { status: 503 })
        } else {
            Ok("test-delivery-id-not-a-continuation".into())
        }
    }
}

impl Mail {
    pub fn failing() -> Self {
        Self {
            fail: true,
            ..Self::default()
        }
    }
    pub fn paused(entered: Arc<Notify>, release: Arc<Notify>) -> Self {
        Self {
            gate: Some((entered, release)),
            ..Self::default()
        }
    }
    pub fn len(&self) -> usize {
        self.messages.lock().unwrap().len()
    }
    pub fn message(&self) -> (String, RenderedEmail) {
        self.messages.lock().unwrap().last().unwrap().clone()
    }
    pub fn code(&self) -> String {
        let (_, message) = self.message();
        let codes: Vec<_> = message
            .text
            .lines()
            .filter(|line| line.len() == 8 && line.bytes().all(|b| b.is_ascii_digit()))
            .collect();
        assert_eq!(codes.len(), 1);
        codes[0].into()
    }
}
