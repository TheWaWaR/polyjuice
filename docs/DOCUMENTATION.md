# Basic Concepts
## Account
There are two kind of account, contract account and EoA account.
### Contract Account
The contract account in polyjuice is a cell constrainted by polyjuice type script. The type script args is a [`type_id`][1] value, so that the type script is unique. First 32 bytes of the cell data is the `storage root` (sparse-merkle-tree) of the contract. The second 32 bytes of the cell data is the `code_hash` (`blake2b(code)`) of the contract. Since we want everyone use the contract, we default use an always lock script. We also can use any lock script for access control or other purpose.

### EoA Account
The EoA account in polyjuice is all live cells locked by default secp256k1 sighash lock script. The id of the account is the lock script args.

## Contract
A contract in polyjuice is mostly the same as an ethereum contract. You can write your contract in solidity or vyper or assembly then comiple to EVM byte code. There are some minor differences. Since it's **impossible** to read block information from current block, we instead read block information from most recently block. The most recently means the latest block of blocks include the transaction inputs:

```
max(block_number(inputs))
```

It will effect following op codes:

* `BLOCKHASH` 
* `COINBASE` 
* `TIMESTAMP`
* `NUMBER`
* `DIFFICULTY`
* `GASLIMIT`

The `DIFFICULTY` value is the difficulty of [Nervos CKB chain][2]. The `GASLIMIT` here is a constant value which is equals max value of `int64_t`(9223372036854775807). The cost of the transaction is determined by its size and [cycles][4], so gas limit is meaningless in polyjuice. 

The `COINBASE` return value and `SELFDESTRUCT` target is the first 20 bytes of lock script hash, which is:

```
blake2b(lock_script)[0..20]
```

## Program
A program is a `CREATE` or `CALL` with its parameters. Since polyjuice support contract call contract, a polyjuice transaction can contains multiple programs. Programs are serialized into witness.

## Generator
Polyjuice generator can generate a polyjuice transaction through follow JSONRPC API:
``` rust
fn create(sender: H160, code: Bytes) -> TransactionReceipt;
fn call(sender: H160, contract_address: H160, input: Bytes) -> TransactionReceipt;
```

## Validator
Polyjuice validator is the type script verify the transformation of contract cells.

## Indexer
Indexer is a polyjuice module for indexing every polyjuice transaction in CKB block. The contract metadata, changes and all the logs emitted from the polyjuice transaction will be saved. Also all live cells will be indexed for running generator (build polyjuice transaction).

# Design Details
## How to organize cells in a CKB transaction?
![Transaction Structure][8]
[Transaction Structure.pdf][5]

## The CKB transaction generation process
![How Generator Works][9]
[How Generator Works.pdf][6]

## The CKB transaction validation process
![How Validator Works][10]
[How Validator Works.pdf][7]

## Communicate through ckb-vm syscalls
In `generator` and `indexer`, we use syscalls to handle the event emitted from program execution process. Below are the syscalls we currently used:

- `2177` is for `ckb_debug`, useful when you want debug the `generator`
- `3075` is for returning the EVM result
- `3076` is for logging
- `3077` is for saving `SELFDESTRUCT` beneficiary address
- `3078` is for handle `CALL` and `CREATE` op codes
- `3079` is for returning code size of a contract to EVM
- `3080` is for returning a code slice of a contract to EVM


# Implementation Details
## How to handle contract creation?
A `CREATE` from a `sender` or `contract` will lead to a contract creation. In `generator`, polyjuice will be assigned a `type_id` type script and the contract code hash will be saved in data field next to account storage hash. In `validator`, type script will check the contract code hash match the `code_hash` in data field.

## How to handle contract destruction?
A contract destrunction only happen when a `SELFDESTRUCT` op code is invoked. In `generator`, the destructed contract is consumed as input, and put an output cell as the beneficiary cell and the lock script must can be unlock by beneficiary address.

## How to generate contract call contract CKB transaction?
When `CALL` or `CREATE` op code is invoked in EVM we call it a contract call contract transaction. When invoke `CALL` op code, `generator` load contract code and latest storage from database or saved state (the contract already been loaded) by `destination` and execute the program. When invoke `CREATE` op code, `generator` will put an output cell just like how contract creation works.

## How to validate contract call contract CKB transaction?
The first contract created or called by `sender` we call it **entrace** contract, other contracts if have any we call them normal contract. Only one program is allowed in **entrance** contract, and all its calls to normal contracts' programs must match the order and count. All normal contracts' calls to sub normal contracts' programs must check they are match the order, since one contract may called by multiple contracts the count can not be checked by normal contract. Normal contracts only check their own programs, **entrance** contract will check all programs in current CKB transaction are been called with restrict order.

## How to verify contract sender?
Since the `sender` information will be used in EVM exection, we require `sender` sign the polyjuice transaction, and put the signature into witness. There are two parts must be inclued in the sign target:
1. transaction hash
2. all contracts' related witnesses

Need to mention that, contract related witness is serialized as part of the `WitnessArgs` molecue structure, and the information is located in `input_type` (contract call/destruction) field or `output_type` (contract creation).

## How to handle logs?
In `validator`, the logs are just been ignored. When the polyjuice transaction is generated by `generator`, the logs are saved and returned as part of the transaction receipt. When the polyjuice transaction is processed by `indexer`, the logs are saved to database for user to query.

In `generator` and `indexer` the logs are trigged by `LOG` op code, then:
1. function `emit_log` callback function is called
2. `emit_log` invoke a log `syscall` with `topics` and `data` as arguments
3. Rust syscall callback function is called, the arguments been extracted and saved


[1]: https://github.com/nervosnetwork/rfcs/blob/master/rfcs/0022-transaction-structure/0022-transaction-structure.md#type-id
[2]: https://github.com/nervosnetwork/rfcs/blob/master/rfcs/0020-ckb-consensus-protocol/0020-ckb-consensus-protocol.md#dynamic-difficulty-adjustment-mechanism
[4]: https://github.com/nervosnetwork/rfcs/blob/master/rfcs/0014-vm-cycle-limits/0014-vm-cycle-limits.md
[5]: assets/polyjuice-transaction-structure.pdf
[6]: assets/polyjuice-how-generator-works.pdf
[7]: assets/polyjuice-how-validator-works.pdf
[8]: assets/polyjuice-transaction-structure.jpg
[9]: assets/polyjuice-how-generator-works.jpg
[10]: assets/polyjuice-how-validator-works.jpg
