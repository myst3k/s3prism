use std::sync::Arc;

use crate::backend::client::SiteClient;
use crate::config::SharedRuntimeConfig;
use crate::metadata::MetadataStore;

#[derive(Clone)]
pub struct AppState {
    pub config: SharedRuntimeConfig,
    pub store: MetadataStore,
    pub clients: Vec<SiteClient>,
    pub upload_semaphore: Arc<tokio::sync::Semaphore>,
    pub purge_notify: Arc<tokio::sync::Notify>,
}
