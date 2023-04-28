use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Not enough balance to withdraw input amount")]
    NotEnoughBalance {},

    #[error("Sender sent less than the fee amount")]
    SentLessThanFee {},

    #[error("Sender does not have enough coin to make transferand pay fee")]
    NotEnoughCoin {},

    #[error("Sender sent an incorrect coin.")]
    SentIncorrectCoin {},
    
    #[error("Only enough coins to pay recipients either no coins, or an uneven amount of coins (ie transfer_amount = fee + 1")]
    RecipientPaidZeroOrOneCoin {},
    
}
