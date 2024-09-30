use alloy_primitives::Log;
use alloy_rlp::Decodable;
use alloy_sol_types::{sol, SolEvent};

use crate::{
    prover_result::{DeployTokenMessage, FinTransferMessage, InitTransferMessage},
    stringify, ChainKind, OmniAddress, TransferMessage, H160,
};

const ERR_INVALIDE_SIGNATURE_HASH: &str = "ERR_INVALIDE_SIGNATURE_HASH";

sol! {
    event InitTransfer(
        address indexed sender,
        address indexed tokenAddress,
        uint128 indexed nonce,
        string token,
        uint128 amount,
        uint128 fee,
        string recipient
    );

    event FinTransfer(
        uint128 indexed nonce,
        string token,
        uint128 amount,
        address recipient,
        string feeRecipient
    );

    event DeployToken(
        address indexed tokenAddress,
        string token,
        string name,
        string symbol,
        uint8 decimals
    );
}

pub fn parse_evm_event<T: SolEvent, V: TryFromLog<Log<T>>>(
    chain_kind: ChainKind,
    log_rlp: Vec<u8>,
) -> Result<V, String>
where
    <V as TryFromLog<Log<T>>>::Error: std::fmt::Display,
{
    let rlp_decoded = Log::decode(&mut log_rlp.as_slice()).map_err(stringify)?;
    V::try_from_log(
        chain_kind,
        T::decode_log(&rlp_decoded, true).map_err(stringify)?,
    )
    .map_err(stringify)
}

pub trait TryFromLog<T>: Sized {
    type Error;
    fn try_from_log(chain_kind: ChainKind, value: T) -> Result<Self, Self::Error>;
}

impl TryFromLog<Log<FinTransfer>> for FinTransferMessage {
    type Error = String;

    fn try_from_log(chain_kind: ChainKind, event: Log<FinTransfer>) -> Result<Self, Self::Error> {
        if event.topics().0 != FinTransfer::SIGNATURE_HASH {
            return Err(ERR_INVALIDE_SIGNATURE_HASH.to_string());
        }

        Ok(FinTransferMessage {
            nonce: near_sdk::json_types::U128(event.data.nonce),
            amount: near_sdk::json_types::U128(event.data.amount),
            fee_recipient: event.data.feeRecipient.parse().map_err(stringify)?,
            emitter_address: OmniAddress::from_evm_address(chain_kind, H160(event.address.into()))?,
        })
    }
}

impl TryFromLog<Log<InitTransfer>> for InitTransferMessage {
    type Error = String;

    fn try_from_log(chain_kind: ChainKind, event: Log<InitTransfer>) -> Result<Self, Self::Error> {
        if event.topics().0 != InitTransfer::SIGNATURE_HASH {
            return Err(ERR_INVALIDE_SIGNATURE_HASH.to_string());
        }

        Ok(InitTransferMessage {
            emitter_address: OmniAddress::from_evm_address(chain_kind, H160(event.address.into()))?,
            transfer: TransferMessage {
                origin_nonce: near_sdk::json_types::U128(event.data.nonce),
                token: event.data.token.parse().map_err(stringify)?,
                amount: near_sdk::json_types::U128(event.data.amount),
                recipient: event.data.recipient.parse().map_err(stringify)?,
                fee: near_sdk::json_types::U128(event.data.fee),
                sender: OmniAddress::from_evm_address(chain_kind, H160(event.data.sender.into()))?,
            },
        })
    }
}

impl TryFromLog<Log<DeployToken>> for DeployTokenMessage {
    type Error = String;

    fn try_from_log(chain_kind: ChainKind, event: Log<DeployToken>) -> Result<Self, Self::Error> {
        if event.topics().0 != DeployToken::SIGNATURE_HASH {
            return Err(ERR_INVALIDE_SIGNATURE_HASH.to_string());
        }

        Ok(DeployTokenMessage {
            emitter_address: OmniAddress::from_evm_address(chain_kind, H160(event.address.into()))?,
            token: event.data.token.parse().map_err(stringify)?,
            token_address: OmniAddress::from_evm_address(
                chain_kind,
                H160(event.data.tokenAddress.into()),
            )?,
        })
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::IntoLogData;

    use super::*;
    sol! {
        event TestFinTransfer(
            uint128 indexed nonce,
            string token,
            uint128 amount,
            address recipient,
            string feeRecipient
        );
    }

    #[test]
    fn test_decode_log_with_same_params_with_validation() {
        let event = FinTransfer {
            nonce: 55,
            amount: 100,
            token: "some_token".to_owned(),
            recipient: [0; 20].into(),
            feeRecipient: "some_fee_recipient".to_owned(),
        };
        let test_event = TestFinTransfer {
            nonce: event.nonce,
            amount: event.amount,
            token: event.token.clone(),
            recipient: event.recipient,
            feeRecipient: event.feeRecipient.clone(),
        };
        let log = Log {
            address: [1; 20].into(),
            data: event.to_log_data(),
        };
        let test_log = Log {
            address: log.address,
            data: test_event.to_log_data(),
        };

        assert_ne!(log, test_log);

        let decoded_log = FinTransfer::decode_log(&log, true).unwrap();
        let decoded_test_log = TestFinTransfer::decode_log(&test_log, true).unwrap();

        assert_ne!(FinTransfer::SIGNATURE_HASH, TestFinTransfer::SIGNATURE_HASH);
        assert_eq!(FinTransfer::SIGNATURE_HASH, decoded_log.topics().0);
        assert_eq!(TestFinTransfer::SIGNATURE_HASH, decoded_test_log.topics().0);
    }
}