#!/bin/sh
account_factory_code_id=1
ans_code_id=2
ibc_client_code_id=3
ibc_host_code_id=4
manager_code_id=5
module_factory_code_id=6
proxy_code_id=7
vc_code_id=8
bs721_profile_code_id=9
bs721_marketplace_code_id=10

admin_key=""
bidder_key=""
admin_addr=""
bidder_addr=""
binary=
chain_id=
gas_price=""
tx_flags="--from=$admin_key  --chain-id $chain_id --gas auto --gas-adjustment 2 --gas-prices=$gas_price -y -o json"
tx_flags_2="--from=$bidder_key --chain-id $chain_id --gas auto --gas-adjustment 2 --gas-prices=$gas_price -y -o json"

# fund second account 
echo 'fund second account'
fund=$($binary tx bank send $admin_addr $bidder_addr 1000000000ubtsg --gas auto -y -o json )
fund_hash=$(echo "$fund" | jq -r '.txhash');
echo 'waiting for tx to process'
sleep 6;
fund_tx=$($binary q tx $fund_hash -o json)

# ANS
echo 'Creating ANS'
ans_i=$($binary tx wasm i $ans_code_id '{"admin": "'$admin_addr'"}'  --label="ans_host" --admin $admin_addr  $tx_flags)
ans_hash=$(echo "$ans_i" | jq -r '.txhash');
echo 'waiting for tx to process'
sleep 6;
ans_tx=$($binary q tx $ans_hash -o json)
ans_addr=$(echo "$ans_tx" | jq -r '.logs[].events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value')
echo "ans_addr: $ans_addr"

# Version Control
echo 'Creating Version Control'
vc_i=$($binary tx wasm i $vc_code_id '{"admin": "'$admin_addr'", "security_disabled": false}'  --label="abstract_version_control" --admin $admin_addr  $tx_flags)
vc_hash=$(echo "$vc_i" | jq -r '.txhash')
echo 'waiting for tx to process'
sleep 6;
vc_tx=$($binary q tx $vc_hash -o json)
vc_addr=$(echo "$vc_tx" | jq -r '.logs[].events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value')
echo "vc_addr: $vc_addr"

# Module Factory
echo 'Creating Module Factory'
mf_i=$($binary tx wasm i $module_factory_code_id '{"admin": "'$admin_addr'","version_control_address":"'$vc_addr'","ans_host_address":"'$ans_addr'"}' $tx_flags --label="abstract_module_factory" --admin $admin_addr)
mf_hash=$(echo "$mf_i" | jq -r '.txhash')
echo 'waiting for tx to process'
sleep 6;
mf_tx=$($binary q tx $mf_hash -o json)
module_factory_addr=$(echo "$mf_tx" | jq -r '.logs[].events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value')
echo "module_factory_addr: $module_factory_addr"

# Account Factory
echo 'Creating Account Factory'
af_i=$($binary tx wasm i $account_factory_code_id '{"admin": "'$admin_addr'", "version_control_address":"'$vc_addr'","ans_host_address":"'$ans_addr'", "module_factory_address":"'$module_factory_addr'","min_profile_length": 3, "max_profile_length":128, "profile_bps": "10"}'  --label="abstract_account_factory" --admin $admin_addr  $tx_flags)
af_hash=$(echo "$af_i" | jq -r '.txhash')
echo 'waiting for tx to process'
sleep 6;
af_tx=$($binary q tx $af_hash -o json)
account_factory_addr=$(echo "$af_tx" | jq -r '.logs[].events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value')
echo "account_factory_addr: $account_factory_addr"

# IBC Client
echo 'Creating IBC Client'
ibc_client_i=$($binary tx wasm i $ibc_client_code_id '{"ans_host_address":"'$ans_addr'","version_control_address":"'$vc_addr'"}' --admin $admin_addr  $tx_flags --label="ibc_client")
ibc_client_hash=$(echo "$ibc_client_i" | jq -r '.txhash')
echo 'waiting for tx to process'
sleep 6;
ibc_c_tx=$($binary q tx $ibc_client_hash -o json)
ibc_client_addr=$(echo "$ibc_c_tx" | jq -r '.logs[].events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value')
echo "ibc_client_addr: $ibc_client_addr"

# IBC Host
echo 'Creating IBC Host'
ibc_host_i=$($binary tx wasm i $ibc_host_code_id '{"ans_host_address":"'$ans_addr'","account_factory_address":"'$account_factory_addr'","version_control_address":"'$vc_addr'"}' --admin $admin_addr  $tx_flags --label="ibc_host")
ibc_host_hash=$(echo "$ibc_host_i" | jq -r '.txhash')
echo 'waiting for tx to process'
sleep 6;
ibc_h_tx=$($binary q tx $ibc_host_hash -o json)
ibc_host_addr=$(echo "$ibc_h_tx" | jq -r '.logs[].events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value')
echo "ibc_host_addr: $ibc_host_addr"

# Update VC config
echo 'Updating VC Config'
$binary tx wasm e $vc_addr '{"update_config":{"account_factory_address":"'$account_factory_addr'"}}' $tx_flags
echo 'waiting for tx to process'
sleep 6;

# Propose Modules to VC
echo 'Proposing Modules to VC'
MSG=$(cat <<EOF
{
    "propose_modules": {"modules": [
        [{"name": "manager","namespace":"abstract","version": {"version": "0.22.1"}},{"account_base": $manager_code_id}],
        [{"name": "proxy","namespace": "abstract","version": {"version": "0.22.1"}},{"account_base": $proxy_code_id}],
        [{"name": "ans-host","namespace": "abstract","version": {"version": "0.22.1"}},{"native": "$ans_addr"}],
        [{"name": "version-control","namespace": "abstract","version": {"version": "0.22.1"}},{"native": "$vc_addr"}],
        [{"name": "account-factory","namespace": "abstract","version": {"version": "0.22.1"}},{"native": "$account_factory_addr"}],
        [{"name": "module-factory","namespace": "abstract", "version": {"version": "0.22.1"}},{"native": "$module_factory_addr"}],
        [{"name": "ibc-client","namespace": "abstract","version": {"version": "0.22.1"}},{"native": "$ibc_client_addr"}],
        [{"name": "ibc-host","namespace": "abstract","version": {"version": "0.22.1"}},{"native": "$ibc_host_addr"}]
    ]}
}
EOF
)
echo $MSG
$binary tx wasm e $vc_addr "$MSG" $tx_flags
echo 'waiting for tx to process'
sleep 6;
# Approve Modules to VC
echo 'Approve Modules to VC'
MSG2=$(cat <<EOF
{"approve_or_reject_modules": {
    "approves": [
        {"name": "manager","namespace": "abstract","version": {"version": "0.22.1"}},
        {"name": "proxy","namespace": "abstract","version": {"version": "0.22.1"}},
        {"name": "ans-host","namespace": "abstract","version": {"version": "0.22.1"}},
        {"name": "version-control","namespace": "abstract","version": {"version": "0.22.1"}},
        {"name": "account-factory","namespace": "abstract","version": {"version": "0.22.1"}},
        {"name": "module-factory","namespace": "abstract","version": {"version": "0.22.1"}},
        {"name": "ibc-client","namespace": "abstract","version": {"version": "0.22.1"}},
        {"name": "ibc-host","namespace": "abstract","version": {"version": "0.22.1"}}
    ],
    "rejects": []
    }
}
EOF
)
$binary tx wasm e $vc_addr "$MSG2" $tx_flags
echo 'waiting for tx to process'
sleep 6;

# Update Account Factory Config 
echo 'Update Account Factory'
$binary tx wasm e $account_factory_addr '{"update_config":{"ibc_host":"'$ibc_host_addr'"}}' $tx_flags
echo 'waiting for tx to process'
sleep 6;

# Setup Profile Infra on Account Factory
echo 'Setup Profile Infra on Account Factory'
infra_tx=$($binary tx wasm e $account_factory_addr '{"setup_profile_infra":{"marketplace_code_id":'$bs721_marketplace_code_id',"profile_code_id":'$bs721_profile_code_id'}}' $tx_flags) 
echo 'waiting for tx to process'
sleep 6;

infra_tx_hash=$(echo "$infra_tx" | jq -r '.txhash')
echo $infra_tx_hash
infra_query=$($binary q tx $infra_tx_hash -o json)
bs721_profile_addr=$(echo "$infra_query" | jq -r '.logs[].events[] | select(.type == "wasm") | .attributes[] | select(.key == "bs721_profile_address") | .value')
bs721_marketplace_addr=$(echo "$infra_query" | jq -r '.logs[].events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value')
echo 'bs721_profile_addr: '$bs721_profile_addr''
echo 'bs721_marketplace_addr: '$bs721_marketplace_addr''

## Create Account 
echo 'Create Account 1'
admin_tx=$($binary tx wasm e $account_factory_addr '{"create_account": {"governance":{"Monarchy":{"monarch":"'$admin_addr'"}},"name":"first-os","install_modules":[],"bs_profile":"the-monk-on-iron-mountain"}}' $tx_flags --amount 10000000ubtsg )
echo 'waiting for tx to process'
sleep 6;
admin_account_tx=$(echo "$admin_tx" | jq -r '.txhash')
echo $admin_account_tx
admin_query=$($binary q tx $admin_account_tx -o json)
admin_manager_addr=$(echo "$admin_query" | jq -r '.logs[].events[] | select(.type == "wasm-abstract") | .attributes[] | select(.key == "manager_address") | .value')
admin_proxy_addr=$(echo "$admin_query" | jq -r '.logs[].events[] | select(.type == "wasm-abstract") | .attributes[] | select(.key == "proxy_address") | .value')
echo 'admin_manager_addr: '$admin_manager_addr''
echo 'admin_proxy_addr: '$admin_proxy_addr''

# echo 'Creating Account 2'
# bidder_tx=$($binary tx wasm e $account_factory_addr '{"create_account": {"governance":{"Monarchy":{"monarch":"'$bidder_addr'"}},"name":"second-os","install_modules":[],"bs_profile":"the-monk-on-iron-mountain2"}}' $tx_flags_2 --amount 10000000ubtsg )
# echo 'waiting for tx to process'
# sleep 6;
# bidder_account_tx=$(echo "$bidder_tx" | jq -r '.txhash')
# bidder_query=$($binary q tx $bidder_account_tx -o json)
# bidder_manager_addr=$(echo "$bidder_query" | jq -r '.logs[].events[] | select(.type == "wasm-abstract") | .attributes[] | select(.key == "manager_address") | .value')
# bidder_proxy_addr=$(echo "$bidder_query" | jq -r '.logs[].events[] | select(.type == "wasm-abstract") | .attributes[] | select(.key == "proxy_address") | .value')
# echo 'bidder_manager_addr: '$bidder_manager_addr''
# echo 'bidder_proxy_addr: '$bidder_proxy_addr''

# Query Profile Collection 
echo 'Query Profile Collection'
$binary q wasm contract-state smart $bs721_profile_addr '{"all_tokens":{}}'
$binary q wasm contract-state smart $bs721_profile_addr '{"all_tokens":{}}'


# Bid Marketplace
# echo 'bid on marketplace'
# $binary tx wasm e $bs721_marketplace_addr  '{"set_bid":{"token_id":"the-monk-on-iron-mountain"}}' $tx_flags_2 --amount 101ubtsg
# echo 'waiting for tx to process'
# sleep 6;

## Query Marketplace 
# echo 'query ask'
# $binary q wasm contract-state smart $bs721_marketplace_addr '{"ask":{"token_id":"the-monk-on-iron-mountain"}}'
# echo 'query bid'
# $binary q wasm contract-state smart $bs721_marketplace_addr '{"bid":{"token_id":"the-monk-on-iron-mountain"}}'

# Accept Bid 
# echo 'accept bid'
# $binary tx wasm e $bs721_marketplace_addr  '{"accept_bid":{"token_id":"the-monk-on-iron-mountain", "bidder":"'$bidder_addr'"}}' $tx_flags
# echo 'waiting for tx to process'
# sleep 6;

# Query Marketplace Again
# echo 'assert new owner'
# $binary q wasm contract-state smart $bs721_profile_addr '{"name":{"address":"'$bidder_addr'"}}'
# $binary q wasm contract-state smart $bs721_marketplace_addr '{"ask":{"token_id":"the-monk-on-iron-mountain"}}'

# Call Functions Through Smart Contract Account
echo 'use account contracts to call msg burning tokens'
$binary q bank balances $admin_proxy_addr
burn_msg_binary='{"module_action":{"msgs":[{"bank":{"burn":{"amount":[{"amount":"100","denom":"ubtsg"}]}}}]}}' 
burn_binary=$(echo $burn_msg_binary | jq -c . | base64)

burn_msg_tx=$($binary tx wasm e $admin_manager_addr '{"exec_on_module":{"module_id":"abstract:proxy","exec_msg":"'$burn_binary'"}}' $tx_flags)
burn_tx=$(echo "$burn_msg_tx" | jq -r '.txhash')

echo $burn_tx
# Query Proxy balance
sleep 6;
$binary q bank balances $admin_proxy_addr