use std::{collections::HashMap, sync::Arc};

use agent_utils::ProcessStep;
use nomad_ethereum::bindings::{home::Home, replica::Replica};
use nomad_xyz_configuration::{contracts::CoreContracts, NomadConfig};
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    faucets::Faucets,
    init::provider_for,
    metrics::Metrics,
    steps::{
        between::{BetweenEvents, BetweenMetrics},
        dispatch_wait::DispatchWait,
        producer::{DispatchProducer, ProcessProducer, RelayProducer, UpdateProducer},
        relay_wait::RelayWait,
        update_wait::UpdateWait,
    },
    DispatchFaucet, ProcessFaucet, Provider, RelayFaucet, UpdateFaucet,
};

#[derive(Debug)]
pub(crate) struct Domain {
    pub(crate) network: String,
    pub(crate) domain_number: u32,
    pub(crate) home: Home<Provider>,
    pub(crate) replicas: HashMap<String, Replica<Provider>>,
}

impl Domain {
    pub(crate) fn home_address(&self) -> String {
        format!("{:?}", self.home.address())
    }

    pub(crate) fn from_config(
        config: &NomadConfig,
        network: &str,
        to_monitor: &[String],
    ) -> eyre::Result<Self> {
        let network = network.to_owned();
        let provider = provider_for(config, &network)?;

        let domain_number = config
            .protocol()
            .resolve_name_to_domain(&network)
            .expect("invalid config");

        let CoreContracts::Evm(core) = config.core().get(&network).expect("invalid config");

        let home = Home::new(core.home.proxy.as_ethereum_address()?, provider.clone());

        let replicas = core
            .replicas
            .iter()
            .filter(|(s, _)| to_monitor.contains(s))
            .map(|(k, v)| {
                let replica = Replica::new(
                    v.proxy.as_ethereum_address().expect("invalid addr"),
                    provider.clone(),
                );
                (k.clone(), replica)
            })
            .collect();

        Ok(Domain {
            network,
            home,
            replicas,
            domain_number,
        })
    }

    pub(crate) fn name(&self) -> &str {
        self.network.as_ref()
    }

    pub(crate) fn home(&self) -> &Home<Provider> {
        &self.home
    }

    pub(crate) fn replicas(&self) -> &HashMap<String, Replica<Provider>> {
        &self.replicas
    }

    pub(crate) fn dispatch_producer(&self) -> DispatchFaucet {
        let (tx, rx) = unbounded_channel();

        DispatchProducer::new(self.home.clone(), &self.network, tx).run_until_panic();

        rx
    }

    pub(crate) fn update_producer(&self) -> UpdateFaucet {
        let (tx, rx) = unbounded_channel();

        UpdateProducer::new(self.home.clone(), &self.network, tx).run_until_panic();

        rx
    }

    pub fn relay_producer_for(&self, replica: &Replica<Provider>, replica_of: &str) -> RelayFaucet {
        let (tx, rx) = unbounded_channel();

        RelayProducer::new(replica.clone(), &self.network, replica_of, tx).run_until_panic();
        rx
    }

    pub(crate) fn relay_producers(&self) -> HashMap<&str, RelayFaucet> {
        self.replicas()
            .iter()
            .map(|(network, replica)| {
                let producer = self.relay_producer_for(replica, network);
                (network.as_str(), producer)
            })
            .collect()
    }

    pub(crate) fn process_producer_for(
        &self,
        replica: &Replica<Provider>,
        replica_of: &str,
    ) -> ProcessFaucet {
        let (tx, rx) = unbounded_channel();

        ProcessProducer::new(replica.clone(), &self.network, replica_of, tx).run_until_panic();
        rx
    }

    pub(crate) fn process_producers(&self) -> HashMap<&str, ProcessFaucet> {
        self.replicas()
            .iter()
            .map(|(replica_of, replica)| {
                let producer = self.process_producer_for(replica, replica_of);
                (replica_of.as_str(), producer)
            })
            .collect()
    }

    pub(crate) fn count_dispatches<'a>(
        &'a self,
        faucets: &mut Faucets<'a>,
        metrics: BetweenMetrics,
        event: impl AsRef<str>,
    ) {
        let network = self.network.clone();
        let emitter = self.home_address();

        tracing::debug!(
            network = network,
            home = emitter,
            event = event.as_ref(),
            "starting counter",
        );
        BetweenEvents::new(
            faucets.dispatch_pipe(self.name()),
            metrics,
            network,
            event,
            emitter,
        )
        .run_until_panic();
    }

    pub(crate) fn count_updates<'a>(
        &'a self,
        faucets: &mut Faucets<'a>,
        metrics: BetweenMetrics,
        event: impl AsRef<str>,
    ) {
        let network = self.network.clone();
        let emitter = self.home_address();

        let pipe = faucets.update_pipe(&self.network);
        tracing::debug!(
            network = network,
            home = emitter,
            event = event.as_ref(),
            "starting counter",
        );
        BetweenEvents::new(pipe, metrics, network, event, emitter).run_until_panic();
    }

    pub(crate) fn count_relays<'a>(&'a self, faucets: &mut Faucets<'a>, metrics: Arc<Metrics>) {
        self.replicas.iter().for_each(|(replica_of, replica)| {
            let emitter = format!("{:?}", replica.address());
            let network = self.name();
            let event = "relay";
            tracing::debug!(
                network = network,
                replica = emitter,
                event,
                "starting counter",
            );

            let pipe = faucets.relay_pipe(&self.network, replica_of);

            let metrics = metrics.between_metrics(network, event, &emitter, Some(replica_of));

            BetweenEvents::new(pipe, metrics, network, event, emitter).run_until_panic();
        });
    }

    pub(crate) fn count_processes<'a>(&'a self, faucets: &mut Faucets<'a>, metrics: Arc<Metrics>) {
        self.replicas.iter().for_each(|(replica_of, replica)| {
            let emitter = format!("{:?}", replica.address());
            let network = self.name();
            let event = "process";
            tracing::debug!(
                network = network,
                replica = emitter,
                event,
                "starting counter",
            );

            let pipe = faucets.process_pipe(&self.network, replica_of);

            let metrics = metrics.between_metrics(network, event, &emitter, Some(replica_of));

            BetweenEvents::new(pipe, metrics, network, event, emitter).run_until_panic();
        });
    }

    pub(crate) fn dispatch_to_update<'a>(
        &'a self,
        faucets: &mut Faucets<'a>,
        metrics: Arc<Metrics>,
    ) {
        let metrics = metrics.dispatch_wait_metrics(&self.network, &self.home_address());

        let dispatch_pipe = faucets.dispatch_pipe(self.name());
        let update_pipe = faucets.update_pipe(self.name());

        DispatchWait::new(
            dispatch_pipe,
            update_pipe,
            self.name(),
            self.home_address(),
            metrics,
        )
        .run_until_panic();
    }

    pub(crate) fn update_to_relay<'a>(&'a self, faucets: &mut Faucets<'a>, metrics: Arc<Metrics>) {
        let mut relay_faucets = HashMap::new();
        let mut relay_sinks = HashMap::new();

        // we want to go through each network that is NOT this network
        // get the relay sink FOR THIS NETWORK'S REPLICA
        let other_nets: Vec<&str> = faucets
            .relays
            .keys()
            // does not match this network
            .filter(|k| **k != self.network)
            .copied()
            .collect();

        faucets
            .relays
            .iter_mut()
            .filter(|(k, _)| other_nets.contains(k))
            .for_each(|(k, v)| {
                // create a new channel
                let (sink, faucet) = unbounded_channel();
                // insert this in the map we'll give to the metrics task
                relay_sinks.insert(k.to_string(), sink);

                // replace the faucet for the replica matching this network
                // in the global producers map
                let faucet = v
                    .insert(self.name(), faucet)
                    .expect("missing relay producer");
                // insert upstream faucet into the map we'll give to the
                // metrics task
                relay_faucets.insert(k.to_string(), faucet);
            });

        let update_pipe = faucets.update_pipe(self.name());

        let metrics = metrics.update_wait_metrics(self.name(), &other_nets, &self.home_address());

        UpdateWait::new(
            update_pipe,
            self.name(),
            metrics,
            relay_sinks,
            relay_faucets,
        )
        .run_until_panic();
    }

    pub(crate) fn relay_to_process<'a>(&'a self, faucets: &mut Faucets<'a>, metrics: Arc<Metrics>) {
        self.replicas.iter().for_each(|(replica_of, replica)| {
            let emitter = format!("{:?}", replica.address());

            let relay_pipe = faucets.relay_pipe(self.name(), replica_of);
            let process_pipe = faucets.process_pipe(self.name(), replica_of);

            let metrics = metrics.relay_wait_metrics(self.name(), replica_of, &emitter);

            RelayWait::new(
                relay_pipe,
                process_pipe,
                self.name().to_owned(),
                replica_of.to_owned(),
                emitter,
                metrics,
            )
            .run_until_panic();
        });
    }
}
