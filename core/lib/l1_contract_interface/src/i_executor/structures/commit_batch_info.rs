use std::sync::Arc;

use zkevm_test_harness_1_4_2::kzg::KzgSettings;
use zksync_types::{
    commitment::{pre_boojum_serialize_commitments, serialize_commitments, L1BatchWithMetadata},
    ethabi::Token,
    pubdata_da::PubdataDA,
    web3::{contract::Error as Web3ContractError, error::Error as Web3ApiError},
    ProtocolVersionId, U256,
};

use crate::{
    i_executor::commit::kzg::{KzgInfo, ZK_SYNC_BYTES_PER_BLOB},
    Tokenizable,
};

/// These are used by the L1 Contracts to indicate what DA layer is used for pubdata
const PUBDATA_SOURCE_CALLDATA: u8 = 0;
const PUBDATA_SOURCE_BLOBS: u8 = 1;

/// Encoding for `CommitBatchInfo` from `IExecutor.sol`
#[derive(Debug)]
pub struct CommitBatchInfo<'a> {
    pub l1_batch_with_metadata: &'a L1BatchWithMetadata,
    pub pubdata_da: PubdataDA,
    pub kzg_settings: Option<Arc<KzgSettings>>,
}

impl<'a> CommitBatchInfo<'a> {
    pub fn new(
        l1_batch_with_metadata: &'a L1BatchWithMetadata,
        pubdata_da: PubdataDA,
        kzg_settings: Option<Arc<KzgSettings>>,
    ) -> Self {
        Self {
            l1_batch_with_metadata,
            pubdata_da,
            kzg_settings,
        }
    }

    fn base_tokens(&self) -> Vec<Token> {
        if self
            .l1_batch_with_metadata
            .header
            .protocol_version
            .unwrap_or_else(ProtocolVersionId::last_potentially_undefined)
            .is_pre_boojum()
        {
            vec![
                Token::Uint(U256::from(self.l1_batch_with_metadata.header.number.0)),
                Token::Uint(U256::from(self.l1_batch_with_metadata.header.timestamp)),
                Token::Uint(U256::from(
                    self.l1_batch_with_metadata.metadata.rollup_last_leaf_index,
                )),
                Token::FixedBytes(
                    self.l1_batch_with_metadata
                        .metadata
                        .merkle_root_hash
                        .as_bytes()
                        .to_vec(),
                ),
                Token::Uint(U256::from(self.l1_batch_with_metadata.header.l1_tx_count)),
                Token::FixedBytes(
                    self.l1_batch_with_metadata
                        .metadata
                        .l2_l1_merkle_root
                        .as_bytes()
                        .to_vec(),
                ),
                Token::FixedBytes(
                    self.l1_batch_with_metadata
                        .header
                        .priority_ops_onchain_data_hash()
                        .as_bytes()
                        .to_vec(),
                ),
                Token::Bytes(
                    self.l1_batch_with_metadata
                        .metadata
                        .initial_writes_compressed
                        .clone()
                        .unwrap(),
                ),
                Token::Bytes(
                    self.l1_batch_with_metadata
                        .metadata
                        .repeated_writes_compressed
                        .clone()
                        .unwrap(),
                ),
                Token::Bytes(pre_boojum_serialize_commitments(
                    &self.l1_batch_with_metadata.header.l2_to_l1_logs,
                )),
                Token::Array(
                    self.l1_batch_with_metadata
                        .header
                        .l2_to_l1_messages
                        .iter()
                        .map(|message| Token::Bytes(message.to_vec()))
                        .collect(),
                ),
                Token::Array(
                    self.l1_batch_with_metadata
                        .raw_published_factory_deps
                        .iter()
                        .map(|bytecode| Token::Bytes(bytecode.to_vec()))
                        .collect(),
                ),
            ]
        } else {
            vec![
                // `batchNumber`
                Token::Uint(U256::from(self.l1_batch_with_metadata.header.number.0)),
                // `timestamp`
                Token::Uint(U256::from(self.l1_batch_with_metadata.header.timestamp)),
                // `indexRepeatedStorageChanges`
                Token::Uint(U256::from(
                    self.l1_batch_with_metadata.metadata.rollup_last_leaf_index,
                )),
                // `newStateRoot`
                Token::FixedBytes(
                    self.l1_batch_with_metadata
                        .metadata
                        .merkle_root_hash
                        .as_bytes()
                        .to_vec(),
                ),
                // `numberOfLayer1Txs`
                Token::Uint(U256::from(self.l1_batch_with_metadata.header.l1_tx_count)),
                // `priorityOperationsHash`
                Token::FixedBytes(
                    self.l1_batch_with_metadata
                        .header
                        .priority_ops_onchain_data_hash()
                        .as_bytes()
                        .to_vec(),
                ),
                // `bootloaderHeapInitialContentsHash`
                Token::FixedBytes(
                    self.l1_batch_with_metadata
                        .metadata
                        .bootloader_initial_content_commitment
                        .unwrap()
                        .as_bytes()
                        .to_vec(),
                ),
                // `eventsQueueStateHash`
                Token::FixedBytes(
                    self.l1_batch_with_metadata
                        .metadata
                        .events_queue_commitment
                        .unwrap()
                        .as_bytes()
                        .to_vec(),
                ),
                // `systemLogs`
                Token::Bytes(serialize_commitments(
                    &self.l1_batch_with_metadata.header.system_logs,
                )),
            ]
        }
    }
}

impl<'a> Tokenizable for CommitBatchInfo<'a> {
    fn from_token(_token: Token) -> Result<Self, Web3ContractError>
    where
        Self: Sized,
    {
        // Currently there is no need to decode this struct.
        // We still want to implement `Tokenizable` trait for it, so that *once* it's needed
        // the implementation is provided here and not in some other inconsistent way.
        Err(Web3ContractError::Api(Web3ApiError::Decoder(
            "Not implemented".to_string(),
        )))
    }

    fn into_token(self) -> Token {
        let mut tokens = self.base_tokens();

        let protocol_version = self
            .l1_batch_with_metadata
            .header
            .protocol_version
            .unwrap_or_else(ProtocolVersionId::last_potentially_undefined);

        if protocol_version.is_pre_boojum() {
            return Token::Tuple(tokens);
        }

        if protocol_version.is_pre_1_4_2() {
            tokens.push(
                // `totalL2ToL1Pubdata` without pubdata source byte
                Token::Bytes(
                    self.l1_batch_with_metadata
                        .header
                        .pubdata_input
                        .clone()
                        .unwrap_or(self.l1_batch_with_metadata.construct_pubdata()),
                ),
            );
        } else {
            let pubdata = self
                .l1_batch_with_metadata
                .header
                .pubdata_input
                .clone()
                .unwrap_or(self.l1_batch_with_metadata.construct_pubdata());
            match self.pubdata_da {
                PubdataDA::Calldata => {
                    // We compute and add the blob commitment to the pubdata payload so that we can verify the proof
                    // even if we are not using blobs.
                    let blob_commitment =
                        KzgInfo::new(self.kzg_settings.as_ref().unwrap(), &pubdata)
                            .to_blob_commitment();

                    let result = std::iter::once(PUBDATA_SOURCE_CALLDATA)
                        .chain(pubdata)
                        .chain(blob_commitment)
                        .collect();

                    tokens.push(Token::Bytes(result));
                }
                PubdataDA::Blobs => {
                    let pubdata_commitments = pubdata
                        .chunks(ZK_SYNC_BYTES_PER_BLOB)
                        .flat_map(|blob| {
                            let kzg_info = KzgInfo::new(self.kzg_settings.as_ref().unwrap(), blob);
                            kzg_info.to_pubdata_commitment().to_vec()
                        })
                        .collect::<Vec<u8>>();

                    let result = std::iter::once(PUBDATA_SOURCE_BLOBS)
                        .chain(pubdata_commitments)
                        .collect();

                    tokens.push(Token::Bytes(result));
                }
            }
        }

        Token::Tuple(tokens)
    }
}
