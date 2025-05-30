# LRC20

## LRC20 protocol

LRC20 is a protocol for creating tokenized assets on top of the Bitcoin protocol (analogous to ERC-20 tokens with basic functions like issuance, transfer, freezing, unfreezing and burning).
LRC20 is an open protocol that utilizes Bitcoin as the base layer to move digital assets. Any Bitcoin node can become a LRC20 node by running the LRC20 software, LRC20d. LRC20 nodes broadcast and process LRC20 transactions in a similar way to how Bitcoin nodes process bitcoin transactions.

Using the LRC20 protocol, an issuer can mint a stablecoin or issue other types of fungible tokens on Bitcoin, with the token type identified by the issuer through the issuer’s issuing key. Users can transfer these tokens – which we call LRC20coin – among each other without the issuer having to take action. 
The beauty of LRC20 lies in its open nature. You could just start a LRC20 node simply by running the LRC20d software. This network of LRC20 nodes then works together, much like the Bitcoin network itself, to broadcast LRC20 transactions to bitcoin and validate LRC20 transactions.

The LRC20 protocol prioritizes compliance by allowing LRC20coin issuers to freeze assets for regulatory, legal, or compliance-related reasons. This freeze instruction is broadcasted to the Bitcoin network, ensuring LRC20 nodes will block transactions involving the specified assets. Importantly, issuers retain the flexibility to "unfreeze" these assets should the situation change, demonstrating the LRC20 protocol's adaptability to real-world requirements and commitment to responsible digital asset management.

## Development

* [LRC20 book]() - TODO
* [Developer notes]() - TODO
* [Productivity notes]() - TODO
* [Source code documention]() - TODO build rust book
* [Local development setup](../infrastructure/README.md) - How to set up and running local development network
* [CLI client](../apps/cli/) - CLI client up and running
* [Node](../apps/node/) - LRC20 node up and running
* [Desktop Wallet]() - TODO

## Architecture

Before we dive deep into architecture, there is a component that works like glue to connect all our components. It's an `Event bus`. All communication, except the communication to storage goes through the `Event bus`. 

```mermaid
flowchart TD
    P2P --> |1.Received new transaction| TC[Transaction checker]
    TC --> |2.Isolated check for transaction| TC
    TC --> |3.Received tx to attach to DAG| TA[Transaction attacher]
    TA --> |4.Attach transaction to token DAG| TA
    TA --> |5.Get transaction needed to build DAG| S[Storage]
    TA --> |6.Request missing locally transaction to build DAG| P2P
    TA --> |7.When DAG is built, save all txs| S
    RA[RPC API] --> |8.Request data about transactions for client| S
    I[Indexer] --> |9.Add data about freeze/unfreeze for UTXOs| S
```

Communication:
* [Bitcoin client](../crates/bitcoin-client/) - asynchronous wrapper on top of `bitcoincore-rpc`.
* [Controller](../crates/controller/) - message handler for P2P and RPC.
* [P2P](../crates/p2p/) - bitcoin P2P to broadcast LRC20 data (and in future, get all data from bitcoin through P2P).
* [RPC api](../crates/rpc-api/) - description of RPC api in Rust traits. Used to generate RPC client for wallets and as specification for backend implementation.
* [RPC server](../crates/rpc-server/) - implementation of RPC api.

Event bus:
* [Event bus](../crates/event-bus/) and [Event bus macros](../event-bus-macros/) - event bus implementation, utility crate which provides a simple interface for managing event channels between internal services. It provides you the ability to create the `EventBus` instance which can be used to publish events and subscribe to them.

Storage:
* [Storage](../crates/storage/) - Provides traits and implementations of storage for LRC20 transactions.
* [Indexers](../crates/indexers/) - indexer for bitcoin blocks for LRC20 protocol needs.

Transactions
* [Devkit](../crates/dev-kit/) - database, txbuilder, coin selection and wallet implementation for LRC20 transactions.
* [Transaction checker](../crates/tx-check/) - functions and entities for isolated transactions checking.
* [Transaction attacher](../crates/tx-attach/) - service inside node which builds graph of dependencies between LRC20 transactions and stores one that are considered "attached".

Types:
* [Receipts](../crates/receipts/) - types for cryptography implementation to LRC20 protocol.
* [Bulletproof](../crates/bulletproof/) - bulletproofs++ implementation for lrc20 transactions with hidden amount.
* [Types](../crates/types/) - utility types.

## Transaction flow

The flow of a LRC20 transaction received in any way is described by the following sequence diagram:

```mermaid
sequenceDiagram
    participant External Sources
    participant Controller
    participant Tx Confirmator
    participant Tx Checker
    participant GraphBuilder
    participant Storage
    External Sources->>Controller: - Recieve new txs via P2P/RPC<br>- Recieve new announcements from the indexer
    Controller->>Storage: Add txs to the mempool with the "initialized" status
    Controller->>Tx Checker: Isolated check for the txs
    Tx Checker->>Controller: Notify about checked and invalid txs
    Controller->>Storage: Change the txs status in the mempool to "waiting-mined"
    Controller->>Tx Confirmator: Send the txs for confirmation
    Tx Confirmator->>Tx Confirmator: Wait for the first confirmation (1 block)
    Tx Confirmator->>Controller: Send mined txs to broadcast via P2P
    Controller->>Storage: Change the txs status in the mempool to "mined"
    Controller->>External Sources: Broadcast mined txs via P2P
    Tx Confirmator->>Tx Confirmator: Wait for the the full confirmation (6 blocks by default)
    Tx Confirmator->>Controller: Notify about confirmed txs
    Controller->>Tx Checker: Full check for txs
    Tx Checker->>Controller: Notify about checked and invalid txs
    Controller->>Storage: Change the txs status in the mempool to "attaching"
    Controller->>GraphBuilder: Send checked txs for attaching
    GraphBuilder->>GraphBuilder: Attach transactions to token DAG
    GraphBuilder->>Storage: Get txs needed to build the DAG
    GraphBuilder->>External Sources: Request locally missing txs from the P2P peers
    GraphBuilder->>Storage: When the DAG is built, save attached txs
    GraphBuilder->>Controller: Notify about attached txs
    Controller->>Storage: Remove attached and invalid txs from the mempool
```
