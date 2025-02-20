//! # Abstract Adapter
//!
//! Basis for an interfacing contract to an external service.
#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]
use cosmwasm_std::{Empty, Response};

pub type AdapterResult<C = Empty> = Result<Response<C>, AdapterError>;
// Default to Empty

pub use error::AdapterError;

pub use crate::state::AdapterContract;

// Useful re-exports
pub use abstract_std as std;
// re-export objects specifically
pub use abstract_sdk as sdk;
pub use abstract_std::objects;
pub mod traits {
    pub use abstract_sdk::{features::*, prelude::*};
}

mod interface;
pub use abstract_interface;
#[cfg(feature = "test-utils")]
pub use abstract_testing;

mod endpoints;
pub mod error;
/// Abstract SDK trait implementations
pub mod features;
mod handler;
pub mod msgs;
#[cfg(feature = "schema")]
pub mod schema;
pub mod state;

#[cfg(feature = "test-utils")]
pub mod mock {
    use abstract_sdk::{base::InstantiateEndpoint, AbstractSdkError};
    use abstract_std::{adapter::*, objects::dependency::StaticDependency};
    use abstract_testing::{mock_env_validated, prelude::*, TEST_VERSION};
    use cosmwasm_std::{testing::*, OwnedDeps, Response, StdError};
    use cw_storage_plus::Item;
    use thiserror::Error;

    use crate::{AdapterContract, AdapterError};

    crate::adapter_msg_types!(MockAdapterContract, MockExecMsg, MockQueryMsg);

    pub const TEST_METADATA: &str = "test_metadata";
    pub const TEST_AUTHORIZED_ADDR: &str = "test_authorized_address";

    #[derive(Error, Debug, PartialEq)]
    pub enum MockError {
        #[error(transparent)]
        Std(#[from] StdError),

        #[error(transparent)]
        Adapter(#[from] AdapterError),

        #[error(transparent)]
        Abstract(#[from] abstract_std::AbstractError),

        #[error(transparent)]
        AbstractSdk(#[from] AbstractSdkError),
    }

    #[cosmwasm_schema::cw_serde]
    pub struct MockInitMsg {}

    #[cosmwasm_schema::cw_serde]
    pub struct MockExecMsg {}

    #[cosmwasm_schema::cw_serde]
    #[derive(cw_orch::QueryFns, cosmwasm_schema::QueryResponses)]
    pub enum MockQueryMsg {
        #[returns(ReceivedIbcCallbackStatus)]
        GetReceivedIbcCallbackStatus {},

        #[returns(String)]
        GetSomething {},
    }

    #[cosmwasm_schema::cw_serde]
    pub struct ReceivedIbcCallbackStatus {
        pub received: bool,
    }

    #[cosmwasm_schema::cw_serde]
    pub struct MockSudoMsg {}

    /// Mock Adapter type
    pub type MockAdapterContract =
        AdapterContract<MockError, MockInitMsg, MockExecMsg, MockQueryMsg, MockSudoMsg>;

    pub const MOCK_DEP: StaticDependency = StaticDependency::new("module_id", &[">0.0.0"]);

    // Easy way to see if an ibc-callback was actually received.
    pub const IBC_CALLBACK_RECEIVED: Item<bool> = Item::new("ibc_callback_received");
    /// use for testing
    pub const MOCK_ADAPTER: MockAdapterContract =
        MockAdapterContract::new(TEST_MODULE_ID, TEST_VERSION, Some(TEST_METADATA))
            .with_instantiate(|deps, _, _, _, _| {
                IBC_CALLBACK_RECEIVED.save(deps.storage, &false)?;

                Ok(Response::new().set_data("mock_init".as_bytes()))
            })
            .with_execute(|_, _, _, _, _| Ok(Response::new().set_data("mock_exec".as_bytes())))
            .with_query(|deps, _, _, msg| match msg {
                MockQueryMsg::GetReceivedIbcCallbackStatus {} => {
                    to_json_binary(&ReceivedIbcCallbackStatus {
                        received: IBC_CALLBACK_RECEIVED.load(deps.storage)?,
                    })
                    .map_err(Into::into)
                }
                MockQueryMsg::GetSomething {} => to_json_binary("mock_query").map_err(Into::into),
            })
            .with_sudo(|_, _, _, _| Ok(Response::new().set_data("mock_sudo".as_bytes())))
            .with_ibc_callback(|deps, _, _, _, _| {
                IBC_CALLBACK_RECEIVED.save(deps.storage, &true).unwrap();
                Ok(Response::new().set_data("mock_callback".as_bytes()))
            })
            .with_replies(&[(1u64, |_, _, _, msg| {
                #[allow(deprecated)]
                Ok(Response::new().set_data(msg.result.unwrap().data.unwrap()))
            })]);

    pub type AdapterMockResult = Result<(), MockError>;
    // export these for upload usage
    crate::export_endpoints!(MOCK_ADAPTER, MockAdapterContract);

    crate::cw_orch_interface!(MOCK_ADAPTER, MockAdapterContract, MockInitMsg, MockAdapterI);
    pub fn mock_init(
        deps: &mut OwnedDeps<MockStorage, MockApi, MockQuerier>,
    ) -> Result<Response, MockError> {
        let adapter = MOCK_ADAPTER;
        let abstr = AbstractMockAddrs::new(deps.api);

        let info = message_info(&abstr.owner, &[]);
        let env = mock_env_validated(deps.api);
        let init_msg = InstantiateMsg {
            base: BaseInstantiateMsg {
                registry_address: abstr.registry.to_string(),
            },
            module: MockInitMsg {},
        };
        adapter.instantiate(deps.as_mut(), env, info, init_msg)
    }

    pub fn mock_init_custom(
        deps: &mut OwnedDeps<MockStorage, MockApi, MockQuerier>,
        module: MockAdapterContract,
    ) -> Result<Response, MockError> {
        let abstr = AbstractMockAddrs::new(deps.api);

        let info = message_info(&abstr.owner, &[]);
        let env = mock_env_validated(deps.api);
        let init_msg = InstantiateMsg {
            base: BaseInstantiateMsg {
                registry_address: abstr.registry.to_string(),
            },
            module: MockInitMsg {},
        };
        module.instantiate(deps.as_mut(), env, info, init_msg)
    }

    /// Generate a cw-orch instance for a mock adapter
    /// - $name: name of the contract (&str)
    /// - $id: id of the contract (&str)
    /// - $version: version of the contract (&str)
    /// - $deps: dependencies of the contract (&[StaticDependency])
    #[macro_export]
    macro_rules! gen_adapter_mock {
    ($name:ident, $id:expr, $version:expr, $deps:expr) => {
        use $crate::std::adapter::*;
        use $crate::sdk::base::Handler;
        use ::cosmwasm_std::Empty;
        use ::abstract_adapter::mock::{MockExecMsg, MockQueryMsg, MockInitMsg, MockAdapterContract, MockError};
        use ::cw_orch::environment::CwEnv;

        const MOCK_ADAPTER: ::abstract_adapter::mock::MockAdapterContract = ::abstract_adapter::mock::MockAdapterContract::new($id, $version, None)
        .with_dependencies($deps)
        .with_execute(|_, _, _, _, _| Ok(::cosmwasm_std::Response::new().set_data("mock_exec".as_bytes())));

        fn instantiate(
            deps: ::cosmwasm_std::DepsMut,
            env: ::cosmwasm_std::Env,
            info: ::cosmwasm_std::MessageInfo,
            msg: <::abstract_adapter::mock::MockAdapterContract as ::abstract_sdk::base::InstantiateEndpoint>::InstantiateMsg,
        ) -> Result<::cosmwasm_std::Response, <::abstract_adapter::mock::MockAdapterContract as ::abstract_sdk::base::Handler>::Error> {
            use ::abstract_sdk::base::InstantiateEndpoint;
            MOCK_ADAPTER.instantiate(deps, env, info, msg)
        }

        /// Execute entrypoint
        fn execute(
            deps: ::cosmwasm_std::DepsMut,
            env: ::cosmwasm_std::Env,
            info: ::cosmwasm_std::MessageInfo,
            msg: <::abstract_adapter::mock::MockAdapterContract as ::abstract_sdk::base::ExecuteEndpoint>::ExecuteMsg,
        ) -> Result<::cosmwasm_std::Response, <::abstract_adapter::mock::MockAdapterContract as ::abstract_sdk::base::Handler>::Error> {
            use ::abstract_sdk::base::ExecuteEndpoint;
            MOCK_ADAPTER.execute(deps, env, info, msg)
        }

        /// Query entrypoint
        fn query(
            deps: ::cosmwasm_std::Deps,
            env: ::cosmwasm_std::Env,
            msg: <::abstract_adapter::mock::MockAdapterContract as ::abstract_sdk::base::QueryEndpoint>::QueryMsg,
        ) -> Result<::cosmwasm_std::Binary, <::abstract_adapter::mock::MockAdapterContract as ::abstract_sdk::base::Handler>::Error> {
            use ::abstract_sdk::base::QueryEndpoint;
            MOCK_ADAPTER.query(deps, env, msg)
        }

        type Exec = $crate::std::adapter::ExecuteMsg<MockExecMsg>;
        type Query = $crate::std::adapter::QueryMsg<MockQueryMsg>;
        type Init = $crate::std::adapter::InstantiateMsg<MockInitMsg>;
        #[cw_orch::interface(Init, Exec, Query, Empty)]
        pub struct $name ;

        impl <T: ::cw_orch::prelude::CwEnv> ::abstract_interface::AdapterDeployer<T, MockInitMsg> for $name <T> {}

        impl<Chain> $crate::abstract_interface::RegisteredModule for $name<Chain> {
            type InitMsg = MockInitMsg;

            fn module_id<'a>() -> &'a str {
                $crate::traits::ModuleIdentification::module_id(&MOCK_ADAPTER)
            }

            fn module_version<'a>() -> &'a str {
                MOCK_ADAPTER.version()
            }

            fn dependencies<'a>() -> &'a [$crate::objects::dependency::StaticDependency] {
                MOCK_ADAPTER.dependencies()
            }
        }

        impl<T: ::cw_orch::prelude::CwEnv> Uploadable for $name<T> {
            fn wrapper() -> <Mock as ::cw_orch::environment::TxHandler>::ContractSource {
                Box::new(ContractWrapper::<
                    Exec,
                    _,
                    _,
                    _,
                    _,
                    _,
                >::new_with_empty(
                    self::execute,
                    self::instantiate,
                    self::query,
                ))
            }
        }

        impl<Chain: ::cw_orch::environment::CwEnv> $name <Chain> {
            pub fn new_test(chain: Chain) -> Self {
                Self(
                    ::cw_orch::contract::Contract::new($id, chain),
                )
            }
        }
    };
}
}
