//! Password fixtures complete the same email-first flow as public callers.

use irongate::{
    config::environment::RuntimeAuthConfig,
    core::subjects::Subject,
    email::{EmailDeliveryError, RenderedEmail, VerificationEmailSender},
    providers::password::{
        register_password_user, verify_password_email, PasswordRegistrationInput,
        PasswordVerificationInput,
    },
    store::AuthStore,
};
use std::sync::Mutex;

#[derive(Default)]
pub struct Mailbox(Mutex<Vec<String>>);

#[async_trait::async_trait]
impl VerificationEmailSender for Mailbox {
    async fn send_verification_email(
        &self,
        _: &str,
        message: RenderedEmail,
    ) -> Result<String, EmailDeliveryError> {
        let token = message
            .text
            .lines()
            .filter_map(|line| url::Url::parse(line).ok())
            .find_map(|url| {
                url.query_pairs()
                    .find(|(key, _)| key == "token")
                    .map(|(_, value)| value.into_owned())
            })
            .expect("email link token");
        self.0.lock().unwrap().push(token);
        Ok("test-delivery".into())
    }
}

impl Mailbox {
    pub fn last(&self) -> String {
        self.0
            .lock()
            .unwrap()
            .last()
            .expect("email delivered")
            .clone()
    }

    pub fn len(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

pub async fn register_verified_user(
    store: &AuthStore,
    runtime: &RuntimeAuthConfig,
    mailbox: &Mailbox,
    email: &str,
    password: &str,
) -> Subject {
    register_password_user(store, runtime, mailbox, PasswordRegistrationInput { email })
        .await
        .expect("register fixture email");
    let outcome = verify_password_email(
        store,
        runtime,
        PasswordVerificationInput {
            token: &mailbox.last(),
            new_password: password,
        },
    )
    .await
    .expect("complete fixture registration");
    Subject::from_persisted(outcome.subject)
}
