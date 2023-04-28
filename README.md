# Cosmwasm two-to-one coin transfer

This is a cosmwasm contract that allows a user to
transfer coin to two recipients in a single transaction. The coin
is held in the contract, which the recipients can withdraw at any time. 
The owner of the contract can set a fee, which is charged by all transfer calls.

The sender must specify two recipients and a transfer amount. The
coin is split evenly between the recipients, after fees (if there is enough to do so).
The sender will be charged one coin less than what they specify if the transfer amount - fees
is an odd number.
Successful transfers will result in recipients having
a non-zero balance within the contract. Recipients
with non-zero balances can withdraw any amount
up to their balance. Any user can
query their balance on the contract. The contract has an owner, a fee, and a coin denomination, all of
which can be queried. When instantiating the contract
the owner, fee and coin denomination must be specified (eg. "sei").

## Running this contract

You will need Rust 1.44.1+ with `wasm32-unknown-unknown` target installed.

You can run unit tests on this via: 

`cargo test`

