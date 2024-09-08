#[cfg(test)]
mod tests {
    use near_sdk::{borsh, json_types::U128, serde_json::json, AccountId};
    use near_workspaces::types::NearToken;
    use omni_types::{
        locker_args::{FinTransferArgs, StorageDepositArgs},
        prover_result::{InitTransferMessage, ProverResult},
        OmniAddress, TransferMessage,
    };

    const MOCK_TOKEN_PATH: &str = "./../target/wasm32-unknown-unknown/release/mock_token.wasm";
    const MOCK_PROVER_PATH: &str = "./../target/wasm32-unknown-unknown/release/mock_prover.wasm";
    const LOCKER_PATH: &str = "./../target/wasm32-unknown-unknown/release/nep141_locker.wasm";
    const NEP141_DEPOSIT: NearToken = NearToken::from_yoctonear(1250000000000000000000);

    fn relayer_account_id() -> AccountId {
        "relayer".parse().unwrap()
    }

    fn account_1() -> AccountId {
        "account_1".parse().unwrap()
    }

    fn eth_factory_address() -> OmniAddress {
        "eth:0x252e87862A3A720287E7fd527cE6e8d0738427A2"
            .parse()
            .unwrap()
    }

    fn eth_eoa_address() -> OmniAddress {
        "eth:0xc5ed912ca6db7b41de4ef3632fa0a5641e42bf09"
            .parse()
            .unwrap()
    }

    #[tokio::test]
    async fn test_fin_transfer_storage_deposit() {
        struct TestStorageDeposit<'a> {
            storage_deposit_accounts: Vec<(AccountId, bool)>,
            error: Option<&'a str>,
        }
        let test_data = [
            TestStorageDeposit {
                storage_deposit_accounts: [(account_1(), true), (relayer_account_id(), true)]
                    .to_vec(),
                error: None,
            },
            TestStorageDeposit {
                storage_deposit_accounts: [(account_1(), false), (relayer_account_id(), false)]
                    .to_vec(),
                error: Some("STORAGE_ERR: The transfer recipient was omitted"),
            },
            TestStorageDeposit {
                storage_deposit_accounts: [(account_1(), true), (relayer_account_id(), false)]
                    .to_vec(),
                error: Some("STORAGE_ERR: The fee recipient was omitted"),
            },
            TestStorageDeposit {
                storage_deposit_accounts: [(account_1(), false), (relayer_account_id(), true)]
                    .to_vec(),
                error: Some("STORAGE_ERR: The transfer recipient was omitted"),
            },
        ];

        for test in test_data.into_iter().enumerate() {
            let result = test_fin_transfer(test.1.storage_deposit_accounts).await;

            match result {
                Ok(_) => assert!(test.1.error.is_none()),
                Err(e) => assert!(
                    e.to_string().contains(test.1.error.unwrap()),
                    "Test index: {}, err: {}",
                    test.0,
                    e
                ),
            }
        }
    }

    async fn test_fin_transfer(
        storage_deposit_accounts: Vec<(AccountId, bool)>,
    ) -> anyhow::Result<()> {
        let worker = near_workspaces::sandbox().await?;
        let token_contract = worker.dev_deploy(&std::fs::read(MOCK_TOKEN_PATH)?).await?;
        token_contract
            .call("new_default_meta")
            .args_json(json!({
                "owner_id": token_contract.id(),
                "total_supply": U128(u128::MAX)
            }))
            .max_gas()
            .transact()
            .await?
            .unwrap();

        let prover_contract = worker.dev_deploy(&std::fs::read(MOCK_PROVER_PATH)?).await?;
        let locker_contract = worker.dev_deploy(&std::fs::read(LOCKER_PATH)?).await?;

        let (_, sk) = worker.dev_generate().await;
        let relayer_account = worker.create_tla(relayer_account_id(), sk).await?.unwrap();

        worker
            .root_account()
            .unwrap()
            .transfer_near(locker_contract.id(), NearToken::from_near(10))
            .await?
            .unwrap();

        locker_contract
            .call("new")
            .args_json(json!({
                "prover_account": prover_contract.id(),
                "mpc_signer": "mpc.testnet",
                "nonce": U128(0)
            }))
            .max_gas()
            .transact()
            .await?
            .into_result()?;

        token_contract
            .call("storage_deposit")
            .args_json(json!({
                "account_id": locker_contract.id(),
                "registration_only": true,
            }))
            .deposit(NEP141_DEPOSIT)
            .max_gas()
            .transact()
            .await?
            .into_result()?;

        // Transfer tokens
        token_contract
            .call("ft_transfer")
            .args_json(json!({
                "receiver_id": locker_contract.id(),
                "amount": U128(1000),
            }))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await?
            .into_result()?;

        // Add factory
        locker_contract
            .call("add_factory")
            .args_json(json!({
                "address": eth_factory_address(),
            }))
            .max_gas()
            .transact()
            .await?
            .into_result()?;

        // Fin transfer
        relayer_account
            .call(locker_contract.id(), "fin_transfer")
            .args_borsh(FinTransferArgs {
                chain_kind: omni_types::ChainKind::Eth,
                storage_deposit_args: StorageDepositArgs {
                    token: token_contract.id().clone(),
                    accounts: storage_deposit_accounts,
                },
                prover_args: borsh::to_vec(&ProverResult::InitTransfer(InitTransferMessage {
                    emitter_address: eth_factory_address(),
                    transfer: TransferMessage {
                        origin_nonce: U128(1),
                        token: token_contract.id().clone(),
                        recipient: OmniAddress::Near(account_1().to_string()),
                        amount: U128(999),
                        fee: U128(1),
                        sender: eth_eoa_address(),
                    },
                }))
                .unwrap(),
            })
            .deposit(NEP141_DEPOSIT.saturating_mul(2))
            .max_gas()
            .transact()
            .await?
            .into_result()?;

        Ok(())
    }
}
