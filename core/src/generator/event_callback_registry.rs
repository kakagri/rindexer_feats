use crate::manifest::yaml::EventInputIndexedFilters;
use crate::provider::JsonRpcCachedProvider;
use ethers::addressbook::Address;
use ethers::contract::LogMeta;
use ethers::prelude::ValueOrArray;
use ethers::types::{Bytes, Log, H256, U256, U64};
use futures::future::BoxFuture;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::{any::Any, sync::Arc};
use tokio::time::sleep;
use tracing::{debug, error};

pub type Decoder = Arc<dyn Fn(Vec<H256>, Bytes) -> Arc<dyn Any + Send + Sync> + Send + Sync>;

pub fn noop_decoder() -> Decoder {
    Arc::new(move |_topics: Vec<H256>, _data: Bytes| {
        Arc::new(String::new()) as Arc<dyn Any + Send + Sync>
    }) as Decoder
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AddressDetails {
    pub address: ValueOrArray<Address>,

    pub indexed_filters: Option<Vec<EventInputIndexedFilters>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FactoryDetails {
    pub address: String,

    #[serde(rename = "eventName")]
    pub event_name: String,

    #[serde(rename = "parameterName")]
    pub parameter_name: String,

    pub abi: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FilterDetails {
    pub event_name: String,

    pub indexed_filters: Option<EventInputIndexedFilters>,
}

#[derive(Clone)]
pub enum IndexingContractSetup {
    Address(AddressDetails),
    Filter(FilterDetails),
    Factory(FactoryDetails),
}

impl IndexingContractSetup {
    pub fn is_filter(&self) -> bool {
        matches!(self, IndexingContractSetup::Filter(_))
    }
}

/// Represents a contract on a specific network with its setup and associated provider.
#[derive(Clone)]
pub struct NetworkContract {
    pub id: String,
    pub network: String,
    pub indexing_contract_setup: IndexingContractSetup,
    pub cached_provider: Arc<JsonRpcCachedProvider>,
    pub decoder: Decoder,
    pub start_block: Option<U64>,
    pub end_block: Option<U64>,
}

impl NetworkContract {
    pub fn decode_log(&self, log: Log) -> Arc<dyn Any + Send + Sync> {
        (self.decoder)(log.topics, log.data)
    }
}

#[derive(Clone)]
pub struct ContractInformation {
    pub name: String,
    pub details: Vec<NetworkContract>,
    pub abi: String,
    pub reorg_safe_distance: bool,
}

#[derive(Debug, Clone)]
pub struct TxInformation {
    pub network: String,
    pub address: Address,
    pub block_hash: H256,
    pub block_number: U64,
    pub transaction_hash: H256,
    pub log_index: U256,
    pub transaction_index: U64,
}

#[derive(Debug, Clone)]
pub struct EventResult {
    pub log: Log,
    pub decoded_data: Arc<dyn Any + Send + Sync>,
    pub tx_information: TxInformation,
}

impl EventResult {
    pub fn new(network_contract: Arc<NetworkContract>, log: &Log) -> Self {
        let log_meta = LogMeta::from(log);
        Self {
            log: log.clone(),
            decoded_data: network_contract.decode_log(log.clone()),
            tx_information: TxInformation {
                network: network_contract.network.to_string(),
                address: log.address,
                block_hash: log_meta.block_hash,
                block_number: log_meta.block_number,
                transaction_hash: log_meta.transaction_hash,
                transaction_index: log_meta.transaction_index,
                log_index: log_meta.log_index,
            },
        }
    }
}

pub type EventCallbackResult<T> = Result<T, String>;
pub type EventCallbackType =
    Arc<dyn Fn(Vec<EventResult>) -> BoxFuture<'static, EventCallbackResult<()>> + Send + Sync>;

pub struct EventInformation {
    pub indexer_name: String,
    pub topic_id: String,
    pub event_name: String,
    pub index_event_in_order: bool,
    pub contract: ContractInformation,
    pub callback: EventCallbackType,
}

impl EventInformation {
    pub fn info_log_name(&self) -> String {
        format!("{}::{}", self.contract.name, self.event_name)
    }
}

impl Clone for EventInformation {
    fn clone(&self) -> Self {
        EventInformation {
            indexer_name: self.indexer_name.clone(),
            topic_id: self.topic_id.clone(),
            event_name: self.event_name.clone(),
            index_event_in_order: self.index_event_in_order,
            contract: self.contract.clone(),
            callback: Arc::clone(&self.callback),
        }
    }
}

#[derive(Clone)]
pub struct EventCallbackRegistry {
    pub events: Vec<EventInformation>,
}

impl Default for EventCallbackRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl EventCallbackRegistry {
    pub fn new() -> Self {
        EventCallbackRegistry { events: Vec::new() }
    }

    pub fn find_event(&self, topic_id: &str) -> Option<&EventInformation> {
        self.events.iter().find(|e| e.topic_id == topic_id)
    }

    pub fn register_event(&mut self, event: EventInformation) {
        self.events.push(event);
    }

    pub async fn trigger_event(&self, topic_id: &str, data: Vec<EventResult>) {
        let mut attempts = 0;
        let mut delay = Duration::from_millis(100);

        if let Some(event_information) = self.find_event(topic_id) {
            debug!(
                "{} - Pushed {} events",
                data.len(),
                event_information.info_log_name()
            );

            loop {
                match (event_information.callback)(data.clone()).await {
                    Ok(_) => {
                        debug!("Event processing succeeded for topic_id: {}", topic_id);
                        break;
                    }
                    Err(e) => {
                        attempts += 1;
                        error!(
                            "{} Event processing failed - topic_id: {}. Retrying... (attempt {}). Error: {}",
                            event_information.info_log_name(), topic_id, attempts, e
                        );

                        // Exponential backoff logic
                        sleep(delay).await;
                        delay = (delay * 2).min(Duration::from_secs(15)); // Max delay of 15 seconds

                        // add some jitter to the delay to avoid thundering herd problem
                        let jitter = Duration::from_millis(rand::thread_rng().gen_range(0..1000));
                        sleep(delay + jitter).await;
                    }
                }
            }
        } else {
            error!(
                "EventCallbackRegistry: No event found for topic_id: {}",
                topic_id
            );
        }
    }

    pub fn complete(&self) -> Arc<Self> {
        Arc::new(self.clone())
    }
}
