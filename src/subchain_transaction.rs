mod solana {
    pub use solana_sdk::instruction::Instruction;
    pub use solana_sdk::pubkey::Pubkey as Address;
}
mod evm {
    pub use primitive_types::H160 as Address;
}
use std::collections::{BTreeMap, BTreeSet};

use primitive_types::{H256, U256};
use serde::{Deserialize, Serialize};
use solana_sdk::instruction::AccountMeta;

pub type ChainID = u64;

const EVM_LOADER_ID: solana::Address =
    solana::Address::from_str_const("EVM1111111111111111111111111111111111111111");

pub const EVM_INSTRUCTION_BORSH_PREFIX: u8 = 255u8;

pub fn evm_state_subchain_account(chain_id: ChainID) -> solana::Address {
    const EVM_SUBCHAIN_SEED_PREFIX: &[u8] = b"evm_subchain";

    let (evm_subchain_state_pda, _bump_seed) = solana::Address::find_program_address(
        &[EVM_SUBCHAIN_SEED_PREFIX, &chain_id.to_be_bytes()],
        &EVM_LOADER_ID,
    );
    evm_subchain_state_pda
}

const EVM_SUBCHAIN_STORAGE_INDEX: usize = 3;

pub fn create_evm_subchain_account(
    owner: solana::Address,
    chain_id: ChainID,
    config: SubchainConfig,
    extended_config_storage: Option<solana::Address>,
) -> solana::Instruction {
    let evm_subchain_state_pda = evm_state_subchain_account(chain_id);
    let mut account_metas = vec![
        AccountMeta::new(EVM_LOADER_ID, false),
        AccountMeta::new(evm_subchain_state_pda, false),
        AccountMeta::new(owner, true),
        AccountMeta::new(solana_sdk::system_program::ID, false),
    ];
    if let Some(extended_config_storage) = extended_config_storage {
        account_metas.insert(
            EVM_SUBCHAIN_STORAGE_INDEX,
            AccountMeta::new(extended_config_storage, true),
        )
    }

    create_evm_instruction_with_borsh(
        EVM_LOADER_ID,
        &EvmInstruction::EvmSubchain(EvmSubChain::CreateAccount { chain_id, config }),
        account_metas,
    )
}

pub fn create_evm_instruction_with_borsh(
    program_id: solana::Address,
    instruction: &EvmInstruction,
    accounts: Vec<AccountMeta>,
) -> solana::Instruction {
    use borsh::BorshSerialize;

    let mut data = vec![EVM_INSTRUCTION_BORSH_PREFIX];
    instruction.serialize(&mut data).unwrap();
    solana::Instruction {
        accounts,
        program_id,
        data,
    }
}

use borsh::{BorshDeserialize, BorshSerialize};

#[allow(clippy::large_enum_variant)]
#[derive(
    BorshSerialize,
    BorshDeserialize,
    // TODO(L): add schema generation custom command
    // BorshSchema,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Ord,
    PartialOrd,
)]
pub enum EvmInstruction {
    SwapNativeToEther {},
    FreeOwnership {},
    EvmBigTransaction {},
    ExecuteTransaction {},

    /// account_structure [
    ///     account_key[0] - evm state
    ///     account_key[1] - custom evm state
    ///     account_key[2] - signer
    /// ]
    EvmSubchain(EvmSubChain),
}

#[derive(
    BorshSerialize,
    BorshDeserialize,
    // TODO(L): add schema generation custom command
    // BorshSchema,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Ord,
    PartialOrd,
)]

// NOTE: do not forget to update `solana_transaction_status::parse_evm` when changing instruction data
pub enum EvmSubChain {
    CreateAccount {
        chain_id: ChainID,
        config: SubchainConfig,
    },

    ExecuteTransaction {},
}

#[derive(
    BorshSerialize,
    BorshDeserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
)]
pub enum Hardfork {
    Istanbul,
}

#[derive(
    BorshSerialize,
    BorshDeserialize,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
)]
pub struct AllocAccount {
    pub code: Vec<u8>,
    pub storage: BTreeMap<H256, H256>,
    pub balance: U256,
    pub nonce: u64,
}

// Part of config that is stored in seperate account (storage).
// if Extended and regular config is provided - they will be merged by rewriting overlapping accounts.
#[derive(
    BorshSerialize,
    BorshDeserialize,
    // BorshSchema,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Ord,
    PartialOrd,
    Default,
)]
pub struct ExtendedConfig {
    pub alloc: BTreeMap<evm::Address, AllocAccount>,
}

#[derive(
    BorshSerialize,
    BorshDeserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
)]
pub struct SubchainConfig {
    pub alloc: BTreeMap<evm::Address, AllocAccount>,
    pub whitelisted: BTreeSet<solana::Address>,
    pub hardfork: Hardfork,
    pub network_name: String,
    pub token_name: String,
    pub min_gas_price: U256,
}

impl Default for SubchainConfig {
    fn default() -> Self {
        Self {
            alloc: BTreeMap::new(),
            whitelisted: BTreeSet::new(),
            hardfork: Hardfork::Istanbul,
            network_name: String::new(),
            token_name: String::new(),
            min_gas_price: U256::zero(),
        }
    }
}
