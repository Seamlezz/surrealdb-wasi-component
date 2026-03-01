use std::any::type_name;

use anyhow::{Context, Result, anyhow};
use ciborium::into_writer;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::bindings::seamlezz::surrealdb::call;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveAction {
    Create,
    Update,
    Delete,
    Killed,
}

#[derive(Debug, Clone)]
pub struct LiveEvent {
    pub subscription_id: u64,
    pub query_id: String,
    pub action: LiveAction,
    pub data: Vec<u8>,
}

impl LiveEvent {
    pub fn parse<D: DeserializeOwned>(&self) -> Result<D> {
        ciborium::from_reader::<D, _>(self.data.as_slice()).with_context(|| {
            format!(
                "failed to parse live event data into type {}",
                type_name::<D>()
            )
        })
    }
}

impl From<call::LiveAction> for LiveAction {
    fn from(value: call::LiveAction) -> Self {
        match value {
            call::LiveAction::Create => Self::Create,
            call::LiveAction::Update => Self::Update,
            call::LiveAction::Delete => Self::Delete,
            call::LiveAction::Killed => Self::Killed,
        }
    }
}

impl From<call::LiveEvent> for LiveEvent {
    fn from(value: call::LiveEvent) -> Self {
        Self {
            subscription_id: value.subscription_id,
            query_id: value.query_id,
            action: value.action.into(),
            data: value.data,
        }
    }
}

pub struct LiveSubscription {
    subscription_id: u64,
    stream: wit_bindgen::rt::async_support::StreamReader<call::LiveEvent>,
}

impl LiveSubscription {
    pub fn id(&self) -> u64 {
        self.subscription_id
    }

    pub async fn next_event(&mut self) -> Result<Option<LiveEvent>> {
        let Some(event) = self.stream.next().await else {
            return Ok(None);
        };

        Ok(Some(event.into()))
    }

    pub async fn cancel(self) -> Result<()> {
        call::cancel(self.subscription_id)
            .await
            .map_err(|error| anyhow!(error))?;

        Ok(())
    }
}

pub struct LiveQuery<'a> {
    query_str: &'a str,
    params: Vec<(String, Vec<u8>)>,
    bind_error: Option<anyhow::Error>,
}

impl<'a> LiveQuery<'a> {
    pub fn bind<T: Serialize>(mut self, key: impl Into<String>, value: T) -> Self {
        if self.bind_error.is_some() {
            return self;
        }

        let key = key.into();
        let mut serialized = Vec::new();

        if let Err(error) = into_writer(&value, &mut serialized)
            .with_context(|| format!("failed to bind key {} with type {}", key, type_name::<T>()))
        {
            self.bind_error = Some(error);
            return self;
        }

        self.params.push((key, serialized));
        self
    }

    pub async fn execute(self) -> Result<LiveSubscription> {
        if let Some(error) = self.bind_error {
            return Err(error);
        }

        let (subscription_id, stream) =
            call::subscribe(self.query_str.to_string(), self.params).await;
        Ok(LiveSubscription {
            subscription_id,
            stream,
        })
    }
}

pub fn subscribe(query_str: &str) -> LiveQuery<'_> {
    LiveQuery {
        query_str,
        params: Vec::new(),
        bind_error: None,
    }
}
