//! OIDC provider-state store operations.

use super::{to_value, AuthStore};
use crate::error::StorageError;
use crate::storage::{TransactCondition, TransactOperation};
use crate::store::keys::StoreKey;
use crate::store::records::{ProviderStateBinding, ProviderStateRecord};
use serde_json::Value;
use subtle::ConstantTimeEq;

#[derive(Clone, Copy)]
pub enum ProviderStateProof<'a> {
    GoogleBrowser(&'a str),
    Apple,
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn validate(record: &ProviderStateRecord) -> Result<(), StorageError> {
    if !valid_digest(&record.session_lookup_digest)
        || record.pkce_verifier.is_empty()
        || record.nonce.is_empty()
        || record.created_at >= record.expires_at
        || matches!(&record.binding, ProviderStateBinding::Google { browser_digest } if !valid_digest(browser_digest))
    {
        return Err(StorageError::DynamoDB("invalid provider state".into()));
    }
    Ok(())
}

impl AuthStore {
    pub async fn create_provider_state(
        &self,
        state_digest: &str,
        record: ProviderStateRecord,
    ) -> Result<(), StorageError> {
        validate(&record)?;
        if !valid_digest(state_digest) {
            return Err(StorageError::DynamoDB("invalid provider state key".into()));
        }
        let key = StoreKey::provider_state(state_digest);
        let now = (self.provider_issuance_now)();
        if now < record.created_at || now >= record.expires_at {
            return Err(StorageError::ConditionFailed(
                "provider state expired".into(),
            ));
        }
        self.storage
            .transact(vec![TransactOperation::Put {
                key: key.parts(),
                value: to_value(&record)?,
                expiry: Some(record.expires_at),
                condition: Some(TransactCondition::NotExists),
            }])
            .await
    }

    pub async fn take_provider_state(
        &self,
        state_digest: &str,
        proof: ProviderStateProof<'_>,
    ) -> Result<Option<ProviderStateRecord>, StorageError> {
        if !valid_digest(state_digest) {
            return Ok(None);
        }
        let key = StoreKey::provider_state(state_digest);
        let original: Value = match self.get_record(&key).await? {
            Some(value) => value,
            None => return Ok(None),
        };
        let record: ProviderStateRecord = serde_json::from_value(original.clone())
            .map_err(|_| StorageError::DynamoDB("invalid provider state".into()))?;
        validate(&record)?;
        let matches = match (&record.binding, proof) {
            (
                ProviderStateBinding::Google { browser_digest },
                ProviderStateProof::GoogleBrowser(provided),
            ) => {
                valid_digest(provided)
                    && bool::from(browser_digest.as_bytes().ct_eq(provided.as_bytes()))
            }
            (ProviderStateBinding::Apple, ProviderStateProof::Apple) => true,
            _ => false,
        };
        let now = (self.provider_issuance_now)();
        if !matches || now < record.created_at || now >= record.expires_at {
            return Ok(None);
        }
        // The proof authorizes only this read, never a newer replacement or a retry plan.
        let operation = TransactOperation::Delete {
            key: key.parts(),
            condition: Some(TransactCondition::AttributeEquals {
                name: "value".to_string(),
                value: original,
            }),
        };
        if (self.provider_issuance_now)() >= record.expires_at {
            return Ok(None);
        }
        let result = self.storage.transact(vec![operation]).await;

        match result {
            Ok(()) => Ok(Some(record)),
            Err(StorageError::ConditionFailed(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }
}
