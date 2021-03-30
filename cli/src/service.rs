// This file is part of Substrate.

// Copyright (C) 2018-2020 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![warn(unused_extern_crates)]

//! Service implementation. Specialized wrapper over substrate service.
use cumulus_client_consensus_relay_chain::{
    build_relay_chain_consensus, BuildRelayChainConsensusParams
};
use futures::prelude::*;
use parami_node_executor::Executor;
use parami_node_primitives::Block;
use polkadot_primitives::v0::CollatorPair;
use cumulus_primitives_core::ParaId;
use parami_node_rpc as node_rpc;
use sp_trie::PrefixedMemoryDB;
use cumulus_client_service::{
    prepare_node_config, start_collator, start_full_node, StartCollatorParams, StartFullNodeParams,
};
use parami_node_runtime::RuntimeApi;
use sc_client_api::{ExecutorProvider, RemoteBackend};
use sc_consensus_babe;
use sc_network::{Event, NetworkService};
use cumulus_client_network::build_block_announce_validator;
use sc_service::{config::Configuration, error::Error as ServiceError, RpcHandlers, TaskManager, TFullClient, Role, PartialComponents, TFullBackend};
use sc_telemetry::TelemetryConnectionNotifier;
use sp_inherents::InherentDataProviders;
use sp_runtime::traits::Block as BlockT;
use sp_runtime::traits::BlakeTwo256;
use std::sync::Arc;
use sc_telemetry::TelemetrySpan;
type FullClient = sc_service::TFullClient<Block, RuntimeApi, Executor>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;
type FullGrandpaBlockImport =
    grandpa::GrandpaBlockImport<FullBackend, Block, FullClient, FullSelectChain>;
type LightClient = sc_service::TLightClient<Block, RuntimeApi, Executor>;

/// Starts a `ServiceBuilder` for a full service.
///
/// Use this macro if you don't actually need the full service, but just the builder in order to
/// be able to perform chain operations.
pub fn new_partial(
    config: &Configuration,
) -> Result<
    PartialComponents<
        TFullClient<Block, RuntimeApi, Executor>,
        TFullBackend<Block>,
        (),
        sp_consensus::import_queue::BasicQueue<Block, PrefixedMemoryDB<BlakeTwo256>>,
        sc_transaction_pool::FullPool<Block, TFullClient<Block, RuntimeApi, Executor>>,
        (),
    >,
    sc_service::Error,
> {
    let inherent_data_providers = sp_inherents::InherentDataProviders::new();

    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts::<Block, RuntimeApi, Executor>(&config)?;
    let client = Arc::new(client);

    let registry = config.prometheus_registry();

    let transaction_pool = sc_transaction_pool::BasicPool::new_full(
        config.transaction_pool.clone(),
        config.role.is_authority().into(),
        config.prometheus_registry(),
        task_manager.spawn_handle(),
        client.clone(),
    );

    let import_queue = cumulus_client_consensus_relay_chain::import_queue(
        client.clone(),
        client.clone(),
        inherent_data_providers.clone(),
        &task_manager.spawn_essential_handle(),
        registry.clone(),
    )?;

    let params = PartialComponents {
        backend,
        client,
        import_queue,
        keystore_container,
        task_manager,
        transaction_pool,
        inherent_data_providers,
        select_chain: (),
        other: (),
    };

    Ok(params)
}

pub struct NewFullBase {
    pub task_manager: TaskManager,
    pub inherent_data_providers: InherentDataProviders,
    pub client: Arc<FullClient>,
    pub network: Arc<NetworkService<Block, <Block as BlockT>::Hash>>,
    pub network_status_sinks: sc_service::NetworkStatusSinks<Block>,
    pub transaction_pool: Arc<sc_transaction_pool::FullPool<Block, FullClient>>,
}

/// Creates a full service from the configuration.
pub fn new_full_base(
    mut config: Configuration,
    with_startup_data: impl FnOnce(
        &sc_consensus_babe::BabeBlockImport<Block, FullClient, FullGrandpaBlockImport>,
        &sc_consensus_babe::BabeLink<Block>,
    ),
) -> Result<NewFullBase, ServiceError> {
    let sc_service::PartialComponents {
        client,
        backend,
        mut task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        inherent_data_providers,
        other: (rpc_extensions_builder, import_setup, rpc_setup),
    } = new_partial(&config)?;

    let shared_voter_state = rpc_setup;

    config
        .network
        .extra_sets
        .push(grandpa::grandpa_peers_set_config());

    #[cfg(feature = "cli")]
    config.network.request_response_protocols.push(
        sc_finality_grandpa_warp_sync::request_response_config_for_chain(
            &config,
            task_manager.spawn_handle(),
            backend.clone(),
        ),
    );

    let (network, network_status_sinks, system_rpc_tx, network_starter) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            on_demand: None,
            block_announce_validator_builder: None,
        })?;

    if config.offchain_worker.enabled {
        sc_service::build_offchain_workers(
            &config,
            backend.clone(),
            task_manager.spawn_handle(),
            client.clone(),
            network.clone(),
        );
    }

    let role = config.role.clone();
    let force_authoring = config.force_authoring;
    let backoff_authoring_blocks =
        Some(sc_consensus_slots::BackoffAuthoringOnFinalizedHeadLagging::default());
    let name = config.network.node_name.clone();
    let enable_grandpa = !config.disable_grandpa;
    let prometheus_registry = config.prometheus_registry().cloned();

    let (_rpc_handlers, telemetry_connection_notifier) =
        sc_service::spawn_tasks(sc_service::SpawnTasksParams {
            config,
            backend: backend.clone(),
            client: client.clone(),
            keystore: keystore_container.sync_keystore(),
            network: network.clone(),
            rpc_extensions_builder: Box::new(rpc_extensions_builder),
            transaction_pool: transaction_pool.clone(),
            task_manager: &mut task_manager,
            on_demand: None,
            remote_blockchain: None,
            network_status_sinks: network_status_sinks.clone(),
            system_rpc_tx,
        })?;

    let (block_import, grandpa_link, babe_link) = import_setup;

    (with_startup_data)(&block_import, &babe_link);

    if let sc_service::config::Role::Authority { .. } = &role {
        let proposer = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool.clone(),
            prometheus_registry.as_ref(),
        );

        let can_author_with =
            sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());

        let babe_config = sc_consensus_babe::BabeParams {
            keystore: keystore_container.sync_keystore(),
            client: client.clone(),
            select_chain,
            env: proposer,
            block_import,
            sync_oracle: network.clone(),
            inherent_data_providers: inherent_data_providers.clone(),
            force_authoring,
            backoff_authoring_blocks,
            babe_link,
            can_author_with,
        };

        let babe = sc_consensus_babe::start_babe(babe_config)?;
        task_manager
            .spawn_essential_handle()
            .spawn_blocking("babe-proposer", babe);
    }

    // Spawn authority discovery module.
    if role.is_authority() {
        let authority_discovery_role =
            sc_authority_discovery::Role::PublishAndDiscover(keystore_container.keystore());
        let dht_event_stream =
            network
                .event_stream("authority-discovery")
                .filter_map(|e| async move {
                    match e {
                        Event::Dht(e) => Some(e),
                        _ => None,
                    }
                });
        let (authority_discovery_worker, _service) = sc_authority_discovery::new_worker_and_service(
            client.clone(),
            network.clone(),
            Box::pin(dht_event_stream),
            authority_discovery_role,
            prometheus_registry.clone(),
        );

        task_manager.spawn_handle().spawn(
            "authority-discovery-worker",
            authority_discovery_worker.run(),
        );
    }

    // if the node isn't actively participating in consensus then it doesn't
    // need a keystore, regardless of which protocol we use below.
    let keystore = if role.is_authority() {
        Some(keystore_container.sync_keystore())
    } else {
        None
    };

    let config = grandpa::Config {
        // FIXME #1578 make this available through chainspec
        gossip_duration: std::time::Duration::from_millis(333),
        justification_period: 1,
        name: Some(name),
        observer_enabled: false,
        keystore,
        is_authority: role.is_network_authority(),
    };

    if enable_grandpa {
        // start the full GRANDPA voter
        // NOTE: non-authorities could run the GRANDPA observer protocol, but at
        // this point the full voter should provide better guarantees of block
        // and vote data availability than the observer. The observer has not
        // been tested extensively yet and having most nodes in a network run it
        // could lead to finality stalls.
        let grandpa_config = grandpa::GrandpaParams {
            config,
            link: grandpa_link,
            network: network.clone(),
            telemetry_on_connect: telemetry_connection_notifier.map(|x| x.on_connect_stream()),
            voting_rule: grandpa::VotingRulesBuilder::default().build(),
            prometheus_registry,
            shared_voter_state,
        };

        // the GRANDPA voter task is considered infallible, i.e.
        // if it fails we take down the service with it.
        task_manager
            .spawn_essential_handle()
            .spawn_blocking("grandpa-voter", grandpa::run_grandpa_voter(grandpa_config)?);
    }

    network_starter.start_network();
    Ok(NewFullBase {
        task_manager,
        inherent_data_providers,
        client,
        network,
        network_status_sinks,
        transaction_pool,
    })
}

/// Builds a new service for a full client.
pub fn new_full(config: Configuration) -> Result<TaskManager, ServiceError> {
    new_full_base(config, |_, _| ()).map(|NewFullBase { task_manager, .. }| task_manager)
}

pub fn new_light_base(
    config: Configuration,
) -> Result<
    (
        TaskManager,
        RpcHandlers,
        Option<TelemetryConnectionNotifier>,
        Arc<LightClient>,
        Arc<NetworkService<Block, <Block as BlockT>::Hash>>,
        Arc<
            sc_transaction_pool::LightPool<Block, LightClient, sc_network::config::OnDemand<Block>>,
        >,
    ),
    ServiceError,
> {
    let (client, backend, keystore_container, mut task_manager, on_demand) =
        sc_service::new_light_parts::<Block, RuntimeApi, Executor>(&config)?;

    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let transaction_pool = Arc::new(sc_transaction_pool::BasicPool::new_light(
        config.transaction_pool.clone(),
        config.prometheus_registry(),
        task_manager.spawn_handle(),
        client.clone(),
        on_demand.clone(),
    ));

    let (grandpa_block_import, _) = grandpa::block_import(
        client.clone(),
        &(client.clone() as Arc<_>),
        select_chain.clone(),
    )?;
    let justification_import = grandpa_block_import.clone();

    let (babe_block_import, babe_link) = sc_consensus_babe::block_import(
        sc_consensus_babe::Config::get_or_compute(&*client)?,
        grandpa_block_import,
        client.clone(),
    )?;

    let inherent_data_providers = sp_inherents::InherentDataProviders::new();

    let import_queue = sc_consensus_babe::import_queue(
        babe_link,
        babe_block_import,
        Some(Box::new(justification_import)),
        client.clone(),
        select_chain.clone(),
        inherent_data_providers.clone(),
        &task_manager.spawn_handle(),
        config.prometheus_registry(),
        sp_consensus::NeverCanAuthor,
    )?;

    let (network, network_status_sinks, system_rpc_tx, network_starter) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            on_demand: Some(on_demand.clone()),
            block_announce_validator_builder: None,
        })?;
    network_starter.start_network();

    if config.offchain_worker.enabled {
        sc_service::build_offchain_workers(
            &config,
            backend.clone(),
            task_manager.spawn_handle(),
            client.clone(),
            network.clone(),
        );
    }

    let light_deps = node_rpc::LightDeps {
        remote_blockchain: backend.remote_blockchain(),
        fetcher: on_demand.clone(),
        client: client.clone(),
        pool: transaction_pool.clone(),
    };

    let rpc_extensions = node_rpc::create_light(light_deps);

    let (rpc_handlers, telemetry_connection_notifier) =
        sc_service::spawn_tasks(sc_service::SpawnTasksParams {
            on_demand: Some(on_demand),
            remote_blockchain: Some(backend.remote_blockchain()),
            rpc_extensions_builder: Box::new(sc_service::NoopRpcExtensionBuilder(rpc_extensions)),
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            keystore: keystore_container.sync_keystore(),
            config,
            backend,
            network_status_sinks,
            system_rpc_tx,
            network: network.clone(),
            task_manager: &mut task_manager,
        })?;

    Ok((
        task_manager,
        rpc_handlers,
        telemetry_connection_notifier,
        client,
        network,
        transaction_pool,
    ))
}



/// Builds a new service for a light client.
pub fn new_light(config: Configuration) -> Result<TaskManager, ServiceError> {
    new_light_base(config).map(|(task_manager, _, _, _, _, _)| task_manager)
}

/// Start a node with the given parachain `Configuration` and relay chain `Configuration`.
///
/// This is the actual implementation that is abstract over the executor and the runtime api.
#[sc_tracing::logging::prefix_logs_with("Parachain")]
async fn start_node_impl<RB>(
    parachain_config: Configuration,
    collator_key: CollatorPair,
    polkadot_config: Configuration,
    id: ParaId,
    validator: bool,
    rpc_ext_builder: RB,
) -> sc_service::error::Result<(TaskManager, Arc<TFullClient<Block, RuntimeApi, Executor>>)>
    where
        RB: Fn(
            Arc<TFullClient<Block, RuntimeApi, Executor>>,
        ) -> jsonrpc_core::IoHandler<sc_rpc::Metadata>
        + Send
        + 'static,
{
    if matches!(parachain_config.role, Role::Light) {
        return Err("Light client not supported!".into());
    }

    let parachain_config = prepare_node_config(parachain_config);

    let polkadot_full_node =
        cumulus_client_service::build_polkadot_full_node(polkadot_config, collator_key.public())
            .map_err(|e| match e {
                polkadot_service::Error::Sub(x) => x,
                s => format!("{}", s).into(),
            })?;

    let params = new_partial(&parachain_config)?;
    params
        .inherent_data_providers
        .register_provider(sp_timestamp::InherentDataProvider)
        .unwrap();

    let client = params.client.clone();
    let backend = params.backend.clone();
    let block_announce_validator = build_block_announce_validator(
        polkadot_full_node.client.clone(),
        id,
        Box::new(polkadot_full_node.network.clone()),
        polkadot_full_node.backend.clone(),
    );

    let prometheus_registry = parachain_config.prometheus_registry().cloned();
    let transaction_pool = params.transaction_pool.clone();
    let mut task_manager = params.task_manager;
    let import_queue = params.import_queue;
    let (network, network_status_sinks, system_rpc_tx, start_network) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &parachain_config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            on_demand: None,
            block_announce_validator_builder: Some(Box::new(|_| block_announce_validator)),
        })?;

    let rpc_client = client.clone();
    let rpc_extensions_builder = Box::new(move |_, _| rpc_ext_builder(rpc_client.clone()));

    let telemetry_span = TelemetrySpan::new();
    let _telemetry_span_entered = telemetry_span.enter();

    if parachain_config.offchain_worker.enabled {
        sc_service::build_offchain_workers(
            &parachain_config, backend.clone(), task_manager.spawn_handle(), client.clone(), network.clone(),
        );
    }

    sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        on_demand: None,
        remote_blockchain: None,
        rpc_extensions_builder,
        client: client.clone(),
        transaction_pool: transaction_pool.clone(),
        task_manager: &mut task_manager,
        config: parachain_config,
        keystore: params.keystore_container.sync_keystore(),
        backend: backend.clone(),
        network: network.clone(),
        network_status_sinks,
        system_rpc_tx,
        telemetry_span: Some(telemetry_span.clone()),
    })?;

    let announce_block = {
        let network = network.clone();
        Arc::new(move |hash, data| network.announce_block(hash, Some(data)))
    };

    if validator {
        let proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool,
            prometheus_registry.as_ref(),
        );
        let spawner = task_manager.spawn_handle();

        let parachain_consensus = build_relay_chain_consensus(BuildRelayChainConsensusParams {
            para_id: id,
            proposer_factory,
            inherent_data_providers: params.inherent_data_providers,
            block_import: client.clone(),
            relay_chain_client: polkadot_full_node.client.clone(),
            relay_chain_backend: polkadot_full_node.backend.clone(),
        });

        let params = StartCollatorParams {
            para_id: id,
            block_status: client.clone(),
            announce_block,
            client: client.clone(),
            task_manager: &mut task_manager,
            collator_key,
            relay_chain_full_node: polkadot_full_node,
            spawner,
            backend,
            parachain_consensus,
        };

        start_collator(params).await?;
    } else {
        let params = StartFullNodeParams {
            client: client.clone(),
            announce_block,
            task_manager: &mut task_manager,
            para_id: id,
            polkadot_full_node,
        };

        start_full_node(params)?;
    }

    start_network.start_network();

    Ok((task_manager, client))
}

/// Start a normal parachain node.
pub async fn start_node(
    parachain_config: Configuration,
    collator_key: CollatorPair,
    polkadot_config: Configuration,
    id: ParaId,
    validator: bool,
) -> sc_service::error::Result<(TaskManager, Arc<TFullClient<Block, RuntimeApi, Executor>>)> {
    start_node_impl(
        parachain_config,
        collator_key,
        polkadot_config,
        id,
        validator,
        |_| Default::default(),
    )
        .await
}