# Constant-Product Automated Market Maker (AMM)

A production-grade, constant-product Automated Market Maker (AMM) built on Solana using the **Anchor Framework**. This protocol enables users to initialize liquidity pools for arbitrary SPL token pairs, deposit symmetric liquidity, execute trading swaps matching the $x \cdot y = k$ invariant with an integrated 0.3% LP fee, and dynamically burn LP shares to withdraw underlying reserves.

---

## 📊 Protocol Architecture & PDA Seeds

The protocol enforces airtight isolation using strictly derived canonical Program Derived Addresses (PDAs). Every pool has a singular state instance and a single authority key managing separate secure token vaults.

### 1. Pool State Account
* **Seeds**: `[b"pool", payer_pubkey]`
* **Responsibility**: Tracks administrative configurations, token vault addresses, LP mint addresses, and pool-specific math metadata.

### 2. Pool Authority Account
* **Seeds**: `[b"authority", pool_state_pubkey]`
* **Responsibility**: Serves as the global owner of the Token Vaults and holds sole minting privileges for the LP Mint account. 

### 3. Account Layout Map

                 +---------------------------------------+
                 |           Pool State PDA              |
                 +---------------------------------------+
                                     |
                                     v
                 +---------------------------------------+
                 |         Pool Authority PDA            |
                 +---------------------------------------+
                  /                  |                  \
                 v                   v                   v
    +-----------------+     +-----------------+     +-----------------+
    |  Token Vault A  |     |  Token Vault B  |     |   LP Token Mint |
    | (Owned by PDA)  |     | (Owned by PDA)  |     | (Auth by PDA)   |
    +-----------------+     +-----------------+     +-----------------+

---

## 🧮 Core Invariant & AMM Math

### Constant-Product Formula
All token swap executions tightly adhere to the standard constant-product model:
$$x \cdot y = k$$

Where:
* $x$ = Reserve balance of Token A
* $y$ = Reserve balance of Token B
* $k$ = Invariant liquidity depth constant

### Fee Application (0.3%)
A trading fee of **0.3%** is deducted from the incoming input amount before computing the final output token layout, accruing liquidity linearly back into active pool reserves for active LPs:
$$\Delta y = \frac{y \cdot \Delta x \cdot 0.997}{x + (\Delta x \cdot 0.997)}$$

### Share Accounting (Liquidity Operations)
* **Initial Deposit**: Uses a geometric mean square-root approximation bounding logic to determine the base allocation of LP tokens when $L_{\text{supply}} = 0$.
* **Proportional Depositing / Withdrawals**: Ensures that every downstream liquidity adjustment cleanly balances current state reserve ratios perfectly. Rounding structures always lean in favor of protecting active pool assets against first-depositor inflation exploits.

---

## 🔒 Security Posture & Hardening

* **Reinitialization Defenses**: Secured natively via Anchor account discriminators, restricting double initialization attacks across active pool states.
* **Overflow Immunity**: Built completely using checked integer math definitions (`checked_mul`, `checked_add`, `checked_div`), eliminating panic paths across high-precision execution routes.
* **Privilege Validation**: Enforces localized signer checks on incoming user interfaces, isolating instruction vectors from spoofed parameters.

---

## 🛠️ Local Verification & Testing Suite

The project includes an extensive modular integration testing engine validating runtime state behaviors directly using `LiteSVM`.

### Run the Integration Tests
Execute the comprehensive end-to-end framework test targets natively via your terminal:

```bash
# Test Initialization
cargo test --test test_initialize -- --nocapture

# Test Swap Execution Math & Fee Allocation
cargo test --test test_swap -- --nocapture

# Test Symmetric Liquidity Deposits
cargo test --test test_deposit_liquidity -- --nocapture

# Test LP Share Proportional Burn Redemptions
cargo test --test test_withdraw -- --nocapture

---