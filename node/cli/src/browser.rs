///////////////////////////////////////////////////////////////////////////////
//
//  Copyright 2018-2020 Airalab <research@aira.life>
//
//  Licensed under the Apache License, Version 2.0 (the "License");
//  you may not use this file except in compliance with the License.
//  You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
//
///////////////////////////////////////////////////////////////////////////////

use crate::chain_spec::ChainSpec;
use browser_utils::{
    browser_configuration, init_console_log, set_console_error_panic_hook, Client,
};
use log::info;
use std::str::FromStr;
use wasm_bindgen::prelude::*;

/// Starts the client.
#[wasm_bindgen]
pub async fn start_client(
    chain_spec: Option<String>,
    log_level: String,
) -> Result<Client, JsValue> {
    start_inner(chain_spec, log_level)
        .await
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

async fn start_inner(
    chain_spec: Option<String>,
    log_level: String,
) -> Result<Client, Box<dyn std::error::Error>> {
    set_console_error_panic_hook();
    init_console_log(log::Level::from_str(&log_level)?)?;
    let chain_spec = match chain_spec {
        Some(chain_spec) => ChainSpec::from_json_bytes(chain_spec.as_bytes().to_vec())
            .map_err(|e| format!("{:?}", e))?,
        None => crate::chain_spec::development_config(),
    };

    let config = browser_configuration(chain_spec).await?;

    info!("Substrate browser node");
    info!("✌️  version {}", config.impl_version);
    info!("❤️  by Parity Technologies, 2017-2020");
    info!("📋 Chain specification: {}", config.chain_spec.name());
    info!("🏷  Node name: {}", config.network.node_name);
    info!("👤 Role: {:?}", config.role);

    // Create the service. This is the most heavy initialization step.
    let (task_manager, rpc_handlers) = crate::service::new_light_base(config)
        .map(|(components, rpc_handlers, _, _, _)| (components, rpc_handlers))
        .map_err(|e| format!("{:?}", e))?;

    Ok(browser_utils::start_client(task_manager, rpc_handlers))
}
