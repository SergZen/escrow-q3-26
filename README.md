# Anchor Escrow Program

A secure, production-ready, and modular **Escrow Smart Contract** built on Solana using the **Anchor Framework**. This program enables trustless peer-to-peer token exchanges where a Maker can lock up tokens (`Token A`) and specify a desired amount of tokens (`Token B`) they wish to receive from a Taker.

## Overview

The program leverages Solana's **Program Derived Addresses (PDAs)** and Associated Token Accounts to securely hold tokens in escrow without relying on centralized intermediaries.

### Key Features
- **Modular Structure**: Clean separation of concerns with instruction handlers separated into dedicated files (`make`, `take`, `refund`, `update`).
- **State/Instruction Separation**: Leveraging Anchor’s best practices for account validation and execution logic.
- **Built-in Visuals**: Pre-designed architecture diagrams mapping out the main instruction workflows.

---

## Project Architecture

---

## Core Workflows & Instructions

### 1. Make Escrow (`make.rs`)
The **Maker** initializes the escrow account. 
- Creates an `Escrow` state account mapping the terms of the deal.
- Transfers the offered `Token A` from the Maker's token account into a secure Program-Owned Token Vault.
- *Visual schema can be found at:* `arch/make.png`

### 2. Take Escrow (`take.rs`)
The **Taker** fulfills the escrow terms.
- Transfers the requested `Token B` from the Taker to the Maker.
- Unlocks and transfers the escrowed `Token A` from the Vault to the Taker.
- Closes the Vault and Escrow state accounts, returning rent lamports back to the Maker.
- *Visual schema can be found at:* `arch/take.png`

### 3. Refund Escrow (`refund.rs`)
The **Maker** decides to cancel the active escrow.
- Verifies that the signer is the original Maker.
- Transfers locked `Token A` from the Vault back to the Maker's wallet.
- Closes the state accounts and reclaims rent.
- *Visual schema can be found at:* `arch/refund.png`

### 4. Update Terms (`update.rs`)
Allows the **Maker** to alter the escrow requirements before any Taker executes the swap.

---

## Prerequisites

Ensure you have the following tools installed:
- [Rust](https://rust-lang.org) (version specified in `rust-toolchain.toml`)
- [Solana CLI](https://solana.com)
- [Anchor CLI](https://anchor-lang.com)

---

## Getting Started

### 1. Clone & Install Dependencies
```bash
git clone https://github.com/SergZen/escrow-q3-26
cd escrow-q3-26
cargo build
```

### 2. Configure Your Environment
Update the `Anchor.toml` file with your local wallet path and preferred cluster network (localnet, devnet).

### 3. Build the Program
```bash
anchor build
```

### 4. Run Integration Tests
The repository includes a comprehensive testing builder (`escrow_test_builder.rs`) to test edge cases, success scenarios, and security validation.
```bash
anchor test
```

### 5. Deploy
```bash
anchor deploy
```

---

## Security & Best Practices
- **PDA Verification**: The escrow vault is securely tied to the program runtime using seeds defined in `constants.rs`.
- **Signer Checks**: Crucial instructions like `refund` and `update` mandate strict constraint checks (`mut, has_one = maker`) ensuring unauthorized wallets cannot drain funds.
- **Rent Reclamation**: All closure mechanisms safely return Solana Lamports allocated for storage state rent back to the initializer.