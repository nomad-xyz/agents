use std::{collections::HashMap, sync::Arc};

use ethers::{
    middleware::TimeLag,
    prelude::{Http, Provider as EthersProvider},
};

use nomad_xyz_configuration::NomadConfig;
use tokio::task::JoinHandle;

use agent_utils::{
    init::{config, networks_from_env, rpc_from_env},
    HomeReplicaMap, ProcessStep,
};

use crate::{
    domain::Domain,
    faucets::Faucets,
    metrics::Metrics,
    steps::{e2e::E2ELatency, terminal::Terminal},
    ArcProvider, DispatchFaucet, ProcessFaucet, RelayFaucet, UpdateFaucet,
};

pub(crate) fn provider_for(config: &NomadConfig, network: &str) -> eyre::Result<ArcProvider> {
    tracing::info!(network, "Instantiating provider");

    let url = rpc_from_env(network).or_else(|| {
        config
            .rpcs
            .get(network)
            .and_then(|set| set.iter().next().cloned())
    });

    eyre::ensure!(
        url.is_some(),
        "Missing Url. Please specify by config or env var."
    );

    let url = url.expect("checked on previous line");
    let provider = EthersProvider::<Http>::try_from(&url)?;

    let timelag = config
        .protocol()
        .networks
        .get(network)
        .expect("missing protocol block in config")
        .specs
        .finalization_blocks;

    tracing::debug!(url = url.as_str(), timelag, network, "Connect network");
    Ok(TimeLag::new(provider, timelag).into())
}

pub(crate) fn monitor() -> eyre::Result<Monitor> {
    Monitor::from_config(&config()?)
}

#[derive(Debug)]
pub(crate) struct Monitor {
    networks: HashMap<String, Domain>,
    metrics: Arc<Metrics>,
}

impl Monitor {
    pub(crate) fn from_config(config: &NomadConfig) -> eyre::Result<Self> {
        let mut networks = HashMap::new();
        let to_monitor =
            networks_from_env().unwrap_or_else(|| config.networks.iter().cloned().collect());
        for network in config.networks.iter().filter(|s| to_monitor.contains(s)) {
            networks.insert(
                network.to_owned(),
                Domain::from_config(config, network, &to_monitor)?,
            );
        }
        let metrics = Metrics::new()?.into();
        Ok(Monitor { networks, metrics })
    }

    pub(crate) fn run_http_server(&self) -> JoinHandle<()> {
        self.metrics.clone().run_http_server()
    }

    fn run_dispatch_producers(&self) -> HashMap<&str, DispatchFaucet> {
        let faucets: HashMap<_, _> = self
            .networks
            .iter()
            .map(|(network, domain)| (network.as_str(), domain.dispatch_producer()))
            .collect();
        tracing::debug!(count = faucets.len(), "running dispatch_producer");
        faucets
    }

    fn run_update_producers(&self) -> HashMap<&str, UpdateFaucet> {
        let faucets: HashMap<_, _> = self
            .networks
            .iter()
            .map(|(network, domain)| (network.as_str(), domain.update_producer()))
            .collect();
        tracing::debug!(count = faucets.len(), "running update_producer");
        faucets
    }

    fn run_relay_producers(&self) -> HomeReplicaMap<RelayFaucet> {
        let faucets: HashMap<_, _> = self
            .networks
            .iter()
            .map(|(network, domain)| (network.as_str(), domain.relay_producers()))
            .collect();
        tracing::debug!(count = faucets.len(), "running relay_producers");
        faucets
    }

    fn run_process_producers(&self) -> HomeReplicaMap<ProcessFaucet> {
        let faucets: HashMap<_, _> = self
            .networks
            .iter()
            .map(|(network, domain)| (network.as_str(), domain.process_producers()))
            .collect();
        tracing::debug!(count = faucets.len(), "running process_producers");
        faucets
    }

    pub(crate) fn producers(&self) -> Faucets {
        Faucets {
            dispatches: self.run_dispatch_producers(),
            updates: self.run_update_producers(),
            relays: self.run_relay_producers(),
            processes: self.run_process_producers(),
        }
    }

    #[tracing::instrument(skip_all, level = "debug")]
    fn run_between_dispatch<'a>(&'a self, faucets: &mut Faucets<'a>) {
        self.networks.iter().for_each(|(chain, domain)| {
            let emitter = domain.home_address();
            let event = "dispatch";

            let metrics = self.metrics.between_metrics(chain, event, &emitter, None);

            domain.count_dispatches(faucets, metrics, event);
        })
    }

    #[tracing::instrument(skip_all, level = "debug")]
    fn run_between_update<'a>(&'a self, faucets: &mut Faucets<'a>) {
        self.networks.iter().for_each(|(chain, domain)| {
            let emitter = format!("{:?}", domain.home().address());
            let event = "update";

            let metrics = self.metrics.between_metrics(chain, event, &emitter, None);

            domain.count_updates(faucets, metrics, event);
        })
    }

    #[tracing::instrument(skip_all, level = "debug")]
    fn run_between_relay<'a>(&'a self, faucets: &mut Faucets<'a>) {
        self.networks.values().for_each(|domain| {
            domain.count_relays(faucets, self.metrics.clone());
        });
    }

    #[tracing::instrument(skip_all, level = "debug")]
    fn run_between_process<'a>(&'a self, faucets: &mut Faucets<'a>) {
        self.networks.values().for_each(|domain| {
            domain.count_processes(faucets, self.metrics.clone());
        });
    }

    pub(crate) fn run_betweens<'a>(&'a self, faucets: &mut Faucets<'a>) {
        self.run_between_dispatch(faucets);
        self.run_between_update(faucets);
        self.run_between_relay(faucets);
        self.run_between_process(faucets);
    }

    #[tracing::instrument(skip_all, level = "debug")]
    pub(crate) fn run_dispatch_to_update<'a>(&'a self, faucets: &mut Faucets<'a>) {
        self.networks.values().for_each(|domain| {
            domain.dispatch_to_update(faucets, self.metrics.clone());
        });
    }

    pub(crate) fn run_update_to_relay<'a>(&'a self, faucets: &mut Faucets<'a>) {
        self.networks
            .values()
            .for_each(|v| v.update_to_relay(faucets, self.metrics.clone()));
    }

    pub(crate) fn run_relay_to_process<'a>(&'a self, faucets: &mut Faucets<'a>) {
        self.networks
            .values()
            .for_each(|domain| domain.relay_to_process(faucets, self.metrics.clone()));
    }

    pub(crate) fn run_e2e<'a>(&'a self, faucets: &mut Faucets<'a>) {
        let (process_sinks, process_faucets) = faucets.swap_all_processes();
        let (dispatch_sinks, dispatch_faucets) = faucets.swap_all_dispatches();

        let metrics = self
            .metrics
            .e2e_metrics(process_sinks.keys().map(AsRef::as_ref));

        let domain_to_network = self
            .networks
            .values()
            .map(|domain| (domain.domain_number, domain.name().to_owned()))
            .collect();

        E2ELatency::new(
            dispatch_faucets,
            process_faucets,
            domain_to_network,
            metrics,
            dispatch_sinks,
            process_sinks,
        )
        .run_until_panic();
    }

    /// take ownership of all faucets and terminate them
    pub(crate) fn run_terminals<'a>(&'a self, faucets: Faucets<'a>) -> Vec<JoinHandle<()>> {
        let mut tasks = vec![];

        faucets.dispatches.into_iter().for_each(|(_, v)| {
            tasks.push(Terminal::new(v).run_until_panic());
        });

        faucets.updates.into_iter().for_each(|(_, v)| {
            tasks.push(Terminal::new(v).run_until_panic());
        });

        faucets.relays.into_iter().for_each(|(_, v)| {
            v.into_iter().for_each(|(_, v)| {
                tasks.push(Terminal::new(v).run_until_panic());
            });
        });

        faucets.processes.into_iter().for_each(|(_, v)| {
            v.into_iter().for_each(|(_, v)| {
                tasks.push(Terminal::new(v).run_until_panic());
            });
        });

        tasks
    }
}
