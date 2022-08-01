use color_eyre::{
    eyre::{ensure, WrapErr},
    Result,
};
use ethers::core::types::H256;
use nomad_core::db::{DbError, TypedDB, DB};
use nomad_core::{
    accumulator::NomadProof, utils, CommittedMessage, Decode, NomadMessage, RawCommittedMessage,
    SignedUpdate, SignedUpdateWithMeta, UpdateMeta,
};
use nomad_xyz_configuration::contracts::CoreContracts;
use nomad_xyz_configuration::NomadConfig;
use tokio::time::sleep;
use tracing::{debug, info};

use std::future::Future;
use std::time::Duration;

use nomad_core::db::iterator::PrefixIterator;

const LEAF_IDX: &str = "leaf_index_";
const LEAF: &str = "leaf_";
const PREV_ROOT: &str = "update_prev_root_";
const PROOF: &str = "proof_";
const MESSAGE: &str = "message_";
const UPDATE: &str = "update_";
const UPDATE_META: &str = "update_metadata_";
const LATEST_ROOT: &str = "update_latest_root_";
const LATEST_LEAF_INDEX: &str = "latest_known_leaf_index_";
const UPDATER_PRODUCED_UPDATE: &str = "updater_produced_update_";
const PROVER_LATEST_COMMITTED: &str = "prover_latest_committed_";
const PROCESSOR_ATTEMPTED: &str = "processor_attempted_";

const CORE_INTEGRITY: &str = "core_ingerity_";

/// DB handle for storing data tied to a specific home.
///
/// Key structure: ```<entity>_<additional_prefix(es)>_<key>```
#[derive(Debug, Clone)]
pub struct NomadDB(TypedDB);

impl From<TypedDB> for NomadDB {
    fn from(db: TypedDB) -> Self {
        NomadDB(db)
    }
}

impl std::ops::Deref for NomadDB {
    type Target = TypedDB;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<TypedDB> for NomadDB {
    fn as_ref(&self) -> &TypedDB {
        &self.0
    }
}

impl AsRef<DB> for NomadDB {
    fn as_ref(&self) -> &DB {
        self.0.as_ref()
    }
}

impl NomadDB {
    /// Instantiated new `NomadDB`
    pub fn new(entity: impl AsRef<str>, db: DB) -> Self {
        Self(TypedDB::new(entity.as_ref().to_owned(), db))
    }

    /// Check if db is empty
    pub fn is_empty(&self) -> Result<bool> {
        let no_updates = self.update_by_previous_root(H256::zero())?.is_none();
        let no_messages = self.leaf_by_leaf_index(0)?.is_none();
        Ok(no_updates && no_messages)
    }

    /// Store list of messages
    pub fn store_messages(&self, messages: &[RawCommittedMessage]) -> Result<()> {
        for message in messages {
            self.store_latest_message(message)?;

            let committed_message: CommittedMessage = message.clone().try_into()?;
            info!(
                leaf_index = &committed_message.leaf_index,
                origin = &committed_message.message.origin,
                destination = &committed_message.message.destination,
                nonce = &committed_message.message.nonce,
                "Stored new message in db.",
            );
        }

        Ok(())
    }

    /// Store a raw committed message
    ///
    /// Keys --> Values:
    /// - `destination_and_nonce` --> `leaf`
    /// - `leaf_index` --> `leaf`
    /// - `leaf` --> `message`
    pub fn store_raw_committed_message(&self, message: &RawCommittedMessage) -> Result<()> {
        let parsed = NomadMessage::read_from(&mut message.message.clone().as_slice())?;

        let destination_and_nonce = parsed.destination_and_nonce();

        let leaf = message.leaf();

        debug!(
            leaf = ?leaf,
            destination_and_nonce,
            destination = parsed.destination,
            nonce = parsed.nonce,
            leaf_index = message.leaf_index,
            "storing raw committed message in db"
        );
        self.store_leaf(message.leaf_index, destination_and_nonce, leaf)?;
        self.store_keyed_encodable(MESSAGE, &leaf, message)?;
        Ok(())
    }

    /// Store a raw committed message building off of the latest leaf index
    pub fn store_latest_message(&self, message: &RawCommittedMessage) -> Result<()> {
        // If there is no latest root, or if this update is on the latest root
        // update latest root
        match self.retrieve_latest_leaf_index()? {
            Some(idx) => {
                if idx == message.leaf_index - 1 {
                    self.update_latest_leaf_index(message.leaf_index)?;
                } else {
                    debug!(
                        "Attempted to store message not building off latest leaf index. Latest leaf index: {}. Attempted leaf index: {}.",
                        idx,
                        message.leaf_index,
                    )
                }
            }
            None => self.update_latest_leaf_index(message.leaf_index)?,
        }

        self.store_raw_committed_message(message)
    }

    /// Store the latest known leaf_index
    ///
    /// Key --> value: `LATEST_LEAF_INDEX` --> `leaf_index`
    pub fn update_latest_leaf_index(&self, leaf_index: u32) -> Result<(), DbError> {
        self.store_encodable("", LATEST_LEAF_INDEX, &leaf_index)
    }

    /// Retrieve the highest known leaf_index
    pub fn retrieve_latest_leaf_index(&self) -> Result<Option<u32>, DbError> {
        self.retrieve_decodable("", LATEST_LEAF_INDEX)
    }

    /// Store the leaf keyed by leaf_index
    fn store_leaf(
        &self,
        leaf_index: u32,
        destination_and_nonce: u64,
        leaf: H256,
    ) -> Result<(), DbError> {
        debug!(
            leaf_index,
            leaf = ?leaf,
            "storing leaf hash keyed by index and dest+nonce"
        );
        self.store_keyed_encodable(LEAF, &destination_and_nonce, &leaf)?;
        self.store_keyed_encodable(LEAF, &leaf_index, &leaf)
    }

    /// Retrieve a raw committed message by its leaf hash
    pub fn message_by_leaf(&self, leaf: H256) -> Result<Option<RawCommittedMessage>, DbError> {
        self.retrieve_keyed_decodable(MESSAGE, &leaf)
    }

    /// Retrieve the leaf hash keyed by leaf index
    pub fn leaf_by_leaf_index(&self, leaf_index: u32) -> Result<Option<H256>, DbError> {
        self.retrieve_keyed_decodable(LEAF, &leaf_index)
    }

    /// Retrieve the leaf hash keyed by destination and nonce
    pub fn leaf_by_nonce(&self, destination: u32, nonce: u32) -> Result<Option<H256>, DbError> {
        let dest_and_nonce = utils::destination_and_nonce(destination, nonce);
        self.retrieve_keyed_decodable(LEAF, &dest_and_nonce)
    }

    /// Retrieve a raw committed message by its leaf hash
    pub fn message_by_nonce(
        &self,
        destination: u32,
        nonce: u32,
    ) -> Result<Option<RawCommittedMessage>, DbError> {
        let leaf = self.leaf_by_nonce(destination, nonce)?;
        match leaf {
            None => Ok(None),
            Some(leaf) => self.message_by_leaf(leaf),
        }
    }

    /// Retrieve a raw committed message by its leaf index
    pub fn message_by_leaf_index(
        &self,
        index: u32,
    ) -> Result<Option<RawCommittedMessage>, DbError> {
        let leaf: Option<H256> = self.leaf_by_leaf_index(index)?;
        match leaf {
            None => Ok(None),
            Some(leaf) => self.message_by_leaf(leaf),
        }
    }

    /// Store the latest committed
    fn store_latest_root(&self, root: H256) -> Result<(), DbError> {
        debug!(root = ?root, "storing new latest root in DB");
        self.store_encodable("", LATEST_ROOT, &root)
    }

    /// Retrieve the latest committed
    pub fn retrieve_latest_root(&self) -> Result<Option<H256>, DbError> {
        self.retrieve_decodable("", LATEST_ROOT)
    }

    /// Store list of sorted updates and their metadata
    pub fn store_updates_and_meta(&self, updates: &[SignedUpdateWithMeta]) -> Result<()> {
        for update_with_meta in updates {
            self.store_latest_update(&update_with_meta.signed_update)?;
            self.store_update_metadata(update_with_meta)?;

            info!(
                block_number = update_with_meta.metadata.block_number,
                timestamp = ?update_with_meta.metadata.timestamp,
                previous_root = ?&update_with_meta.signed_update.update.previous_root,
                new_root = ?&update_with_meta.signed_update.update.new_root,
                "Stored new update in db.",
            );
        }

        Ok(())
    }

    /// Store update metadata (by update's new root)
    ///
    /// Keys --> Values:
    /// - `update_new_root` --> `update_metadata`
    pub fn store_update_metadata(
        &self,
        update_with_meta: &SignedUpdateWithMeta,
    ) -> Result<(), DbError> {
        let new_root = update_with_meta.signed_update.update.new_root;
        let metadata = update_with_meta.metadata;

        debug!(new_root = ?new_root, metadata = ?metadata, "storing update metadata in DB");

        self.store_keyed_encodable(UPDATE_META, &new_root, &metadata)
    }

    /// Retrieve update metadata (by update's new root)
    pub fn retrieve_update_metadata(&self, new_root: H256) -> Result<Option<UpdateMeta>, DbError> {
        self.retrieve_keyed_decodable(UPDATE_META, &new_root)
    }

    /// Store a signed update building off latest root
    ///
    /// Keys --> Values:
    /// - `LATEST_ROOT` --> `root`
    /// - `new_root` --> `prev_root`
    /// - `prev_root` --> `update`
    pub fn store_latest_update(&self, update: &SignedUpdate) -> Result<(), DbError> {
        debug!(
            previous_root = ?update.update.previous_root,
            new_root = ?update.update.new_root,
            "storing update in DB"
        );

        // If there is no latest root, or if this update is on the latest root
        // update latest root
        match self.retrieve_latest_root()? {
            Some(root) => {
                if root == update.update.previous_root {
                    self.store_latest_root(update.update.new_root)?;
                } else {
                    debug!(
                        "Attempted to store update not building off latest root: {:?}",
                        update
                    )
                }
            }
            None => self.store_latest_root(update.update.new_root)?,
        }

        self.store_update(update)
    }

    /// Store an update.
    ///
    /// Keys --> Values:
    /// - `new_root` --> `prev_root`
    /// - `prev_root` --> `update`
    pub fn store_update(&self, update: &SignedUpdate) -> Result<(), DbError> {
        self.store_keyed_encodable(UPDATE, &update.update.previous_root, update)?;
        self.store_keyed_encodable(
            PREV_ROOT,
            &update.update.new_root,
            &update.update.previous_root,
        )
    }

    /// Retrieve an update by its previous root
    pub fn update_by_previous_root(
        &self,
        previous_root: H256,
    ) -> Result<Option<SignedUpdate>, DbError> {
        self.retrieve_keyed_decodable(UPDATE, &previous_root)
    }

    /// Retrieve an update by its new root
    pub fn update_by_new_root(&self, new_root: H256) -> Result<Option<SignedUpdate>, DbError> {
        let prev_root: Option<H256> = self.retrieve_keyed_decodable(PREV_ROOT, &new_root)?;

        match prev_root {
            Some(prev_root) => self.update_by_previous_root(prev_root),
            None => Ok(None),
        }
    }

    /// Iterate over all leaves
    pub fn leaf_iterator(&self) -> PrefixIterator<H256> {
        PrefixIterator::new(self.0.as_ref().prefix_iterator(LEAF_IDX), LEAF_IDX.as_ref())
    }

    /// Store a proof by its leaf index
    ///
    /// Keys --> Values:
    /// - `leaf_index` --> `proof`
    pub fn store_proof(&self, leaf_index: u32, proof: &NomadProof) -> Result<(), DbError> {
        debug!(leaf_index, "storing proof in DB");
        self.store_keyed_encodable(PROOF, &leaf_index, proof)
    }

    /// Retrieve a proof by its leaf index
    pub fn proof_by_leaf_index(&self, leaf_index: u32) -> Result<Option<NomadProof>, DbError> {
        self.retrieve_keyed_decodable(PROOF, &leaf_index)
    }

    // TODO(james): this is a quick-fix for the prover_sync and I don't like it
    /// poll db ever 100 milliseconds waiting for a leaf.
    pub fn wait_for_leaf(&self, leaf_index: u32) -> impl Future<Output = Result<H256, DbError>> {
        let slf = self.clone();
        async move {
            loop {
                if let Some(leaf) = slf.leaf_by_leaf_index(leaf_index)? {
                    return Ok(leaf);
                }
                sleep(Duration::from_millis(100)).await
            }
        }
    }

    /// Store a pending update in the DB for potential submission.
    pub fn store_produced_update(
        &self,
        previous_root: H256,
        update: &SignedUpdate,
    ) -> Result<(), DbError> {
        self.store_keyed_encodable(UPDATER_PRODUCED_UPDATE, &previous_root, update)
    }

    /// Retrieve a pending update from the DB (if one exists).
    pub fn retrieve_produced_update(
        &self,
        previous_root: H256,
    ) -> Result<Option<SignedUpdate>, DbError> {
        self.retrieve_keyed_decodable(UPDATER_PRODUCED_UPDATE, &previous_root)
    }

    /// Store prover latest root for which db has all leaves/proofs under root
    pub fn store_prover_latest_committed(&self, root: H256) -> Result<(), DbError> {
        self.store_encodable("", PROVER_LATEST_COMMITTED, &root)
    }

    /// Retrieve prover latest root for which db has all leaves/proofs under
    /// root
    pub fn retrieve_prover_latest_committed(&self) -> Result<Option<H256>, DbError> {
        self.retrieve_decodable("", PROVER_LATEST_COMMITTED)
    }

    /// Set a DB entry stating that the processor has previously attempted to
    /// process a message
    pub fn set_previously_attempted(&self, message: &CommittedMessage) -> Result<(), DbError> {
        self.store_encodable(PROCESSOR_ATTEMPTED, message.to_leaf(), &true)
    }

    /// Returns `true` if the processor has previously attempted to process the
    /// mesage
    pub fn previously_attempted(&self, message: &CommittedMessage) -> Result<bool, DbError> {
        match self.retrieve_decodable(PROCESSOR_ATTEMPTED, message.to_leaf())? {
            Some(inner) => Ok(inner),
            None => Ok(false),
        }
    }

    /// Stores a core in the DB for later integrity checks
    pub fn store_core(&self, name: &str, core: &CoreContracts) -> Result<()> {
        let serialized = serde_json::to_string(core)?;
        Ok(self.store_keyed_encodable(CORE_INTEGRITY, &name.to_owned(), &serialized)?)
    }

    /// Retrieves a core from the DB
    pub fn retrieve_core(&self, name: &str) -> Result<Option<CoreContracts>> {
        if let Some(core_json) =
            self.retrieve_keyed_decodable::<_, _, String>(CORE_INTEGRITY, &name.to_owned())?
        {
            return Ok(serde_json::from_str(&core_json)?);
        }
        Ok(None)
    }

    /// Check a core's integrity against the DB. If there is no persisted
    /// object for that core, store it for later integrity checks
    pub fn check_core_integrity(&self, name: &str, core: &CoreContracts) -> Result<()> {
        if let Some(integrity) = self.retrieve_core(name)? {
            ensure!(integrity == *core, "integrity check failed");
        } else {
            self.store_core(name, core)?;
        }
        Ok(())
    }

    /// Checks the integrity of core contract addresses. Error if the DB
    /// contains differing addresses
    pub fn check_integrity(&self, config: &NomadConfig) -> Result<()> {
        for (name, core) in config.core().iter() {
            self.check_core_integrity(name, core)
                .wrap_err_with(|| format!("Error checking core for {}", name))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ethers::types::H256;
    use nomad_core::{accumulator::Proof, Encode, NomadMessage, RawCommittedMessage};
    use nomad_test::test_utils::run_test_db;

    #[tokio::test]
    async fn db_stores_and_retrieves_messages() {
        run_test_db(|db| async move {
            let home_name = "home_1".to_owned();
            let db = NomadDB::new(home_name, db);

            let m = NomadMessage {
                origin: 10,
                sender: H256::from_low_u64_be(4),
                nonce: 11,
                destination: 12,
                recipient: H256::from_low_u64_be(5),
                body: vec![1, 2, 3],
            };

            let message = RawCommittedMessage {
                leaf_index: 100,
                committed_root: H256::from_low_u64_be(3),
                message: m.to_vec(),
            };
            assert_eq!(m.to_leaf(), message.leaf());

            db.store_raw_committed_message(&message).unwrap();

            let by_nonce = db
                .message_by_nonce(m.destination, m.nonce)
                .unwrap()
                .unwrap();
            assert_eq!(by_nonce, message);

            let by_leaf = db.message_by_leaf(message.leaf()).unwrap().unwrap();
            assert_eq!(by_leaf, message);

            let by_index = db
                .message_by_leaf_index(message.leaf_index)
                .unwrap()
                .unwrap();
            assert_eq!(by_index, message);
        })
        .await;
    }

    #[tokio::test]
    async fn db_stores_and_retrieves_proofs() {
        run_test_db(|db| async move {
            let home_name = "home_1".to_owned();
            let db = NomadDB::new(home_name, db);

            let proof = Proof {
                leaf: H256::from_low_u64_be(15),
                index: 32,
                path: Default::default(),
            };
            db.store_proof(13, &proof).unwrap();

            let by_index = db.proof_by_leaf_index(13).unwrap().unwrap();
            assert_eq!(by_index, proof);
        })
        .await;
    }

    #[tokio::test]
    async fn db_integrity_check() {
        let core: CoreContracts = serde_json::from_str(
            r#"{
            "deployHeight": 12098988,
            "governanceRouter": {
              "beacon": "0x1631d12da55cbfb540d46e0dd9bbfb1d3f293dc8",
              "implementation": "0x2e588e0cff16cb8dd343551b435f5fee94f35230",
              "proxy": "0x6cc740e1e17b7b72e1d6c46afea4d44d86657102"
            },
            "home": {
              "beacon": "0x4b162c5c62a67e8a1772c0f04715ed2606b51421",
              "implementation": "0xe015da2b3cfdefb210ad5125744b552e80905468",
              "proxy": "0x884dad9316c61ed353b1d6931ba46663e1c3aacf"
            },
            "replicas": {
              "evmostestnet": {
                "beacon": "0x0c09e151720e0bcf4e2db42a3b5608b3de78e8d7",
                "implementation": "0x7c8cc92daa7d9172dfe5d8319cc74a6166d05c2c",
                "proxy": "0xb372d6b312f678494cf4e1bf5d149e733640e968"
              },
              "goerli": {
                "beacon": "0x0c09e151720e0bcf4e2db42a3b5608b3de78e8d7",
                "implementation": "0x7c8cc92daa7d9172dfe5d8319cc74a6166d05c2c",
                "proxy": "0x5f4d75de162b4c050f27ce2f2374d50e3d7fbbb6"
              },
              "neontestnet": {
                "beacon": "0x0c09e151720e0bcf4e2db42a3b5608b3de78e8d7",
                "implementation": "0x7c8cc92daa7d9172dfe5d8319cc74a6166d05c2c",
                "proxy": "0x495ef7cfee3850ba2afb5fea4c7c06ee0d1d0d6e"
              },
              "rinkeby": {
                "beacon": "0x0c09e151720e0bcf4e2db42a3b5608b3de78e8d7",
                "implementation": "0x7c8cc92daa7d9172dfe5d8319cc74a6166d05c2c",
                "proxy": "0x921dbedc12ba3299deaf8dd9fff0f435d8839edf"
              }
            },
            "updaterManager": "0x7f1b402a570f3221e03e41ef2408b5a215bb0448",
            "upgradeBeaconController": "0x87c44484add9020e7d6c98132311e1cd118ac236",
            "xAppConnectionManager": "0x42e8c0f7981add4c8081be20c813d49571f446f4"
          }"#,
        )
        .unwrap();

        let wrong: CoreContracts = serde_json::from_str(
            r#"{
            "deployHeight": 12098988,
            "governanceRouter": {
              "beacon": "0x0000000000000000000000000000000000000000",
              "implementation": "0x0000000000000000000000000000000000000000",
              "proxy": "0x0000000000000000000000000000000000000000"
            },
            "home": {
              "beacon": "0x0000000000000000000000000000000000000000",
              "implementation": "0x0000000000000000000000000000000000000000",
              "proxy": "0x0000000000000000000000000000000000000000"
            },
            "replicas": {
              "evmostestnet": {
                "beacon": "0x0000000000000000000000000000000000000000",
                "implementation": "0x0000000000000000000000000000000000000000",
                "proxy": "0x0000000000000000000000000000000000000000"
              },
              "goerli": {
                "beacon": "0x0000000000000000000000000000000000000000",
                "implementation": "0x0000000000000000000000000000000000000000",
                "proxy": "0x0000000000000000000000000000000000000000"
              },
              "neontestnet": {
                "beacon": "0x0000000000000000000000000000000000000000",
                "implementation": "0x0000000000000000000000000000000000000000",
                "proxy": "0x0000000000000000000000000000000000000000"
              },
              "rinkeby": {
                "beacon": "0x0000000000000000000000000000000000000000",
                "implementation": "0x0000000000000000000000000000000000000000",
                "proxy": "0x0000000000000000000000000000000000000000"
              }
            },
            "updaterManager": "0x0000000000000000000000000000000000000000",
            "upgradeBeaconController": "0x0000000000000000000000000000000000000000",
            "xAppConnectionManager": "0x0000000000000000000000000000000000000000"
          }"#,
        )
        .unwrap();

        run_test_db(|db| async move {
            let db = NomadDB::new("bootup integrity test", db);
            db.check_core_integrity("toast", &core).unwrap();
            assert!(
                db.check_core_integrity("toast", &wrong).is_err(),
                "should have caught changed addrs"
            );
            db.check_core_integrity("toast", &core).unwrap();
        })
        .await;
    }
}
