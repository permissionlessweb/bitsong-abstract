use ::module_factory::error::ModuleFactoryError;
use ::registry::error::RegistryError;
use abstract_adapter::{
    mock::{self, MockError, MockExecMsg, MockInitMsg},
    AdapterError,
};
use abstract_integration_tests::{
    add_mock_adapter_install_fee, create_default_account, init_mock_adapter, install_adapter,
    install_adapter_with_funds, mock_modules,
    mock_modules::adapter_1::{MockAdapterI1V1, MockAdapterI1V2},
    AResult,
};
use abstract_interface::*;
use abstract_std::{
    account::{AccountModuleInfo, ModuleInstallConfig},
    adapter::{AdapterBaseMsg, AdapterRequestMsg, BaseExecuteMsg, BaseQueryMsgFns},
    objects::{
        fee::FixedFee,
        module::{ModuleInfo, ModuleVersion, Monetization},
    },
    *,
};
use abstract_testing::prelude::*;
use cosmwasm_std::{coin, coins};
use cw_orch::prelude::*;
use mock_modules::{adapter_1, V1, V2};

#[test]
fn installing_one_adapter_should_succeed() -> AResult {
    let chain = MockBech32::new("mock");
    let sender = chain.sender_addr();
    let deployment = Abstract::deploy_on(chain.clone(), ())?;
    let account = create_default_account(&sender, &deployment)?;
    let staking_adapter = init_mock_adapter(chain.clone(), &deployment, None, account.id()?)?;
    install_adapter(&account, TEST_MODULE_ID)?;

    let modules = account.expect_modules(vec![staking_adapter.address()?.to_string()])?;

    assert_eq!(
        modules[0],
        AccountModuleInfo {
            address: staking_adapter.address()?,
            id: TEST_MODULE_ID.to_string(),
            version: cw2::ContractVersion {
                contract: TEST_MODULE_ID.into(),
                version: TEST_VERSION.into(),
            },
        },
    );

    // Configuration is correct
    let adapter_config = staking_adapter.base_config()?;
    assert_eq!(
        adapter_config,
        adapter::AdapterConfigResponse {
            ans_host_address: deployment.ans_host.address()?,
            dependencies: vec![],
            registry_address: deployment.registry.address()?,
        }
    );

    // no authorized addresses registered
    let authorized = staking_adapter.authorized_addresses(account.addr_str()?)?;
    assert_eq!(
        authorized,
        adapter::AuthorizedAddressesResponse { addresses: vec![] }
    );

    take_storage_snapshot!(chain, "install_one_adapter");

    Ok(())
}

#[test]
fn installing_one_adapter_without_fee_should_fail() -> AResult {
    let chain = MockBech32::new("mock");
    let sender = chain.sender_addr();
    chain.set_balance(&sender, coins(12, "ujunox"))?;
    let deployment = Abstract::deploy_on(chain.clone(), ())?;
    let account = create_default_account(&sender, &deployment)?;
    init_mock_adapter(chain.clone(), &deployment, None, account.id()?)?;
    add_mock_adapter_install_fee(
        &deployment,
        Monetization::InstallFee(FixedFee::new(&coin(45, "ujunox"))),
        None,
    )?;

    let without_funds = install_adapter(&account, TEST_MODULE_ID)
        .unwrap_err()
        .downcast::<AbstractInterfaceError>()
        .unwrap()
        .downcast::<ModuleFactoryError>()
        .unwrap();

    assert!(matches!(
        without_funds,
        ModuleFactoryError::Abstract(AbstractError::Fee(_))
    ));

    let with_low_funds = install_adapter_with_funds(&account, TEST_MODULE_ID, &coins(12, "ujunox"))
        .unwrap_err()
        .downcast::<AbstractInterfaceError>()
        .unwrap()
        .downcast::<ModuleFactoryError>()
        .unwrap();

    assert!(matches!(
        with_low_funds,
        ModuleFactoryError::Abstract(AbstractError::Fee(_))
    ));

    Ok(())
}

#[test]
fn installing_one_adapter_with_fee_should_succeed() -> AResult {
    let chain = MockBech32::new("mock");
    Abstract::deploy_on(chain.clone(), ())?;
    abstract_integration_tests::account::installing_one_adapter_with_fee_should_succeed(
        chain.clone(),
    )?;
    take_storage_snapshot!(chain, "install_one_adapter_with_fee");
    Ok(())
}

#[test]
fn install_non_existent_adapterid_should_fail() -> AResult {
    let chain = MockBech32::new("mock");
    let sender = chain.sender_addr();
    let deployment = Abstract::deploy_on(chain.clone(), ())?;
    let account = create_default_account(&sender, &deployment)?;

    let res = install_adapter(&account, "lol:no_chance");

    assert!(res.unwrap_err().root_cause().to_string().contains(
        &RegistryError::ModuleNotFound(ModuleInfo::from_id_latest("lol:no_chance").unwrap())
            .to_string(),
    ));
    Ok(())
}

#[test]
fn install_non_existent_version_should_fail() -> AResult {
    let chain = MockBech32::new("mock");
    let sender = chain.sender_addr();
    let deployment = Abstract::deploy_on(chain.clone(), ())?;
    let account = create_default_account(&sender, &deployment)?;
    init_mock_adapter(chain, &deployment, None, account.id()?)?;

    let res = account.install_module_version(
        TEST_MODULE_ID,
        ModuleVersion::Version("1.2.3".to_string()),
        Some(&Empty {}),
        &[],
    );

    assert!(res.unwrap_err().root().to_string().contains(
        &RegistryError::ModuleNotFound(
            ModuleInfo::from_id(TEST_MODULE_ID, ModuleVersion::Version("1.2.3".to_string()))
                .unwrap(),
        )
        .to_string(),
    ));
    Ok(())
}

#[test]
fn installation_of_duplicate_adapter_should_fail() -> AResult {
    let chain = MockBech32::new("mock");
    let sender = chain.sender_addr();
    let deployment = Abstract::deploy_on(chain.clone(), ())?;
    let account = create_default_account(&sender, &deployment)?;
    let staking_adapter = init_mock_adapter(chain, &deployment, None, account.id()?)?;

    install_adapter(&account, TEST_MODULE_ID)?;

    let modules = account.expect_modules(vec![staking_adapter.address()?.to_string()])?;

    // assert account module
    // check staking adapter
    assert_eq!(
        modules[0],
        AccountModuleInfo {
            address: staking_adapter.address()?,
            id: TEST_MODULE_ID.to_string(),
            version: cw2::ContractVersion {
                contract: TEST_MODULE_ID.into(),
                version: TEST_VERSION.into(),
            },
        },
    );

    // install again
    let second_install_res = install_adapter(&account, TEST_MODULE_ID);
    assert!(second_install_res
        .unwrap_err()
        .to_string()
        .contains("test-module-id"));

    account.expect_modules(vec![staking_adapter.address()?.to_string()])?;

    Ok(())
}

#[test]
fn reinstalling_adapter_should_be_allowed() -> AResult {
    let chain = MockBech32::new("mock");
    let sender = chain.sender_addr();
    let deployment = Abstract::deploy_on(chain.clone(), ())?;
    let account = create_default_account(&sender, &deployment)?;
    let staking_adapter = init_mock_adapter(chain.clone(), &deployment, None, account.id()?)?;

    install_adapter(&account, TEST_MODULE_ID)?;

    let modules = account.expect_modules(vec![staking_adapter.address()?.to_string()])?;

    // check staking adapter
    assert_eq!(
        modules[0],
        AccountModuleInfo {
            address: staking_adapter.address()?,
            id: TEST_MODULE_ID.to_string(),
            version: cw2::ContractVersion {
                contract: TEST_MODULE_ID.into(),
                version: TEST_VERSION.into(),
            },
        },
    );

    // uninstall
    account.uninstall_module(TEST_MODULE_ID.to_string())?;

    // None expected
    account.expect_modules(vec![])?;

    // reinstall
    install_adapter(&account, TEST_MODULE_ID)?;

    account.expect_modules(vec![staking_adapter.address()?.to_string()])?;
    take_storage_snapshot!(chain, "reinstalling_adapter_should_be_allowed");

    Ok(())
}

/// Reinstalling the Adapter should install the latest version
#[test]
fn reinstalling_new_version_should_install_latest() -> AResult {
    let chain = MockBech32::new("mock");
    let sender = chain.sender_addr();
    let deployment = Abstract::deploy_on(chain.clone(), ())?;
    let account = create_default_account(&sender, &deployment)?;
    deployment
        .registry
        .claim_namespace(TEST_ACCOUNT_ID, "tester".to_string())?;

    let adapter1 = MockAdapterI1V1::new_test(chain.clone());
    adapter1
        .deploy(V1.parse().unwrap(), MockInitMsg {}, DeployStrategy::Try)
        .unwrap();

    install_adapter(&account, &adapter1.id())?;

    let modules = account.expect_modules(vec![adapter1.address()?.to_string()])?;

    // check staking adapter
    assert_eq!(
        modules[0],
        AccountModuleInfo {
            address: adapter1.address()?,
            id: adapter1.id(),
            version: cw2::ContractVersion {
                contract: adapter1.id(),
                version: V1.into(),
            },
        },
    );

    // uninstall tendermint staking
    account.uninstall_module(adapter1.id())?;

    account.expect_modules(vec![])?;

    let old_adapter_addr = adapter1.address()?;

    let adapter2 = MockAdapterI1V2::new_test(chain.clone());

    adapter2
        .deploy(V2.parse().unwrap(), MockInitMsg {}, DeployStrategy::Try)
        .unwrap();

    // check that the latest staking version is the new one
    let latest_staking = deployment
        .registry
        .module(ModuleInfo::from_id_latest(&adapter1.id())?)?;
    assert_eq!(
        latest_staking.info.version,
        ModuleVersion::Version(V2.to_string())
    );

    // reinstall
    install_adapter(&account, &adapter2.id())?;

    let modules = account.expect_modules(vec![adapter2.address()?.to_string()])?;

    assert_eq!(
        modules[0],
        AccountModuleInfo {
            // the address stored for MockAdapterI was updated when we instantiated the new version, so this is the new address
            address: adapter2.address()?,
            id: adapter2.id(),
            version: cw2::ContractVersion {
                contract: adapter2.id(),
                // IMPORTANT: The version of the contract did not change although the version of the module in registry did.
                // Beware of this distinction. The version of the contract is the version that's imbedded into the contract's wasm on compilation.
                version: V2.to_string(),
            },
        }
    );
    // assert that the new staking adapter has a different address
    assert_ne!(old_adapter_addr, adapter2.address()?);

    assert_eq!(modules[0].address, adapter2.as_instance().address()?);
    take_storage_snapshot!(chain, "reinstalling_new_version_should_install_latest");

    Ok(())
}

// struct TestModule = AppContract

#[test]
fn unauthorized_exec() -> AResult {
    let chain = MockBech32::new("mock");
    let sender = chain.sender_addr();
    let unauthorized = chain.addr_make("unauthorized");
    let deployment = Abstract::deploy_on(chain.clone(), ())?;
    let account = create_default_account(&sender, &deployment)?;
    let staking_adapter = init_mock_adapter(chain.clone(), &deployment, None, account.id()?)?;
    install_adapter(&account, TEST_MODULE_ID)?;
    // non-authorized address cannot execute
    let res = staking_adapter
        .call_as(&unauthorized)
        .execute(&MockExecMsg {}.into(), &[])
        .unwrap_err();
    assert!(res.root().to_string().contains(&format!(
        "Sender: {} of request to tester:test-module-id is not an Account or Authorized Address",
        unauthorized
    )));
    // neither can the ROOT directly
    let res = staking_adapter
        .execute(&MockExecMsg {}.into(), &[])
        .unwrap_err();
    assert!(res.root().to_string().contains(&format!(
        "Sender: {} of request to tester:test-module-id is not an Account or Authorized Address",
        chain.sender_addr()
    )));
    Ok(())
}

#[test]
fn account_adapter_exec() -> AResult {
    let chain = MockBech32::new("mock");
    let sender = chain.sender_addr();
    let deployment = Abstract::deploy_on(chain.clone(), ())?;
    let account = create_default_account(&sender, &deployment)?;
    let _staking_adapter_one = init_mock_adapter(chain.clone(), &deployment, None, account.id()?)?;

    install_adapter(&account, TEST_MODULE_ID)?;

    chain.set_balance(&account.address()?, vec![Coin::new(100_000u128, TTOKEN)])?;

    account.execute_on_module(
        TEST_MODULE_ID,
        Into::<abstract_std::adapter::ExecuteMsg<MockExecMsg>>::into(MockExecMsg {}),
        vec![],
    )?;

    Ok(())
}

#[test]
fn installing_specific_version_should_install_expected() -> AResult {
    let chain = MockBech32::new("mock");
    let sender = chain.sender_addr();
    let deployment = Abstract::deploy_on(chain.clone(), ())?;
    let account = create_default_account(&sender, &deployment)?;
    deployment
        .registry
        .claim_namespace(TEST_ACCOUNT_ID, "tester".to_string())?;

    let adapter1 = MockAdapterI1V1::new_test(chain.clone());
    adapter1
        .deploy(V1.parse().unwrap(), MockInitMsg {}, DeployStrategy::Try)
        .unwrap();

    let v1_adapter_addr = adapter1.address()?;

    let adapter2 = MockAdapterI1V2::new_test(chain.clone());

    adapter2
        .deploy(V2.parse().unwrap(), MockInitMsg {}, DeployStrategy::Try)
        .unwrap();

    let expected_version = "1.0.0".to_string();

    // install specific version
    account.install_module_version(
        &adapter1.id(),
        ModuleVersion::Version(expected_version),
        Some(&MockInitMsg {}),
        &[],
    )?;

    let modules = account.expect_modules(vec![v1_adapter_addr.to_string()])?;
    let installed_module: AccountModuleInfo = modules[0].clone();
    assert_eq!(installed_module.id, adapter1.id());
    take_storage_snapshot!(chain, "installing_specific_version_should_install_expected");

    Ok(())
}

#[test]
fn account_install_adapter() -> AResult {
    let chain = MockBech32::new("mock");
    let sender = chain.sender_addr();
    let deployment = Abstract::deploy_on(chain.clone(), ())?;
    let account = create_default_account(&sender, &deployment)?;

    deployment
        .registry
        .claim_namespace(TEST_ACCOUNT_ID, "tester".to_owned())?;

    let adapter = MockAdapterI1V1::new_test(chain.clone());
    adapter.deploy(V1.parse().unwrap(), MockInitMsg {}, DeployStrategy::Try)?;
    let adapter_addr = account.install_adapter(&adapter, &[])?;
    let module_addr = account
        .module_info(adapter_1::MOCK_ADAPTER_ID)?
        .unwrap()
        .address;
    assert_eq!(adapter_addr, module_addr);
    take_storage_snapshot!(chain, "account_install_adapter");
    Ok(())
}

#[test]
fn account_adapter_ownership() -> AResult {
    let chain = MockBech32::new("mock");
    let deployment = Abstract::deploy_on(chain.clone(), ())?;

    let sender = chain.sender();
    let account = create_default_account(sender, &deployment)?;

    deployment
        .registry
        .claim_namespace(TEST_ACCOUNT_ID, "tester".to_owned())?;

    let adapter = MockAdapterI1V1::new_test(chain.clone());
    adapter.deploy(V1.parse().unwrap(), MockInitMsg {}, DeployStrategy::Try)?;
    account.install_adapter(&adapter, &[])?;

    let account_addr = account.address()?;

    // Checking module requests

    // Can call either by account owner or account
    adapter.call_as(sender).execute(
        &mock::ExecuteMsg::Module(AdapterRequestMsg {
            account_address: Some(account_addr.to_string()),
            request: MockExecMsg {},
        }),
        &[],
    )?;
    adapter.call_as(&account.address()?).execute(
        &mock::ExecuteMsg::Module(AdapterRequestMsg {
            account_address: Some(account_addr.to_string()),
            request: MockExecMsg {},
        }),
        &[],
    )?;

    // Not admin or account
    let who = chain.addr_make("who");
    let err: MockError = adapter
        .call_as(&who)
        .execute(
            &mock::ExecuteMsg::Module(AdapterRequestMsg {
                account_address: Some(account_addr.to_string()),
                request: MockExecMsg {},
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        MockError::Adapter(AdapterError::UnauthorizedAddressAdapterRequest {
            adapter: adapter_1::MOCK_ADAPTER_ID.to_owned(),
            sender: who.to_string()
        })
    );

    // Checking base requests

    // Can call either by account owner or account
    adapter.call_as(sender).execute(
        &mock::ExecuteMsg::Base(BaseExecuteMsg {
            account_address: Some(account_addr.to_string()),
            msg: AdapterBaseMsg::UpdateAuthorizedAddresses {
                to_add: vec![chain.addr_make("123").to_string()],
                to_remove: vec![],
            },
        }),
        &[],
    )?;

    account.call_as(sender).admin_execute(
        adapter.address()?,
        to_json_binary(&mock::ExecuteMsg::Base(BaseExecuteMsg {
            account_address: Some(account_addr.to_string()),
            msg: AdapterBaseMsg::UpdateAuthorizedAddresses {
                to_add: vec![chain.addr_make("234").to_string()],
                to_remove: vec![],
            },
        }))?,
        &[],
    )?;

    // Raw account without the calling_to_as_admin variable set, should err.
    adapter
        .call_as(&account.address()?)
        .execute(
            &mock::ExecuteMsg::Base(BaseExecuteMsg {
                account_address: Some(account_addr.to_string()),
                msg: AdapterBaseMsg::UpdateAuthorizedAddresses {
                    to_add: vec![chain.addr_make("456").to_string()],
                    to_remove: vec![],
                },
            }),
            &[],
        )
        .unwrap_err();

    // Not admin or account
    let err: MockError = adapter
        .call_as(&who)
        .execute(
            &mock::ExecuteMsg::Base(BaseExecuteMsg {
                account_address: Some(account_addr.to_string()),
                msg: AdapterBaseMsg::UpdateAuthorizedAddresses {
                    to_add: vec![chain.addr_make("345").to_string()],
                    to_remove: vec![],
                },
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        MockError::Adapter(AdapterError::UnauthorizedAdapterRequest {
            adapter: adapter_1::MOCK_ADAPTER_ID.to_owned(),
            sender: who.to_string()
        })
    );

    Ok(())
}

#[test]
fn subaccount_adapter_ownership() -> AResult {
    let chain = MockBech32::new("mock");
    let sender = chain.sender_addr();
    let deployment = Abstract::deploy_on(chain.clone(), ())?;
    let account = create_default_account(&sender, &deployment)?;

    deployment
        .registry
        .claim_namespace(TEST_ACCOUNT_ID, "tester".to_owned())?;

    let adapter = MockAdapterI1V1::new_test(chain.clone());
    adapter.deploy(V1.parse().unwrap(), MockInitMsg {}, DeployStrategy::Try)?;

    let sub_account = account.create_and_return_sub_account(
        AccountDetails {
            name: "My subaccount".to_string(),
            description: None,
            link: None,
            namespace: None,
            install_modules: vec![ModuleInstallConfig::new(
                ModuleInfo::from_id_latest(adapter_1::MOCK_ADAPTER_ID).unwrap(),
                None,
            )],
            account_id: None,
        },
        &[],
    )?;

    let module = sub_account
        .module_info(adapter_1::MOCK_ADAPTER_ID)?
        .unwrap();
    adapter.set_address(&module.address);

    let account_addr = sub_account.address()?;

    // Checking module requests

    // Can call either by account owner or account
    adapter.call_as(&sender).execute(
        &mock::ExecuteMsg::Module(AdapterRequestMsg {
            account_address: Some(account_addr.to_string()),
            request: MockExecMsg {},
        }),
        &[],
    )?;
    sub_account.call_as(&sender).admin_execute(
        adapter.address()?,
        to_json_binary(&mock::ExecuteMsg::Module(AdapterRequestMsg {
            account_address: Some(account_addr.to_string()),
            request: MockExecMsg {},
        }))?,
        &[],
    )?;

    // Raw account without the calling_to_as_admin variable set, should err
    adapter
        .call_as(&account.address()?)
        .execute(
            &mock::ExecuteMsg::Base(BaseExecuteMsg {
                account_address: Some(account_addr.to_string()),
                msg: AdapterBaseMsg::UpdateAuthorizedAddresses {
                    to_add: vec![chain.addr_make("456").to_string()],
                    to_remove: vec![],
                },
            }),
            &[],
        )
        .unwrap_err();

    // Not admin or account
    let who = chain.addr_make("who");
    let err: MockError = adapter
        .call_as(&who)
        .execute(
            &mock::ExecuteMsg::Module(AdapterRequestMsg {
                account_address: Some(account_addr.to_string()),
                request: MockExecMsg {},
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        MockError::Adapter(AdapterError::UnauthorizedAddressAdapterRequest {
            adapter: adapter_1::MOCK_ADAPTER_ID.to_owned(),
            sender: who.to_string()
        })
    );

    // Checking base requests

    // Can call either by account owner or account
    adapter.call_as(&sender).execute(
        &mock::ExecuteMsg::Base(BaseExecuteMsg {
            account_address: Some(account_addr.to_string()),
            msg: AdapterBaseMsg::UpdateAuthorizedAddresses {
                to_add: vec![chain.addr_make("123").to_string()],
                to_remove: vec![],
            },
        }),
        &[],
    )?;
    sub_account.call_as(&sender).admin_execute(
        adapter.address()?,
        to_json_binary(&mock::ExecuteMsg::Base(BaseExecuteMsg {
            account_address: Some(account_addr.to_string()),
            msg: AdapterBaseMsg::UpdateAuthorizedAddresses {
                to_add: vec![chain.addr_make("234").to_string()],
                to_remove: vec![],
            },
        }))?,
        &[],
    )?;

    // Raw account without the calling_to_as_admin variable set, should err
    adapter
        .call_as(&sub_account.address()?)
        .execute(
            &mock::ExecuteMsg::Base(BaseExecuteMsg {
                account_address: Some(account_addr.to_string()),
                msg: AdapterBaseMsg::UpdateAuthorizedAddresses {
                    to_add: vec![chain.addr_make("345").to_string()],
                    to_remove: vec![],
                },
            }),
            &[],
        )
        .unwrap_err();

    // Not admin or account
    let err: MockError = adapter
        .call_as(&who)
        .execute(
            &mock::ExecuteMsg::Base(BaseExecuteMsg {
                account_address: Some(account_addr.to_string()),
                msg: AdapterBaseMsg::UpdateAuthorizedAddresses {
                    to_add: vec![chain.addr_make("345").to_string()],
                    to_remove: vec![],
                },
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        MockError::Adapter(AdapterError::UnauthorizedAdapterRequest {
            adapter: adapter_1::MOCK_ADAPTER_ID.to_owned(),
            sender: who.to_string()
        })
    );
    Ok(())
}
