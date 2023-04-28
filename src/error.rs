use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Not enough balance to withdraw input amount")]
    NotEnoughBalance {},

    #[error("Sender has zero coin")]
    SenderHasZeroCoin {},

    #[error("Sender does not have enough coin to make transfer")]
    NotEnoughCoin {},

    #[error("Sender must send more than zero coin")]
    InvalidZeroAmount {},

    #[error("Sender sent an incorrect coin.")]
    SentIncorrectCoin {},


}
