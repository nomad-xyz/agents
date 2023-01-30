use crate::metrics::metrics::Metrics;
use crate::server::backoff::RestartBackoff;
use crate::server::errors::ServerRejection;
use crate::server::params::{Network, RestartableAgent};

use std::fmt::Debug;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};

use k8s_openapi::api::core::v1::Pod;
use kube::api::Api;
use kube::api::DeleteParams;
use kube::Client;
use serde::Serialize;

use tracing::{debug, error, info, instrument};

const ENVIRONMENT: &str = "dev";

/// Structure that represents a pod which a caller refers to to restart or get it's status.
#[derive(Debug)]
pub struct LifeguardPod {
    pub network: Network,
    pub agent: RestartableAgent,
}

impl LifeguardPod {
    pub fn new(network: Network, agent: RestartableAgent) -> Self {
        Self { network, agent }
    }
}

/// Format the `Lifeguard` into a Nomad's K8s pod name
impl ToString for LifeguardPod {
    fn to_string(&self) -> String {
        format!("{}-{}-{}-0", ENVIRONMENT, self.network, self.agent)
    }
}

/// Enum that represents one of the states of a pod:
///   * `Running` with a start date of the pod
///   * If the pod is in another phase than "Running", contains the phase as a String
#[derive(Serialize)]
pub enum PodStatus {
    Running(DateTime<Utc>),
    Phase(String),
}

/// Enug that represents several possible errors that could be raised in `K8S` structure
#[derive(Debug)]
pub enum K8sError {
    /// Pod reached a backoff limit
    TooEarly(DateTime<Utc>),
    /// Pod was not found in K8s
    NoPod,
    /// Pod has no status when status is requested
    NoStatus,
    /// Pod has no start time when status is requested
    NoStartTime,
    /// Some error was raised during a request to K8s
    Custom(Box<dyn std::error::Error>),
}

impl std::error::Error for K8sError {}

impl std::fmt::Display for K8sError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::TooEarly(t) => write!(f, "{}", t),
            Self::NoPod => write!(f, "NoPod"),
            Self::NoStatus => write!(f, "NoStatus"),
            Self::NoStartTime => write!(f, "NoStartTime"),
            Self::Custom(e) => write!(f, "K8s Error: {}", e),
        }
    }
}

impl From<K8sError> for ServerRejection {
    fn from(error: K8sError) -> Self {
        match error {
            K8sError::TooEarly(t) => ServerRejection::TooEarly(t),
            K8sError::NoPod => ServerRejection::InternalError(error.to_string()),
            K8sError::NoStatus => ServerRejection::InternalError(error.to_string()),
            K8sError::NoStartTime => ServerRejection::InternalError(error.to_string()),
            K8sError::Custom(e) => ServerRejection::InternalError(e.to_string()),
        }
    }
}

/// Main structure that is speaking to K8s.
pub struct K8S {
    client: Client,
    /// Main backoff mechanism
    backoff: RestartBackoff,
    /// Additional backoff limit, that could raise `K8sError::TooEarly`.
    /// Before the main `RestartBackoff` is checked, `K8S` checks the last restart of a pod.
    /// If the start time of the pod is higher than `now()` - `start_time_limit`, then backoff is fired.
    start_time_limit: Duration,
    metrics: Arc<Metrics>,
}

impl K8S {
    pub async fn new(metrics: Arc<Metrics>) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::try_default().await?;
        let backoff = RestartBackoff::new(
            5,
            Some(Duration::seconds(30)),
            Some(Duration::days(1)),
            metrics.clone(),
        );
        Ok(K8S {
            client,
            backoff,
            start_time_limit: Duration::minutes(1), // 1 min,
            metrics,
        })
    }

    /// Method that checks the backoff for the pod
    #[instrument]
    pub async fn check_backoff(&self, pod: &LifeguardPod) -> Result<(), K8sError> {
        debug!(pod = ?pod, "Checking backoff");
        if let Some(next_attempt_time) = self.backoff.inc(pod).await {
            return Err(K8sError::TooEarly(next_attempt_time));
        }
        Ok(())
    }

    /// Method that checks start time backoff limit.
    /// If the start time of the pod is higher than `now()` - `start_time_limit`, then backoff is fired.
    #[instrument]
    pub async fn check_start_time(&self, pod: &LifeguardPod) -> Result<(), K8sError> {
        debug!(pod = ?pod, "Checking start time");
        if let PodStatus::Running(start_time) = self.status(pod).await? {
            let next_attempt = start_time + self.start_time_limit;
            if next_attempt > Utc::now() {
                error!(pod = ?pod, start_time = ?start_time, next_attempt = ?next_attempt, "Too early for the pod");
                self.metrics.backoffs_inc(
                    "start_time",
                    &pod.network.to_string(),
                    &pod.agent.to_string(),
                );

                return Err(K8sError::TooEarly(next_attempt));
            }
        }

        Ok(())
    }

    /// Method that actually deletes the pod
    #[instrument]
    pub async fn delete_pod(&self, pod: &LifeguardPod) -> Result<(), K8sError> {
        debug!(pod = ?pod, "Started deleting pod");
        let pods: Api<Pod> = Api::default_namespaced(self.client.clone());
        let pod_name = pod.to_string();

        pods.delete(&pod_name, &DeleteParams::default())
            .await
            .map_err(|e| K8sError::Custom(Box::new(e)))?;
        info!(pod = ?pod, "Deleted pod");

        Ok(())
    }

    /// Method that deletes the pod, but before hand it checks that all backoff strategies are giving green light
    #[instrument]
    pub async fn try_delete_pod(&self, pod: &LifeguardPod) -> Result<(), K8sError> {
        debug!(pod = ?pod, "Starting full deleting pod procedure");

        // Should run in sequence
        self.check_start_time(pod).await?;
        self.check_backoff(pod).await?;

        self.delete_pod(pod).await?;
        debug!(pod = ?pod, "Finished full deleting pod procedure");
        Ok(())
    }

    /// Method that is used to get a pod status
    #[instrument]
    pub async fn status(&self, pod: &LifeguardPod) -> Result<PodStatus, K8sError> {
        let pods: Api<Pod> = Api::default_namespaced(self.client.clone());

        let name = pod.to_string();

        if let Some(found_pod) = pods
            .get_opt(&name)
            .await
            .map_err(|e| K8sError::Custom(Box::new(e)))?
        {
            debug!(pod = ?pod, "Found requested pod");
            if let Some(status) = found_pod.status {
                let start_time = status.start_time.ok_or(K8sError::NoStatus)?.0;
                let phase = status.phase.ok_or(K8sError::NoStartTime)?;

                info!(pod = ?pod, phase = phase, start_time = ?start_time, "Got pod's status");

                if phase == "Running" {
                    return Ok(PodStatus::Running(start_time));
                } else {
                    return Ok(PodStatus::Phase(phase));
                }
            }
        }
        return Err(K8sError::NoPod);
    }
}

impl Debug for K8S {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("K8S")
            .field("backoff", &self.backoff)
            .field("start_time_limit", &self.start_time_limit)
            .finish()
    }
}
