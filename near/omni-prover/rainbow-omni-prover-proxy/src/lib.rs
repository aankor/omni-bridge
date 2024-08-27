use std::str::FromStr;
use near_plugins::{
    access_control, AccessControlRole, AccessControllable, Pausable,
    Upgradable, pause
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{AccountId, env, ext_contract, near, near_bindgen, Gas, PanicOnDefault, Promise, PromiseError};
use near_sdk::json_types::U128;
use omni_types::{OmniAddress, ProofResult, TransferMessage};
use omni_types::token_unlock_event::TokenUnlockedEvent;

/// Gas to call verify_log_entry on prover.
pub const VERIFY_LOG_ENTRY_GAS: Gas = Gas::from_tgas(50);

#[ext_contract(ext_prover)]
pub trait Prover {
    #[result_serializer(borsh)]
    fn verify_log_entry(
        &self,
        #[serializer(borsh)] log_index: u64,
        #[serializer(borsh)] log_entry_data: Vec<u8>,
        #[serializer(borsh)] receipt_index: u64,
        #[serializer(borsh)] receipt_data: Vec<u8>,
        #[serializer(borsh)] header_data: Vec<u8>,
        #[serializer(borsh)] proof: Vec<Vec<u8>>,
        #[serializer(borsh)] skip_bridge_call: bool,
    ) -> bool;
}

#[derive(Default, BorshDeserialize, BorshSerialize, Clone, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Proof {
    pub log_index: u64,
    pub log_entry_data: Vec<u8>,
    pub receipt_index: u64,
    pub receipt_data: Vec<u8>,
    pub header_data: Vec<u8>,
    pub proof: Vec<Vec<u8>>,
}



#[derive(AccessControlRole, Deserialize, Serialize, Copy, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum Role {
    PauseManager,
    UpgradableCodeStager,
    UpgradableCodeDeployer,
    DAO,
    UnrestrictedValidateProof,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault, Pausable, Upgradable)]
#[access_control(role_type(Role))]
#[pausable(manager_roles(Role::PauseManager, Role::DAO))]
#[upgradable(access_control_roles(
    code_stagers(Role::UpgradableCodeStager, Role::DAO),
    code_deployers(Role::UpgradableCodeDeployer, Role::DAO),
    duration_initializers(Role::DAO),
    duration_update_stagers(Role::DAO),
    duration_update_appliers(Role::DAO),
))]
pub struct RainbowOmniProverProxy {
    pub prover_account: AccountId,
}

#[near_bindgen]
impl RainbowOmniProverProxy {
    #[init]
    #[private]
    #[must_use]
    pub fn init(prover_account: AccountId) -> Self {
        let mut contract = Self {
            prover_account
        };

        contract.acl_init_super_admin(near_sdk::env::predecessor_account_id());
        contract
    }

    #[pause(except(roles(Role::UnrestrictedValidateProof, Role::DAO)))]
    pub fn verify_proof(
        &self,
        msg: Vec<u8>,
    ) -> Promise {
        let proof = Proof::try_from_slice(&msg).unwrap_or_else(|_| env::panic_str("ErrorOnProofParsing"));

        ext_prover::ext(self.prover_account.clone())
            .with_static_gas(VERIFY_LOG_ENTRY_GAS)
            .verify_log_entry(
                proof.log_index,
                proof.log_entry_data.clone(),
                proof.receipt_index,
                proof.receipt_data,
                proof.header_data,
                proof.proof,
                false, // Do not skip bridge call. This is only used for development and diagnostics.
            ).then(
                Self::ext(env::current_account_id())
                    .with_static_gas(VERIFY_LOG_ENTRY_GAS)
                    .verify_log_entry_callback(proof.log_entry_data)
            )
    }

    #[private]
    pub fn verify_log_entry_callback(
        &mut self,
        log_entry_data: Vec<u8>,
        #[callback_result] is_valid: Result<bool, PromiseError>,
    ) -> ProofResult {
        if !is_valid.unwrap_or(false) {
            panic!("Proof is not valid!")
        }

        let event = TokenUnlockedEvent::from_log_entry_data(&log_entry_data);

        return ProofResult::InitTransfer(
            TransferMessage {
                origin_nonce: U128::from(0),
                token: AccountId::from_str(&event.token).unwrap_or_else(|_| env::panic_str("ErrorOnTokenAccountParsing")),
                amount: U128::from(event.amount),
                recipient: OmniAddress::from_str(&event.recipient).unwrap_or(OmniAddress::Near(event.recipient)),
                fee: U128::from(0),
                sender: OmniAddress::from_str(&event.sender).unwrap_or(OmniAddress::Eth(event.sender.parse().unwrap_or_else(|_| env::panic_str("ErrorOnSenderParsing"))))
            }
        );
    }
}