mod indexer;
mod loader;
mod runner;

pub use loader::Loader;
pub use runner::Runner;

use std::convert::TryFrom;
use std::mem;

use crate::types::{ContractAddress, EoaAddress};
use ckb_types::{bytes::Bytes, H160, H256};
use serde::{Deserialize, Serialize};

type BlockNumber = u64;

/// The indexer key type
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
#[repr(u8)]
pub enum KeyType {
    /// The key is just last
    ///   "last" => (BlockNumber, BlockHash)
    Last = 0x00,

    /// The key is a block number
    ///   BlockNumber => BlockHash
    BlockMap = 0x01,

    /// Contract state change
    ///   (ContractAddress, BlockNumber, TransactionIndex, OutputIndex)
    ///      => (TransactionHash, SenderAddress, NewStorageTree, Vec<(Topics, Data)>)
    ContractChange = 0x02,

    /// Contract logs
    ///   (ContractAddress, BlockNumber, TransactionIndex, OutputIndex)
    ///      => Vec<(Topics, Data)>
    ContractLogs = 0x03,

    /// Contract code
    ///   ContractAddress => (Code, OutPoint)
    ContractCode = 0x04,

    /// Changed contracts in the block (for rollback)
    ///   BlockNumber => Vec<ContractAddress>
    BlockContracts = 0xF0,
}

impl TryFrom<u8> for KeyType {
    type Error = String;
    fn try_from(value: u8) -> Result<KeyType, String> {
        match value {
            0x00 => Ok(KeyType::Last),
            0x01 => Ok(KeyType::BlockMap),
            0x02 => Ok(KeyType::ContractChange),
            0x03 => Ok(KeyType::ContractLogs),
            0x04 => Ok(KeyType::ContractCode),
            0xF0 => Ok(KeyType::BlockContracts),
            _ => Err(format!("Invalid KeyType {}", value)),
        }
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum Key {
    Last,
    BlockMap(BlockNumber),
    ContractChange {
        address: ContractAddress,
        number: Option<BlockNumber>,
        /// Transaction index in current block
        tx_index: Option<u32>,
        /// Output index in current transaction
        output_index: Option<u32>,
    },
    ContractLogs {
        address: ContractAddress,
        number: Option<BlockNumber>,
        /// Transaction index in current block
        tx_index: Option<u32>,
        /// Output index in current transaction
        output_index: Option<u32>,
    },
    ContractCode(ContractAddress),
    BlockContracts(BlockNumber),
}

impl From<&Key> for Bytes {
    fn from(key: &Key) -> Bytes {
        fn serialize_record_key(
            key_type: KeyType,
            address: &ContractAddress,
            number: &Option<u64>,
            tx_index: &Option<u32>,
            output_index: &Option<u32>,
        ) -> Bytes {
            let mut bytes = vec![key_type as u8];
            bytes.extend(address.0.as_bytes());
            if let Some(number) = number {
                bytes.extend(&number.to_be_bytes());
                if let Some(tx_index) = tx_index {
                    bytes.extend(&tx_index.to_be_bytes());
                    if let Some(output_index) = output_index {
                        bytes.extend(&output_index.to_be_bytes());
                    }
                }
            }
            bytes.into()
        }
        match key {
            Key::Last => vec![KeyType::Last as u8].into(),
            Key::BlockMap(number) => {
                let mut bytes = vec![KeyType::BlockMap as u8];
                bytes.extend(&number.to_be_bytes());
                bytes.into()
            }
            Key::ContractChange {
                address,
                number,
                tx_index,
                output_index,
            } => serialize_record_key(
                KeyType::ContractChange,
                address,
                number,
                tx_index,
                output_index,
            ),
            Key::ContractLogs {
                address,
                number,
                tx_index,
                output_index,
            } => serialize_record_key(
                KeyType::ContractLogs,
                address,
                number,
                tx_index,
                output_index,
            ),
            Key::ContractCode(address) => {
                let mut bytes = vec![KeyType::ContractCode as u8];
                bytes.extend(address.0.as_bytes());
                bytes.into()
            }
            Key::BlockContracts(number) => {
                let mut bytes = vec![KeyType::BlockContracts as u8];
                bytes.extend(&number.to_be_bytes());
                bytes.into()
            }
        }
    }
}

impl TryFrom<&[u8]> for Key {
    type Error = String;
    fn try_from(data: &[u8]) -> Result<Key, String> {
        fn ensure_content_len(name: &str, content: &[u8], expected: usize) -> Result<(), String> {
            if content.len() != expected {
                Err(format!(
                    "Invalid Key::{} content length: {}",
                    name,
                    content.len()
                ))
            } else {
                Ok(())
            }
        }
        fn deserialize_u64(content: &[u8]) -> u64 {
            let mut number_bytes = [0u8; 8];
            number_bytes.copy_from_slice(content);
            u64::from_be_bytes(number_bytes)
        }
        fn deserialize_u32(content: &[u8]) -> u32 {
            let mut number_bytes = [0u8; 4];
            number_bytes.copy_from_slice(content);
            u32::from_be_bytes(number_bytes)
        }
        fn deserialize_record_key(
            name: &str,
            content: &[u8],
        ) -> Result<(ContractAddress, u64, u32, u32), String> {
            const EXPECTED: usize = mem::size_of::<H160>()
                + mem::size_of::<BlockNumber>()
                + mem::size_of::<u32>()
                + mem::size_of::<u32>();
            assert_eq!(EXPECTED, 20 + 8 + 4 + 4);
            ensure_content_len("ContractChange", content, EXPECTED)?;

            let address = ContractAddress::from(
                H160::from_slice(&content[0..20]).expect("deserialize address"),
            );
            let number = deserialize_u64(&content[20..28]);
            let tx_index = deserialize_u32(&content[28..32]);
            let output_index = deserialize_u32(&content[32..36]);
            Ok((address, number, tx_index, output_index))
        }

        if data.is_empty() {
            return Err(String::from("Can't convert to Key from empty data"));
        }
        let key_type = KeyType::try_from(data[0])?;
        let content = &data[1..];
        match key_type {
            KeyType::Last => Ok(Key::Last),
            KeyType::BlockMap => {
                ensure_content_len("BlockMap", content, mem::size_of::<BlockNumber>())?;
                Ok(Key::Last)
            }
            KeyType::ContractChange => {
                let (address, number, tx_index, output_index) =
                    deserialize_record_key("ContractChange", content)?;
                Ok(Key::ContractChange {
                    address,
                    number: Some(number),
                    tx_index: Some(tx_index),
                    output_index: Some(output_index),
                })
            }
            KeyType::ContractLogs => {
                let (address, number, tx_index, output_index) =
                    deserialize_record_key("ContractLogs", content)?;
                Ok(Key::ContractLogs {
                    address,
                    number: Some(number),
                    tx_index: Some(tx_index),
                    output_index: Some(output_index),
                })
            }
            KeyType::ContractCode => {
                ensure_content_len("ContractCode", content, mem::size_of::<H160>())?;
                let address = ContractAddress::from(
                    H160::from_slice(&content[0..20]).expect("deserialize address"),
                );
                Ok(Key::ContractCode(address))
            }
            KeyType::BlockContracts => {
                ensure_content_len("BlockContracts", content, mem::size_of::<BlockNumber>())?;
                let number = deserialize_u64(&content[0..8]);
                Ok(Key::BlockContracts(number))
            }
        }
    }
}

/// Deserialize/Serialize use bincode
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Value {
    Last {
        number: BlockNumber,
        hash: H256,
    },
    BlockMap(H256),
    ContractChange {
        tx_hash: H256,
        sender: EoaAddress,
        new_storage: Vec<(H256, H256)>,
    },
    ContractCode {
        code: Bytes,
        /// The hash of the transaction where the contract created
        tx_hash: H256,
        /// The output index of the transaction where the contract created
        output_index: u32,
    },
    ContractLogs(Vec<(Vec<H256>, Bytes)>),
    BlockContracts(Vec<ContractAddress>),
}