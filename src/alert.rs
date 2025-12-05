use tgbot::{
    api::{Client, ClientError, ExecuteError},
    types::{ChatId, SendMessage},
};
use uuid::Uuid;

use crate::HandleSubchainError;

pub struct TgAlert {
    client: Option<Client>,
    chat_id: ChatId,
}

impl TgAlert {
    pub fn new_client(
        tg_bot_token: impl AsRef<str>,
        tg_chat_id: impl AsRef<str>,
    ) -> Result<Self, ClientError> {
        let client = Some(Client::new(tg_bot_token.as_ref())?);
        let chat_id = tg_chat_id.as_ref().into();
        Ok(Self { client, chat_id })
    }

    pub fn new_empty() -> Self {
        Self {
            client: None,
            chat_id: "".into(),
        }
    }

    pub async fn notify_subchain_creation_error(
        &self,
        uuid: Uuid,
        error: &HandleSubchainError,
    ) -> Result<(), ExecuteError> {
        let message = format!("Create Subchain Error\n\nid: {uuid}\n\n{error}");
        self.notify(&message).await
    }

    async fn notify(&self, message: &str) -> Result<(), ExecuteError> {
        if let Some(client) = &self.client {
            return client
                .execute(SendMessage::new(self.chat_id.clone(), message.to_string()))
                .await
                .map(drop);
        }
        Ok(())
    }
}
