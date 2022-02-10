use std::io;
use redis::{Client, AsyncCommands, IntoConnectionInfo, RedisError, RedisResult};
use redis::aio::Connection;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug)]
pub struct RedisSessionStore {
    client: Client,
}

#[derive(Serialize, Deserialize)]
pub struct Session {
    sid: String,
    pub(crate) credentials: String,
}

#[derive(Debug)]
pub enum MyError {
    Io(io::Error),
    Json(serde_json::Error),
    Redis(RedisError)
}

impl From<serde_json::Error> for MyError {
    fn from(err: serde_json::Error) -> MyError {
        use serde_json::error::Category;
        match err.classify() {
            Category::Io => {
                MyError::Io(err.into())
            }
            Category::Syntax | Category::Data | Category::Eof => {
                MyError::Json(err)
            }
        }
    }
}

impl From<RedisError> for MyError {
    fn from(err: RedisError) -> Self {
        MyError::Redis(err)
    }
}

impl RedisSessionStore {
    pub(crate) async fn get(&self, sid: String) -> Result<Option<Session>, MyError> {
        let mut connection = self.connection().await?;
        let session_str: Option<String> = connection.get(sid).await?;
        match session_str {
            Some(json) => Ok(serde_json::from_str(&json)?),
            None => Ok(None)
        }
    }
    #[cfg(test)]
    async fn set(&self, session: Session) -> Result<(), MyError> {
        let session_str = serde_json::to_string(&session)?;
        let mut connection = self.connection().await?;
        connection.set(session.sid, session_str).await?;
        Ok(())
    }
    pub fn new(connection_info: impl IntoConnectionInfo) -> RedisResult<Self> {
        Ok(Self {client: Client::open(connection_info)?})
    }
    async fn connection(&self) -> RedisResult<Connection> {
        self.client.get_async_connection().await
    }
    async fn clear_store(&self, keys: &[&str]) -> Result<(), MyError> {
        let mut connection = self.connection().await?;
        for key in keys {
            connection.del(key).await?
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn get_unknown_key() {
        assert!(create_store().await.get(String::from("unknown")).await.unwrap().is_none())
    }

    #[tokio::test]
    async fn get_session() {
        let store = create_store().await;
        store.set(Session {sid: String::from("sid"), credentials: String::from("credentials") }).await.unwrap();

        let session = store.get(String::from("sid")).await.unwrap().unwrap();

        assert_eq!(session.sid, "sid");
        assert_eq!(session.credentials, "credentials");
    }

    async fn create_store() -> RedisSessionStore {
        let store = RedisSessionStore::new("redis://redis/1").unwrap();
        store.clear_store(&["sid"]).await.unwrap();
        store
    }
}