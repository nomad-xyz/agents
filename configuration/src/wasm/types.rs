//! THIS IS AUTOGENERATED CODE, DO NOT EDIT
//! Please edit `data/definitions.ts` and `data/types.rs`
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const _: &'static str = r#"
export type NomadIdentifier = string;
export type NameOrDomain = number | string;

export interface AppConfig {
  displayName: string;
  nativeTokenSymbol: string;
  connections?: string[];
  manualProcessing?: boolean;
  connextEnabled?: boolean;
}

export interface NomadLocator {
  domain: number;
  id: NomadIdentifier;
}

export interface LogConfig {
  fmt: string;
  level: string;
}

export interface BaseAgentConfig {
  interval: number | string;
}

export type ProcessorConfig = BaseAgentConfig & {
  allowed?: string[];
  denied?: string[];
  subsidizedRemotes?: string[];
  s3?: S3Config;
};

export interface AgentConfig {
  rpcStyle: string;
  db: string;
  metrics: number;
  logging: LogConfig;
  updater: BaseAgentConfig;
  relayer: BaseAgentConfig;
  processor: ProcessorConfig;
  watcher: BaseAgentConfig;
  kathy: BaseAgentConfig;
}

export interface Proxy {
  implementation: NomadIdentifier;
  proxy: NomadIdentifier;
  beacon: NomadIdentifier;
}

export interface EvmCoreContracts {
  deployHeight: number;
  upgradeBeaconController: NomadIdentifier;
  xAppConnectionManager: NomadIdentifier;
  updaterManager: NomadIdentifier;
  governanceRouter: Proxy;
  home: Proxy;
  replicas: Record<string, Proxy>;
}

export type DeploymentInfo = EvmCoreContracts;

export interface DeployedCustomToken {
  token: NomadLocator;
  name: string;
  symbol: string;
  decimals: number;
  controller: NomadIdentifier;
  addresses: Proxy;
}

export interface EvmBridgeContracts {
  deployHeight: number;
  bridgeRouter: Proxy;
  tokenRegistry: Proxy;
  bridgeToken: Proxy;
  ethHelper?: NomadIdentifier;
  customs?: Array<DeployedCustomToken>;
}

export type BridgeContracts = EvmBridgeContracts;

export interface Governance {
  governor?: NomadLocator;
  recoveryManager: NomadIdentifier;
  recoveryTimelock: number | string;
}

export interface ContractConfig {
  optimisticSeconds: number | string;
  governance: Governance;
  updater: NomadIdentifier;
  watchers: Array<NomadIdentifier>;
}

export interface NetworkSpecs {
  chainId: number;
  finalizationBlocks: number | string;
  blockTime: number | string;
  supports1559: boolean;
  confirmations: number | string;
  blockExplorer: string;
  indexPageSize: number;
}

export interface CustomTokenSpecifier {
  token: NomadLocator;
  name: string;
  symbol: string;
  decimals: number;
}

export interface BridgeConfiguration {
  weth?: NomadIdentifier;
  customs?: Array<CustomTokenSpecifier>;
}

export interface Domain {
  name: string;
  domain: number;
  connections: Array<string>;
  configuration: ContractConfig;
  specs: NetworkSpecs;
  bridgeConfiguration: BridgeConfiguration;
}

export interface NetworkInfo {
  governor: NomadLocator;
  networks: Record<string, Domain>;
}

export interface HomeUpdateGasLimit {
  perMessage: number;
  base: number;
}

export interface HomeGasLimits {
  update: HomeUpdateGasLimit;
  improperUpdate: HomeUpdateGasLimit;
  doubleUpdate: number;
}

export interface ReplicaGasLimits {
  update: number;
  prove: number;
  process: number;
  proveAndProcess: number;
  doubleUpdate: number;
}

export interface ConnectionManagerGasLimits {
  ownerUnenrollReplica: number;
  unenrollReplica: number;
}

export interface CoreGasConfig {
  home: HomeGasLimits;
  replica: ReplicaGasLimits;
  connectionManager: ConnectionManagerGasLimits;
}

export interface BridgeRouterGasLimits {
  send: number;
}

export interface EthHelperGasLimits {
  send: number;
  sendToEvmLike: number;
}

export interface BridgeGasConfig {
  bridgeRouter: BridgeRouterGasLimits;
  ethHelper: EthHelperGasLimits;
}

export interface NomadGasConfig {
  core: CoreGasConfig;
  bridge: BridgeGasConfig;
}

export interface S3Config {
  bucket: string;
  region: string;
}

export interface NomadConfig {
  version: number;
  environment: string;
  networks: Array<string>;
  rpcs: Record<string, Array<string>>;
  protocol: NetworkInfo;
  core: Record<string, DeploymentInfo>;
  bridge: Record<string, BridgeContracts>;
  agent: Record<string, AgentConfig>;
  gas: Record<string, NomadGasConfig>;
  bridgeGui: Record<string, AppConfig>;
  s3?: S3Config;
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "AppConfig")]
    pub type AppConfig;

    #[wasm_bindgen(typescript_type = "NomadLocator")]
    pub type NomadLocator;

    #[wasm_bindgen(typescript_type = "LogConfig")]
    pub type LogConfig;

    #[wasm_bindgen(typescript_type = "IndexConfig")]
    pub type IndexConfig;

    #[wasm_bindgen(typescript_type = "BaseAgentConfig")]
    pub type BaseAgentConfig;

    #[wasm_bindgen(typescript_type = "AgentConfig")]
    pub type AgentConfig;

    #[wasm_bindgen(typescript_type = "Proxy")]
    pub type Proxy;

    #[wasm_bindgen(typescript_type = "EthereumCoreDeploymentInfo")]
    pub type EthereumCoreDeploymentInfo;

    #[wasm_bindgen(typescript_type = "CoreDeploymentInfo")]
    pub type CoreDeploymentInfo;

    #[wasm_bindgen(typescript_type = "DeployedCustomToken")]
    pub type DeployedCustomToken;

    #[wasm_bindgen(typescript_type = "EthereumBridgeDeploymentInfo")]
    pub type EthereumBridgeDeploymentInfo;

    #[wasm_bindgen(typescript_type = "BridgeDeploymentInfo")]
    pub type BridgeDeploymentInfo;

    #[wasm_bindgen(typescript_type = "Governance")]
    pub type Governance;

    #[wasm_bindgen(typescript_type = "ContractConfig")]
    pub type ContractConfig;

    #[wasm_bindgen(typescript_type = "NetworkSpecs")]
    pub type NetworkSpecs;

    #[wasm_bindgen(typescript_type = "CustomTokenSpecifier")]
    pub type CustomTokenSpecifier;

    #[wasm_bindgen(typescript_type = "BridgeConfiguration")]
    pub type BridgeConfiguration;

    #[wasm_bindgen(typescript_type = "Domain")]
    pub type Domain;

    #[wasm_bindgen(typescript_type = "NetworkInfo")]
    pub type NetworkInfo;

    #[wasm_bindgen(typescript_type = "HomeUpdateGasLimit")]
    pub type HomeUpdateGasLimit;

    #[wasm_bindgen(typescript_type = "HomeGasLimits")]
    pub type HomeGasLimits;

    #[wasm_bindgen(typescript_type = "ReplicaGasLimits")]
    pub type ReplicaGasLimits;

    #[wasm_bindgen(typescript_type = "ConnectionManagerGasLimits")]
    pub type ConnectionManagerGasLimits;

    #[wasm_bindgen(typescript_type = "CoreGasConfig")]
    pub type CoreGasConfig;

    #[wasm_bindgen(typescript_type = "BridgeRouterGasLimits")]
    pub type BridgeRouterGasLimits;

    #[wasm_bindgen(typescript_type = "EthHelperGasLimits")]
    pub type EthHelperGasLimits;

    #[wasm_bindgen(typescript_type = "BridgeGasConfig")]
    pub type BridgeGasConfig;

    #[wasm_bindgen(typescript_type = "NomadGasConfig")]
    pub type NomadGasConfig;

    #[wasm_bindgen(typescript_type = "S3Config")]
    pub type S3Config;

    #[wasm_bindgen(typescript_type = "NomadConfig")]
    pub type NomadConfig;
}
