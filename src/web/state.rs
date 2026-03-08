use crate::config::SharedRuntimeConfig;
use crate::metadata::MetadataStore;

#[derive(Clone)]
pub struct MgmtState {
    pub config: SharedRuntimeConfig,
    pub store: MetadataStore,
    pub mgmt_password: Option<String>,
}
