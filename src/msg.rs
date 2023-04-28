use serde::{Deserialize, Serialize};
use cosmwasm_std::{Uint128};


#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct InstantiateMsg {
    pub owner: String,
    pub coin_denom : String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum QueryMsg {
    Owner {},
    Balance {address : String},
    
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct OwnerResp {
    pub owner: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct BalanceResp {
    pub balance: Uint128,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]

pub enum ExecuteMsg {

    Withdraw { amount : Uint128},

    Transfer {
        transfer_amount: Uint128,
        recipient_1: String,
        recipient_2: String,
    },
}
