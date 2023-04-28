#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    coins, to_binary, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128,
};

use crate::error::ContractError;
use crate::msg::{BalanceResp, ExecuteMsg, InstantiateMsg, OwnerResp, QueryMsg};
use crate::state::{BALANCE, COIN_DENOM, OWNER};

pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    OWNER.save(deps.storage, &deps.api.addr_validate(&msg.owner)?)?;
    COIN_DENOM.save(deps.storage, &msg.coin_denom)?;
    Ok(Response::new())
}

pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Owner {} => to_binary(&query_owner(deps)?),
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
    }
}

pub fn query_owner(deps: Deps) -> StdResult<OwnerResp> {
    let owner = OWNER.load(deps.storage)?.to_string();
    Ok(OwnerResp { owner })
}

pub fn query_balance(deps: Deps, address: String) -> StdResult<BalanceResp> {
    let address = deps.api.addr_validate(&address)?;
    let balance = BALANCE
        .may_load(deps.storage, &address)?
        .unwrap_or_default();
    Ok(BalanceResp { balance })
}

pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Transfer {
            transfer_amount,
            recipient_1,
            recipient_2,
        } => execute_transfer(deps, env, info, transfer_amount, recipient_1, recipient_2),
        ExecuteMsg::Withdraw { amount } => execute_withdraw(deps, env, info, amount),
    }
}

pub fn execute_transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    transfer_amount: Uint128,
    recipient_1: String,
    recipient_2: String,
) -> Result<Response, ContractError> {
    // Make sure the sender actually has enough of the right coins to transfer
    let sender_funds = info.funds;
    let coin_denom: String = COIN_DENOM.load(deps.storage)?.to_string();
    if coin_denom != sender_funds[0].denom {
        return Err(ContractError::SentIncorrectCoin {});
    }
    if sender_funds[0].amount.is_zero() {
        return Err(ContractError::SenderHasZeroCoin {});
    }

    // Don't accept zero coin transfers
    if transfer_amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    // Sender cant send more coin than they own
    if transfer_amount > sender_funds[0].amount {
        return Err(ContractError::NotEnoughCoin {});
    }

    // Get recipients
    let recipient_1 = deps.api.addr_validate(recipient_1.as_str())?;
    let recipient_2 = deps.api.addr_validate(recipient_2.as_str())?;

    // The recipients get floor(trannsfer_amout/2) sei. The remainder is not taken from the user.
    let split_amt = transfer_amount.checked_div_floor((2u128, 1u128)).unwrap();

    // Update recipient_1s balance
    BALANCE.update(
        deps.storage,
        &recipient_1,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + split_amt) },
    )?;
    // Update recipient_2s balance
    BALANCE.update(
        deps.storage,
        &recipient_2,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + split_amt) },
    )?;

    // Make the bank transfer
    let coin_denom = COIN_DENOM.load(deps.storage)?;
    let message = BankMsg::Send {
        to_address: env.contract.address.to_string(),
        amount: coins(split_amt.u128() * (2 as u128), &coin_denom),
    };

    Ok(Response::new().add_message(message).add_attributes(vec![
        ("action", "transfer"),
        ("recipient_1", recipient_1.as_str()),
        ("recipient_2", recipient_2.as_str()),
        ("recipient_1_recieved", &split_amt.to_string()),
        ("recipient_2_recieved", &split_amt.to_string()),
    ]))
}

pub fn execute_withdraw(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // Check that the sender has enough to withdraw
    let balance = BALANCE
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();

    let transfer_check = balance.le(&amount);
    if transfer_check {
        return Err(ContractError::NotEnoughBalance {});
    }

    // Update the senders balance
    BALANCE.update(
        deps.storage,
        &info.sender,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount).unwrap())
        },
    )?;

    // Make the bank transfer
    let coin_denom = COIN_DENOM.load(deps.storage)?;
    let message = BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(amount.u128(), &coin_denom),
    };

    Ok(Response::new().add_message(message).add_attributes(vec![
        ("action", "withdraw"),
        ("sender", info.sender.as_str()),
        ("withdraw_amount", &amount.to_string()),
    ]))
}

#[cfg(test)]
mod tests {

    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    use super::*;

    #[test]
    fn test_instantiate() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn test_query_owner() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // query owner
        let query_msg = QueryMsg::Owner {};
        let owner_resp: OwnerResp =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!("owner", owner_resp.owner);
    }

    #[test]
    fn test_transfer_even_amount() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // execute transfer where the sender sends an even amount of coin
        // eg if a sender sends 100 sei, then the recipients get 50sei each
        let info2 = mock_info(&String::from("some_user"), &balance);
        let recipient_1 = "recipient_1".into();
        let recipient_2 = "recipient_2".into();
        let exec_msg = ExecuteMsg::Transfer {
            transfer_amount: Uint128::new(100),
            recipient_1: recipient_1,
            recipient_2: recipient_2,
        };
        let exec_res = execute(deps.as_mut(), mock_env(), info2, exec_msg).unwrap();
        assert_eq!(("action", "transfer"), exec_res.attributes[0]);
        assert_eq!(("recipient_1", "recipient_1"), exec_res.attributes[1]);
        assert_eq!(("recipient_1_recieved", "50"), exec_res.attributes[3]);
    }

    #[test]
    fn test_transfer_odd_amount() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // execute transfer where the sender sends an odd amount of coin
        // eg if a sender sends 3 sei, then he/she is actually charged 2sei, and that 2sei is split between the recipients
        let info2 = mock_info(&String::from("some_user"), &balance);
        let recipient_1 = "recipient_1".into();
        let recipient_2 = "recipient_2".into();
        let exec_msg = ExecuteMsg::Transfer {
            transfer_amount: Uint128::new(3),
            recipient_1: recipient_1,
            recipient_2: recipient_2,
        };
        let exec_res = execute(deps.as_mut(), mock_env(), info2, exec_msg).unwrap();
        assert_eq!(("action", "transfer"), exec_res.attributes[0]);
        assert_eq!(("recipient_1", "recipient_1"), exec_res.attributes[1]);
        assert_eq!(("recipient_1_recieved", "1"), exec_res.attributes[3]);
    }

    #[test]
    fn test_query_zero_balance() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Each recipient should now have 50sei
        let recipient_1 = "no_bal_user".into();
        let query_msg = QueryMsg::Balance {
            address: recipient_1,
        };
        let balance_resp: BalanceResp =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!(Uint128::new(0), balance_resp.balance);
    }

    #[test]
    fn test_query_nonzero_balance() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // execute transfer where the sender sends an even amount of coin
        // eg if a sender sends 100 sei, then the recipients get 50sei each
        let info2 = mock_info(&String::from("some_user"), &balance);
        let recipient_1 = "recipient_1".into();
        let recipient_2 = "recipient_2".into();
        let exec_msg = ExecuteMsg::Transfer {
            transfer_amount: Uint128::new(100),
            recipient_1: recipient_1,
            recipient_2: recipient_2,
        };
        let exec_res = execute(deps.as_mut(), mock_env(), info2, exec_msg).unwrap();
        assert_eq!(("action", "transfer"), exec_res.attributes[0]);
        assert_eq!(("recipient_1", "recipient_1"), exec_res.attributes[1]);
        assert_eq!(("recipient_1_recieved", "50"), exec_res.attributes[3]);

        // Each recipient should now have 50sei
        let recipient_1 = "recipient_1".into();
        let query_msg = QueryMsg::Balance {
            address: recipient_1,
        };
        let balance_resp: BalanceResp =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!(Uint128::new(50), balance_resp.balance);
    }

    #[test]
    fn test_withdraw_nonzero_amount() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // execute transfer where the sender sends an even amount of coin
        // eg if a sender sends 100 sei, then the recipients get 50sei each
        let info2 = mock_info(&String::from("some_user"), &balance);
        let recipient_1 = "recipient_1".into();
        let recipient_2 = "recipient_2".into();
        let exec_msg = ExecuteMsg::Transfer {
            transfer_amount: Uint128::new(100),
            recipient_1: recipient_1,
            recipient_2: recipient_2,
        };
        let exec_res = execute(deps.as_mut(), mock_env(), info2, exec_msg).unwrap();
        assert_eq!(("action", "transfer"), exec_res.attributes[0]);
        assert_eq!(("recipient_1", "recipient_1"), exec_res.attributes[1]);
        assert_eq!(("recipient_1_recieved", "50"), exec_res.attributes[3]);

        // Each recipient should now have 50sei
        let recipient_1 = "recipient_1".into();
        let query_msg = QueryMsg::Balance {
            address: recipient_1,
        };
        let balance_resp: BalanceResp =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!(Uint128::new(50), balance_resp.balance);

        // The recpient should be able to withdraw the coin
        let info_recip = mock_info(&String::from("recipient_1"), &balance);
        let exec_msg = ExecuteMsg::Withdraw {
            amount: Uint128::new(49),
        };
        let exec_res = execute(deps.as_mut(), mock_env(), info_recip, exec_msg).unwrap();
        assert_eq!(("action", "withdraw"), exec_res.attributes[0]);
        assert_eq!(("sender", "recipient_1"), exec_res.attributes[1]);
        assert_eq!(("withdraw_amount", "49"), exec_res.attributes[2]);

        // recipient_1 should now have 1sei
        let recipient_1 = "recipient_1".into();
        let query_msg = QueryMsg::Balance {
            address: recipient_1,
        };
        let balance_resp: BalanceResp =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!(Uint128::new(1), balance_resp.balance);

        // recipient_2 should still have 50sei
        let recipient_1 = "recipient_2".into();
        let query_msg = QueryMsg::Balance {
            address: recipient_1,
        };
        let balance_resp: BalanceResp =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!(Uint128::new(50), balance_resp.balance);
    }

    #[test]
    fn test_withdraw_not_enough_balance_error() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // execute transfer where the sender sends an even amount of coin
        // eg if a sender sends 100 sei, then the recipients get 50sei each
        let info2 = mock_info(&String::from("some_user"), &balance);
        let recipient_1 = "recipient_1".into();
        let recipient_2 = "recipient_2".into();
        let exec_msg = ExecuteMsg::Transfer {
            transfer_amount: Uint128::new(100),
            recipient_1: recipient_1,
            recipient_2: recipient_2,
        };
        let exec_res = execute(deps.as_mut(), mock_env(), info2, exec_msg).unwrap();
        assert_eq!(("action", "transfer"), exec_res.attributes[0]);
        assert_eq!(("recipient_1", "recipient_1"), exec_res.attributes[1]);
        assert_eq!(("recipient_1_recieved", "50"), exec_res.attributes[3]);

        // The recpient should not be able to withdraw more than their balance
        let info_recip = mock_info(&String::from("recipient_1"), &balance);
        let exec_msg = ExecuteMsg::Withdraw {
            amount: Uint128::new(100),
        };
        let exec_res = execute(deps.as_mut(), mock_env(), info_recip, exec_msg).unwrap_err();
        assert_eq!(ContractError::NotEnoughBalance {}, exec_res);
    }

    #[test]
    fn test_transfer_not_enough_coin_error() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
        };
        let mut deps = mock_dependencies();
        let balance = coins(10, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // sender tries to send more than he/she has
        let info2 = mock_info(&String::from("some_user"), &balance);
        let recipient_1 = "recipient_1".into();
        let recipient_2 = "recipient_2".into();
        let exec_msg = ExecuteMsg::Transfer {
            transfer_amount: Uint128::new(100),
            recipient_1: recipient_1,
            recipient_2: recipient_2,
        };
        let exec_res = execute(deps.as_mut(), mock_env(), info2, exec_msg).unwrap_err();
        assert_eq!(ContractError::NotEnoughCoin {}, exec_res);
    }

    #[test]
    fn test_transfer_zero_enough_coin_error() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
        };
        let mut deps = mock_dependencies();
        let balance = coins(0, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // sender tries to send more than he/she has
        let info2 = mock_info(&String::from("some_user"), &balance);
        let recipient_1 = "recipient_1".into();
        let recipient_2 = "recipient_2".into();
        let exec_msg = ExecuteMsg::Transfer {
            transfer_amount: Uint128::new(100),
            recipient_1: recipient_1,
            recipient_2: recipient_2,
        };
        let exec_res = execute(deps.as_mut(), mock_env(), info2, exec_msg).unwrap_err();
        assert_eq!(ContractError::SenderHasZeroCoin {}, exec_res);
    }

    #[test]
    fn test_trasnfer_wrong_coin_denom() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
        };
        let mut deps = mock_dependencies();
        let balance = coins(0, "not_sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // sender tries to send more than he/she has
        let info2 = mock_info(&String::from("some_user"), &balance);
        let recipient_1 = "recipient_1".into();
        let recipient_2 = "recipient_2".into();
        let exec_msg = ExecuteMsg::Transfer {
            transfer_amount: Uint128::new(100),
            recipient_1: recipient_1,
            recipient_2: recipient_2,
        };
        let exec_res = execute(deps.as_mut(), mock_env(), info2, exec_msg).unwrap_err();
        assert_eq!(ContractError::SentIncorrectCoin {}, exec_res);
    }
}
