use ethers::prelude::U64;
use prometheus::{Histogram, HistogramTimer};
use tokio::select;
use tracing::{info_span, Instrument};

use crate::{
    bail_task_if, DispatchFaucet, DispatchSink, ProcessStep, Restartable, StepHandle, UpdateFaucet,
    UpdateSink,
};

#[derive(Debug)]
pub(crate) struct DispatchWaitMetrics {
    pub(crate) timer: Histogram,
    pub(crate) blocks: Histogram,
}

#[derive(Debug)]
pub(crate) struct DispatchWait {
    incoming_dispatch: DispatchFaucet,
    incoming_update: UpdateFaucet,

    network: String,
    emitter: String,

    metrics: DispatchWaitMetrics,

    timers: Vec<HistogramTimer>,
    blocks: Vec<U64>,

    outgoing_update: UpdateSink,
    outgoing_dispatch: DispatchSink,
}

impl std::fmt::Display for DispatchWait {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DispatchToUpdate latency for {}'s home @ {}",
            self.network, self.emitter,
        )
    }
}

impl DispatchWait {
    pub(crate) fn new(
        incoming_dispatch: DispatchFaucet,
        incoming_update: UpdateFaucet,
        network: String,
        emitter: String,
        metrics: DispatchWaitMetrics,
        outgoing_update: UpdateSink,
        outgoing_dispatch: DispatchSink,
    ) -> Self {
        Self {
            incoming_dispatch,
            incoming_update,
            network,
            emitter,
            metrics,
            timers: vec![],
            blocks: vec![],
            outgoing_update,
            outgoing_dispatch,
        }
    }

    fn handle_dispatch(&mut self, block_number: U64) {
        self.timers.push(self.metrics.timer.start_timer());
        self.blocks.push(block_number);
    }

    fn handle_update(&mut self, block_number: U64) {
        // drain the entire vec
        self.timers
            .drain(0..)
            .for_each(|timer| timer.observe_duration());
        self.blocks.drain(0..).for_each(|dispatch_height| {
            let diff = block_number.saturating_sub(dispatch_height);
            self.metrics.blocks.observe(diff.as_u64() as f64);
        });
    }
}

pub(crate) type DispatchWaitTask = Restartable<DispatchWait>;
pub(crate) type DispatchWaitHandle = StepHandle<DispatchWait>;

#[derive(Debug)]
pub struct DispatchWaitOutput {
    pub(crate) dispatches: DispatchFaucet,
    pub(crate) updates: UpdateFaucet,
}

impl ProcessStep for DispatchWait {
    type Output = DispatchWaitOutput;

    fn spawn(mut self) -> DispatchWaitTask
    where
        Self: 'static + Send + Sync + Sized,
    {
        let span = info_span!(
            "DispatchWait",
            network = self.network.as_str(),
            emitter = self.emitter.as_str(),
        );

        tokio::spawn(
            async move {
                loop {
                    // how this works:
                    // For each dispatch, we mark its block height and start a
                    // timer.
                    // Every time an update comes in, we observe all timers, and
                    // then observe all the interblock periods.
                    //
                    // We always send the event onwards before making local
                    // observations, to ensure that the next step gets it
                    // reasonably early

                    select! {
                        // cause the select block to always poll dispatches
                        // first. i.e. ready dispatches will arrive first
                        biased;

                        dispatch_next = self.incoming_dispatch.recv() => {
                            bail_task_if!(
                                dispatch_next.is_none(),
                                self,
                                "inbound dispatch broke"
                            );
                            let dispatch = dispatch_next.expect("checked in block");
                            let block_number = dispatch.meta.block_number;
                            bail_task_if!(
                                self.outgoing_dispatch.send(dispatch).is_err(),
                                self,
                                "outbound dispatch broke"
                            );
                            self.handle_dispatch(block_number);
                        }
                        update_opt = self.incoming_update.recv() => {
                            bail_task_if!(
                                update_opt.is_none(),
                                self,
                                "inbound update broke"
                            );
                            let update = update_opt.expect("checked in block");
                            let block_number = update.meta.block_number;

                            bail_task_if!(
                                self.outgoing_update.send(update).is_err(),
                                self,
                                "outbound update broke"
                            );
                            self.handle_update(block_number);
                        }
                    }
                }
            }
            .instrument(span),
        )
    }
}
