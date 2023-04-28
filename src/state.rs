use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};


pub const OWNER: Item<Addr> = Item::new("owner");
pub const COIN_DENOM: Item<String> = Item::new("coin_denom");
pub const FEE: Item<Uint128> = Item::new("fee");
pub const BALANCE: Map<&Addr, Uint128> = Map::new("balance");
