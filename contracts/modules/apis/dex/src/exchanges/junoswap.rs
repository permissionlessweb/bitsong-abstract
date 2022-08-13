use crate::{
    contract::{DexApi, DexResult},
    error::DexError,
    DEX,
};
use abstract_sdk::OsExecute;
use cosmwasm_std::{
    to_binary, Addr, Coin, CosmosMsg, Decimal, Deps, Fraction, QueryRequest, StdResult, Uint128,
    WasmMsg, WasmQuery,
};
use cw20_junoswap::Denom;
use cw_asset::{Asset, AssetInfo};
use wasmswap::msg::*;
pub const JUNOSWAP: &str = "junoswap";

pub struct JunoSwap {}

impl DEX for JunoSwap {
    fn name(&self) -> &'static str {
        JUNOSWAP
    }
    fn swap(
        &self,
        deps: Deps,
        api: DexApi,
        contract_address: Addr,
        offer_asset: Asset,
        ask_asset: AssetInfo,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
    ) -> DexResult {
        let pair_config: InfoResponse =
            deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: contract_address.to_string(),
                msg: to_binary(&QueryMsg::Info {})?,
            }))?;

        let (offer_token, price) =
            if denom_and_asset_match(&pair_config.token1_denom, &offer_asset.info)? {
                (
                    TokenSelect::Token1,
                    Decimal::from_ratio(pair_config.token2_reserve, pair_config.token1_reserve),
                )
            } else if denom_and_asset_match(&pair_config.token1_denom, &ask_asset)? {
                (
                    TokenSelect::Token2,
                    Decimal::from_ratio(pair_config.token1_reserve, pair_config.token2_reserve),
                )
            } else {
                return Err(DexError::DexMismatch(
                    format!("{}/{}", &offer_asset.info, &ask_asset),
                    self.name().into(),
                    contract_address.to_string(),
                ));
            };

        let min_out: Uint128 = match max_spread {
            None => 0u128.into(),
            Some(spread) => {
                let price_to_use = belief_price.unwrap_or(price);
                let ideal_return = offer_asset.amount * price_to_use;
                ideal_return * (Decimal::one() - spread)
            }
        };

        let msg = ExecuteMsg::Swap {
            input_token: offer_token,
            input_amount: offer_asset.amount,
            min_output: min_out,
            expiration: None,
        };

        let asset_msg = offer_asset.send_msg(contract_address, to_binary(&msg)?)?;
        api.os_execute(deps, vec![asset_msg]).map_err(From::from)
    }

    fn provide_liquidity(
        &self,
        deps: Deps,
        api: DexApi,
        contract_address: Addr,
        offer_assets: Vec<Asset>,
        max_spread: Option<Decimal>,
    ) -> DexResult {
        if offer_assets.len() > 2 {
            return Err(DexError::TooManyAssets(2));
        }
        let pair_config: InfoResponse =
            deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: contract_address.to_string(),
                msg: to_binary(&QueryMsg::Info {})?,
            }))?;
        let (token1, token2) =
            if denom_and_asset_match(&pair_config.token1_denom, &offer_assets[0].info)? {
                (&offer_assets[0], &offer_assets[1])
            } else if denom_and_asset_match(&pair_config.token1_denom, &offer_assets[1].info)? {
                (&offer_assets[1], &offer_assets[0])
            } else {
                return Err(DexError::DexMismatch(
                    format!("{}/{}", offer_assets[0].info, offer_assets[1].info),
                    self.name().into(),
                    contract_address.to_string(),
                ));
            };

        let my_ratio = Decimal::from_ratio(token1.amount, token2.amount);
        let max_token2 = if let Some(max_spread) = max_spread {
            token1.amount * my_ratio.inv().unwrap() * (max_spread + Decimal::one())
        } else {
            Uint128::MAX
        };

        let msg = ExecuteMsg::AddLiquidity {
            token1_amount: token1.amount,
            min_liquidity: Uint128::zero(),
            max_token2,
            expiration: None,
        };
        let mut msgs = cw_approve_msgs(&offer_assets, &api.request_destination)?;
        let coins = coins_in_assets(&offer_assets);
        let junoswap_msg = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_address.into_string(),
            msg: to_binary(&msg)?,
            funds: coins,
        });
        msgs.push(junoswap_msg);
        api.os_execute(deps, msgs).map_err(From::from)
    }

    fn provide_liquidity_symmetric(
        &self,
        deps: Deps,
        api: DexApi,
        contract_address: Addr,
        offer_asset: Asset,
        other_assets: Vec<AssetInfo>,
    ) -> DexResult {
        if other_assets.len() > 1 {
            return Err(DexError::TooManyAssets(2));
        }
        // Get pair info
        let pair_config: InfoResponse =
            deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: contract_address.to_string(),
                msg: to_binary(&QueryMsg::Info {})?,
            }))?;
        // because of the token1 / token2 thing we need to figure out what the offer asset is and calculate the required amount of the other asset.
        let (token_1_amount, token_2_amount, other_asset) =
            if denom_and_asset_match(&pair_config.token1_denom, &offer_asset.info)? {
                let price =
                    Decimal::from_ratio(pair_config.token2_reserve, pair_config.token1_reserve);
                // token2 = token1 * (token2/token1)
                let token_2_amount = offer_asset.amount * price;
                let other_asset = Asset {
                    info: other_assets[0].clone(),
                    amount: token_2_amount,
                };
                (offer_asset.amount, token_2_amount, other_asset)
            } else if denom_and_asset_match(&pair_config.token2_denom, &offer_asset.info)? {
                let price =
                    Decimal::from_ratio(pair_config.token1_reserve, pair_config.token2_reserve);
                // token1 = token2 * (token1/token2)
                let token_1_amount = offer_asset.amount * price;
                let other_asset = Asset {
                    info: other_assets[0].clone(),
                    amount: token_1_amount,
                };
                (token_1_amount, offer_asset.amount, other_asset)
            } else {
                return Err(DexError::DexMismatch(
                    format!("{}/{}", offer_asset.info, other_assets[0]),
                    self.name().into(),
                    contract_address.to_string(),
                ));
            };

        let msg = ExecuteMsg::AddLiquidity {
            token1_amount: token_1_amount,
            min_liquidity: Uint128::zero(),
            max_token2: token_2_amount,
            expiration: None,
        };
        let assets = &[offer_asset, other_asset];
        let mut msgs = cw_approve_msgs(assets, &api.request_destination)?;
        let coins = coins_in_assets(assets);
        let junoswap_msg = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_address.into_string(),
            msg: to_binary(&msg)?,
            funds: coins,
        });
        msgs.push(junoswap_msg);
        api.os_execute(deps, msgs).map_err(From::from)
    }

    fn withdraw_liquidity(
        &self,
        deps: Deps,
        api: &DexApi,
        contract_address: Addr,
        lp_token: Asset,
    ) -> DexResult {
        let junoswap_msg = ExecuteMsg::RemoveLiquidity {
            amount: lp_token.amount,
            min_token1: Uint128::zero(),
            min_token2: Uint128::zero(),
            expiration: None,
        };
        let msg = lp_token.send_msg(contract_address, to_binary(&junoswap_msg)?)?;
        api.os_execute(deps, vec![msg]).map_err(From::from)
    }
}

fn denom_and_asset_match(denom: &Denom, asset: &AssetInfo) -> Result<bool, DexError> {
    match denom {
        Denom::Native(denom_name) => match asset {
            cw_asset::AssetInfoBase::Native(asset_name) => Ok(denom_name == asset_name),
            cw_asset::AssetInfoBase::Cw20(_asset_addr) => Ok(false),
            cw_asset::AssetInfoBase::Cw1155(_, _) => Err(DexError::Cw1155Unsupported),
        },
        Denom::Cw20(denom_addr) => match asset {
            cw_asset::AssetInfoBase::Native(_asset_name) => Ok(false),
            cw_asset::AssetInfoBase::Cw20(asset_addr) => Ok(denom_addr == asset_addr),
            cw_asset::AssetInfoBase::Cw1155(_, _) => Err(DexError::Cw1155Unsupported),
        },
    }
}

fn cw_approve_msgs(assets: &[Asset], spender: &Addr) -> StdResult<Vec<CosmosMsg>> {
    let mut msgs = vec![];
    for asset in assets {
        if let AssetInfo::Cw20(addr) = &asset.info {
            let msg = cw20_junoswap::Cw20ExecuteMsg::IncreaseAllowance {
                spender: spender.to_string(),
                amount: asset.amount,
                expires: None,
            };
            msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                msg: to_binary(&msg)?,
                funds: vec![],
            }))
        }
    }
    Ok(msgs)
}

fn coins_in_assets(assets: &[Asset]) -> Vec<Coin> {
    let mut coins = vec![];
    for asset in assets {
        if let AssetInfo::Native(denom) = &asset.info {
            coins.push(Coin::new(asset.amount.u128(), denom.clone()));
        }
    }
    coins
}
