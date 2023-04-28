#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    coins, to_binary, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128,
};

use crate::error::ContractError;
use crate::msg::{BalanceResp, ExecuteMsg, InstantiateMsg, OwnerResp, QueryMsg};
use crate::state::{BALANCE, COIN_DENOM, FEE, OWNER};

pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    OWNER.save(deps.storage, &deps.api.addr_validate(&msg.owner)?)?;
    COIN_DENOM.save(deps.storage, &msg.coin_denom)?;
    FEE.save(deps.storage, &msg.fee)?;

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
    
    let sender_funds = info.funds;
    let coin_denom: String = COIN_DENOM.load(deps.storage)?.to_string();
    let fee = Uint128::new(FEE.load(deps.storage)?.u128());

    if fee.gt(&transfer_amount){
        return Err(ContractError::SentLessThanFee {});
    }

    // The recipients get floor(trannsfer_amout - fee /2) sei. 
    // The owner gets the fee.
    // The remainder is not taken from the user.
    // Note that floor(trannsfer_amout - fee /2) must be greater than 1 (otherwise recipients cant get evenly paid).
    let transfer_amount_minus_fee = transfer_amount.checked_sub(fee).unwrap();
    let recipient_amt = transfer_amount_minus_fee
        .checked_div_floor((2u128, 1u128))
        .unwrap();


    // Make sure the sender actually has enough of the right coins to transfer
    if coin_denom != sender_funds[0].denom {
        return Err(ContractError::SentIncorrectCoin {});
    }

    if transfer_amount > sender_funds[0].amount {
        return Err(ContractError::NotEnoughCoin {});
    }

    if recipient_amt==Uint128::new(0){
        return Err(ContractError::RecipientPaidZeroOrOneCoin {});
    }

    

    // Get recipients
    let recipient_1 = deps.api.addr_validate(recipient_1.as_str())?;
    let recipient_2 = deps.api.addr_validate(recipient_2.as_str())?;

    // Update recipient_1s balance
    BALANCE.update(
        deps.storage,
        &recipient_1,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default() + recipient_amt)
        },
    )?;
    // Update recipient_2s balance
    BALANCE.update(
        deps.storage,
        &recipient_2,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default() + recipient_amt)
        },
    )?;
    // Update Owners balance
    let owner = OWNER.load(deps.storage)?;
    BALANCE.update(
        deps.storage,
        &owner,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + fee) },
    )?;

    // Make the bank transfer
    let coin_denom = COIN_DENOM.load(deps.storage)?;
    let message = BankMsg::Send {
        to_address: env.contract.address.to_string(),
        amount: coins(recipient_amt.u128() * (2 as u128), &coin_denom),
    };

    let sender_charged = fee.checked_add(recipient_amt.checked_mul(Uint128::new(2)).unwrap()).unwrap();

    Ok(Response::new().add_message(message).add_attributes(vec![
        ("action", "transfer"),
        ("recipient_1", recipient_1.as_str()),
        ("recipient_2", recipient_2.as_str()),
        ("owner", owner.as_str()),
        ("recipient_1_recieved", &recipient_amt.to_string()),
        ("recipient_2_recieved", &recipient_amt.to_string()),
        ("owner_recieved", &fee.to_string()),
        ("sender_charged", &sender_charged.to_string()),
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

    let transfer_check = balance.lt(&amount);
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
            fee: Uint128::new(1),
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn test_query_owner_address() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
            fee: Uint128::new(1),
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // query owner address
        let query_msg = QueryMsg::Owner {};
        let owner_resp: OwnerResp =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!("owner", owner_resp.owner);
    }

    #[test]
    fn test_query_owner_balance() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
            fee: Uint128::new(1),
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // The owner should have 0sei
        let owner = "owner".into();
        let query_msg = QueryMsg::Balance { address: owner };
        let balance_resp: BalanceResp =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!(Uint128::new(0), balance_resp.balance);
    }
    #[test]
    fn test_transfer_even_amount() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
            fee: Uint128::new(2),
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // send 100sei, owner gets 2, recipients get 49sei. None left for the sender
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
        assert_eq!(("recipient_1_recieved", "49"), exec_res.attributes[4]);
        assert_eq!(("owner_recieved", "2"), exec_res.attributes[6]);
        assert_eq!(("sender_charged", "100"), exec_res.attributes[7]);
    }


    #[test]
    fn test_transfer_odd_amount() {
        // Instantiate the contract
        let fee = Uint128::new(2);
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
            fee: fee,
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // send 99sei, owner gets 2, recipients get 48sei. 1sei left for the sender
        let info2 = mock_info(&String::from("some_user"), &balance);
        let recipient_1 = "recipient_1".into();
        let recipient_2 = "recipient_2".into();
        let exec_msg = ExecuteMsg::Transfer {
            transfer_amount: Uint128::new(99),
            recipient_1: recipient_1,
            recipient_2: recipient_2,
        };
        let exec_res = execute(deps.as_mut(), mock_env(), info2, exec_msg).unwrap();
        assert_eq!(("action", "transfer"), exec_res.attributes[0]);
        assert_eq!(("recipient_1", "recipient_1"), exec_res.attributes[1]);
        assert_eq!(("recipient_1_recieved", "48"), exec_res.attributes[4]);
        assert_eq!(("owner_recieved", fee.to_string()), exec_res.attributes[6]);
        assert_eq!(("sender_charged", "98"), exec_res.attributes[7]);
    }

    #[test]
    fn test_transfer_fee_plus_1_error() {
        // Instantiate the contract
        let fee = Uint128::new(2);
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
            fee: fee,
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // send 3sei, owner gets 2, recipients get 1sei and 0sei?. Should throw RecipientPaidZeroOrOneCoin error
        let info2 = mock_info(&String::from("some_user"), &balance);
        let recipient_1 = "recipient_1".into();
        let recipient_2 = "recipient_2".into();
        let exec_msg = ExecuteMsg::Transfer {
            transfer_amount: Uint128::new(3),
            recipient_1: recipient_1,
            recipient_2: recipient_2,
        };
        let exec_res = execute(deps.as_mut(), mock_env(), info2, exec_msg).unwrap_err();
        assert_eq!(ContractError::RecipientPaidZeroOrOneCoin {}, exec_res);

    }

    #[test]
    fn test_transfer_fee_error() {
        // Instantiate the contract
        let fee = Uint128::new(2);
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
            fee: fee,
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // send 2sei, owner gets 2, recipients get 0sei and 0sei?. Should throw RecipientPaidZeroOrOneCoin error
        let info2 = mock_info(&String::from("some_user"), &balance);
        let recipient_1 = "recipient_1".into();
        let recipient_2 = "recipient_2".into();
        let exec_msg = ExecuteMsg::Transfer {
            transfer_amount: Uint128::new(3),
            recipient_1: recipient_1,
            recipient_2: recipient_2,
        };
        let exec_res = execute(deps.as_mut(), mock_env(), info2, exec_msg).unwrap_err();
        assert_eq!(ContractError::RecipientPaidZeroOrOneCoin {}, exec_res);

    }

    #[test]
    fn test_query_zero_balance() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
            fee: Uint128::new(1),
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Users with no balance should be given a zero balance.
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
            fee: Uint128::new(1),
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // transfer 100sei with a 1sei fee.  Recipients should get 49sei, and owner should get 1. Overall, the sender is deducted 99sei.
        let info2 = mock_info(&String::from("some_user"), &balance);
        let recipient_1 = "recipient_1".into();
        let recipient_2 = "recipient_2".into();
        let exec_msg = ExecuteMsg::Transfer {
            transfer_amount: Uint128::new(100),
            recipient_1: recipient_1,
            recipient_2: recipient_2,
        };
        let exec_res: Response = execute(deps.as_mut(), mock_env(), info2, exec_msg).unwrap();
        assert_eq!(("action", "transfer"), exec_res.attributes[0]);
        assert_eq!(("recipient_1", "recipient_1"), exec_res.attributes[1]);
        assert_eq!(("recipient_1_recieved", "49"), exec_res.attributes[4]);
        assert_eq!(("owner_recieved", "1"), exec_res.attributes[6]);

        // Each recipient should now have 49sei
        let recipient_1 = "recipient_1".into();
        let query_msg = QueryMsg::Balance {
            address: recipient_1,
        };
        let balance_resp: BalanceResp =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!(Uint128::new(49), balance_resp.balance);
    }

    #[test]
    fn test_withdraw_nonzero_amount() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
            fee: Uint128::new(1),
        };
        let mut deps = mock_dependencies();
        let balance = coins(100, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // send 100sei, owner gets 1, recipients get 49sei. 1sei left for the sender
        let info2 = mock_info(&String::from("some_user"), &balance);
        let recipient_1 = "recipient_1".into();
        let recipient_2 = "recipient_2".into();
        let exec_msg = ExecuteMsg::Transfer {
            transfer_amount: Uint128::new(100),
            recipient_1: recipient_1,
            recipient_2: recipient_2,
        };
        let exec_res: Response = execute(deps.as_mut(), mock_env(), info2, exec_msg).unwrap();
        assert_eq!(("action", "transfer"), exec_res.attributes[0]);
        assert_eq!(("recipient_1", "recipient_1"), exec_res.attributes[1]);
        assert_eq!(("recipient_1_recieved", "49"), exec_res.attributes[4]);
        assert_eq!(("owner_recieved", "1"), exec_res.attributes[6]);

        // Each recipient should now have 49sei
        let recipient_1 = "recipient_1".into();
        let query_msg = QueryMsg::Balance {
            address: recipient_1,
        };
        let balance_resp: BalanceResp =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!(Uint128::new(49), balance_resp.balance);

        // The recpient should be able to withdraw the 49sei
        let info_recip = mock_info(&String::from("recipient_1"), &balance);
        let exec_msg = ExecuteMsg::Withdraw {
            amount: Uint128::new(49),
        };
        let exec_res: Response = execute(deps.as_mut(), mock_env(), info_recip, exec_msg).unwrap();
        assert_eq!(("action", "withdraw"), exec_res.attributes[0]);
        assert_eq!(("sender", "recipient_1"), exec_res.attributes[1]);
        assert_eq!(("withdraw_amount", "49"), exec_res.attributes[2]);

        // recipient_1 should now have 0sei
        let recipient_1 = "recipient_1".into();
        let query_msg = QueryMsg::Balance {
            address: recipient_1,
        };
        let balance_resp: BalanceResp =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!(Uint128::new(0), balance_resp.balance);

        // recipient_2 should still have 49sei
        let recipient_1 = "recipient_2".into();
        let query_msg = QueryMsg::Balance {
            address: recipient_1,
        };
        let balance_resp: BalanceResp =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!(Uint128::new(49), balance_resp.balance);
    }

    #[test]
    fn test_withdraw_not_enough_balance_error() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
            fee: Uint128::new(1),
        };
        let mut deps = mock_dependencies();
        let balance = coins(101, "sei");
        let info = mock_info(&String::from("some_user"), &balance);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // The recpient should not be able to withdraw more than their balance
        let info_recip = mock_info(&String::from("recipient_1"), &balance);
        let exec_msg = ExecuteMsg::Withdraw {
            amount: Uint128::new(100),
        };
        let exec_res = execute(deps.as_mut(), mock_env(), info_recip, exec_msg).unwrap_err();
        assert_eq!(ContractError::NotEnoughBalance {}, exec_res);
    }


    #[test]
    fn test_transfer_less_than_fee_error() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
            fee: Uint128::new(10000),
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
        assert_eq!(ContractError::SentLessThanFee {  }, exec_res);
    }



    #[test]
    fn test_transfer_not_enough_coin_error() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
            fee: Uint128::new(1),
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
    fn test_transfer_wrong_coin_denom() {
        // Instantiate the contract
        let instantiate_msg = InstantiateMsg {
            coin_denom: "sei".to_owned(),
            owner: "owner".to_owned(),
            fee: Uint128::new(1),
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
