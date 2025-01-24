# Changelog

### Unreleased

- refactor: change `ChainConf::from_env` to avoid expensive json, and more
  verbosely report issues

### v2.0.1

- Add Accountant deploy info to bridge configuration

### v2.0.0

- feature (minor): add accountant to bridge contracts configuration
- Add `availDemo.json` config and test to ensure it builds into `NomadConfig`
- fix: SignerConf should not Display or Debug secrets
- add tests for SignerConf to ensure it does not Display or Debug secrets

### v1.0.0-rc.1

- refactor: remove `configs/testMultiVm.json` (and corresponding tests) and unify changes by putting all into `configs/test.json`
- feature: add additional config called `configs/testMultiVm.json` which has outbound connections to other chains but no inbound connections (no substrate replica support)
- refactor: change contract info (previously CoreContracts and BridgeContracts) to be CoreDeploymentInfo and BridgeDeploymentInfo, with support for substrate variant (not backwards compatible given core and bridge deploy/contract info is now a union not a struct)
- feature: update TS definitions for wasm_bindgen with Substrate-related changes
- feature: add more explicit Processor Config TS declaration
- refactor: make Processor config keys optional, and prevent trivial ser.
- fix: update TS AgentConfig to match rust
- refactor: remove protocol and bridge deploy-time gas value configuration
- feature: pull config files from github pages instead of local versions
- add environment variable overrides for agent configuration
- add tests for agent environment variable overrides
- remove `enabled` flag from agents project-wide

### v0.1.0-rc.25

- adds transaction submitters type to replace transaction signers
- adds gelato config struct
- uses correct staging s3 bucket in staging environment
- remove gnosis chain from list of bridgeGUI networks

### v0.1.0-rc.24

- remove `FromEnv` trait and make env loading conf-struct-specific
- add handling for default keys `TRANSACTIONSIGNERS_DEFAULT_{KEY,ID,REGION}`
  and `RPCS_DEFAULT_RPCSTYLE`
- add tests for new default config keys
- make aws key region non-functional (region should be read from env)
- set goerli as the new hub for development / staging in the gui

### v0.1.0-rc.23

- fix typo in staging goerli rpc url

### v0.1.0-rc.22

- fix broken output tests
- Add Neon testnet to dev and staging configs

### v0.1.0-rc.21

- validation of configured secrets takes in list of remote networks given that
  we can now specify remote networks of interest
- adds 3rd network to test.json to test partial specification of remotes
- add evmostestnet staging deploys

### v0.1.0-rc.20

- add evmos and xdai gas configs (evmDefault)
- add `evmDefault` option to gas configs
- Fix xDai display name

### v0.1.0-rc.19

- duplicate s3 bucket info at top level
  [#151](https://github.com/nomad-xyz/rust/pull/151)
- Simplify JSON representation of signers
- Simplify JSON representation of connection URLs
- Add avalanche

## v0.1.0-rc.18

- enable connext for evmos in production

## v0.1.0-rc.17

- add gas configs feature

### v0.1.0-rc.16

- add Evmos mainnet to production config

### v0.1.0-rc.15

- change connext enabled to false for evmos testnet
- add missing evmos testnet connection to rinkeby

### v0.1.0-rc.14

- remove xDai from production bridge gui

### v0.1.0-rc.13

- add evmostestnet to dev config
- remove connections to self for GUI configs

### v0.1.0-rc.12

- add redeployed dev/staging with rinkeby, goerli, kovan (no xDai)

### v0.1.0-rc.11

- add new optional fields to bridge gui specific configs, update types

### v0.1.0-rc.10

- Update production config to include xDai enrollment

### v0.1.0-rc.9

- Redeploy dev after improper update during devnet transition

### v0.1.0-rc.8

- Add bridge gui AppConfigs to JSON config files

### v0.1.0-rc.7

- adds processor S3 configs to dev/staging/prod configs
- change log level to info default
- change db path to /usr/share/nomad so persistent volumes saved
- add_domain no longer performs config validation
- add_domain returns Result<Option> to match wasm bindings

### v0.1.0-rc.6

- refactor: move common types (e.g. NomadIdentifier) into rust/nomad-types
- refactor: replace BaseAgentConfig with agent-specific public config blocks
  instantiated with interval and enabled there by default
- fix: uint deser_nomad_number expanded beyond just u64

### v0.1.0-rc.5

- fix: to allow TS deploys, bindings no longer perform config validation
  on deserialization

### v0.1.0-rc.4

- fix: expose config version number in TS type
- fix: to allow TS deploys, bindings no longer perform config validation
  on all operations

### v0.1.0-rc.3

- `customs` in `BridgeConfiguration` now properly optional in TS
- Optional properties now skip serialization if they are none
- add `governance_router` to Rust `EvmCoreContracts` struct
- fix test.json replica info to match production.json

### v0.1.0-rc.2

- add `mintGas` to `BridgeConfiguration`
- add `deployGas` to `BridgeConfiguration`
- `customs` in `BridgeConfiguration` is now optional

### v0.1.0-rc.1

- add config for `development`
- add config for `staging`
- add config for `production`
- refactor builtins for better amortization
- move indexing `from` option to contract block `deployHeight`
- move indexing `chunk` option to network block `indexPageSize`
- rename testnet `milkomedatestnet` -> `milkomedaC1testnet`

### v0.1.0-beta.14

- fix: correct import in wasm bindings

### v0.1.0-beta.13

- feature: add `blockExplorer` to network specs
- feature: add `bridgeGui` to top-level config

### v0.1.0-beta.12

- feature: add `confirmations` to network specs

### v0.1.0-beta.11

- fix: add missing chainId in NetworkSpecification ts type

### v0.1.0-beta.10

- fix: add bridgeConfiration in Domain ts type

### v0.1.0-beta.9

- add deploy-time custom token inputs/outputs

### v0.1.0-beta.8

- fix incorrect TS definition

### v0.1.0-beta.7

- add chain id to protocol specification block
- single updater. not multiple

### v0.1.0-beta.6

- correct typo in Domain

### v0.1.0-beta.5

- breakup network specs and contract configuration

### v0.1.0-beta.3

- rename `timelag` to `finalizationBlocks` and move to protocol section
- add version number to top-level config
- add yaml string output
- add a changelog
