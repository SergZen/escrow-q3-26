use std::path::PathBuf;

use anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas};
use anchor_spl::token::spl_token::state::Account as TokenAccount;
use litesvm::LiteSVM;
use solana_program::program_pack::Pack;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_system_interface::instruction as system_ix;
use solana_transaction::Transaction;

const TOKEN_PROGRAM: Pubkey = Pubkey::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM: Pubkey = Pubkey::from_str_const("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const SYSTEM_PROGRAM: Pubkey = Pubkey::from_str_const("11111111111111111111111111111111");

const MINT_LEN: usize = 82;
const TOKEN_ACC_LEN: usize = 165;
const VAULT_LEN: usize = 8 + 32 + 32 + 1;

const SOL: u64 = 1_000_000_000;
const FEE: u64 = 5_000;
const INITIAL: u64 = 1_000_000_000;

fn pid() -> Pubkey {
    token_vault::ID
}

fn find_so() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let candidate = dir.join("target/deploy/token_vault.so");
        if candidate.exists() {
            return candidate;
        }
        if !dir.pop() {
            panic!("target/deploy/token_vault.so not found - run `anchor build` first");
        }
    }
}

fn send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    ixs: &[Instruction],
    extra: &[&Keypair],
) -> Result<(), String> {
    svm.expire_blockhash();
    let mut signers: Vec<&Keypair> = vec![payer];
    signers.extend_from_slice(extra);
    let tx = Transaction::new_signed_with_payer(
        ixs,
        Some(&payer.pubkey()),
        &signers[..],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e.err))
}

fn ata(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[wallet.as_ref(), TOKEN_PROGRAM.as_ref(), mint.as_ref()],
        &ATA_PROGRAM,
    )
    .0
}

fn vault_pda(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[token_vault::VAULT_SEED, owner.as_ref(), mint.as_ref()],
        &pid(),
    )
    .0
}

fn get_token_account(svm: &LiteSVM, acc: &Pubkey) -> TokenAccount {
    let account_data = svm
        .get_account(acc)
        .as_ref()
        .map(|a| a.data.clone())
        .expect("Token account details should exist");

    TokenAccount::unpack(&account_data)
        .expect("Failed to deserialize SPL Token account")
}

fn token_balance(svm: &LiteSVM, acc: &Pubkey) -> u64 {
    get_token_account(svm, acc).amount
}

fn token_owner(svm: &LiteSVM, acc: &Pubkey) -> Pubkey {
    get_token_account(svm, acc).owner
}

fn token_mint(svm: &LiteSVM, acc: &Pubkey) -> Pubkey {
    get_token_account(svm, acc).mint
}

fn lamports(svm: &LiteSVM, k: &Pubkey) -> u64 {
    svm.get_balance(k).unwrap_or(0)
}

fn exists(svm: &LiteSVM, k: &Pubkey) -> bool {
    svm.get_account(k).map_or(false, |a| a.lamports > 0)
}

fn create_mint(svm: &mut LiteSVM, authority: &Keypair, decimals: u8) -> Pubkey {
    let mint = Keypair::new();
    let rent = svm.minimum_balance_for_rent_exemption(MINT_LEN);

    let mut data = vec![20u8, decimals];
    data.extend_from_slice(authority.pubkey().as_ref());
    data.push(0);
    let ixs = [
        system_ix::create_account(&authority.pubkey(), &mint.pubkey(), rent, MINT_LEN as u64, &TOKEN_PROGRAM),
        Instruction {
            program_id: TOKEN_PROGRAM,
            accounts: vec![AccountMeta::new(mint.pubkey(), false)],
            data,
        },
    ];
    send(svm, authority, &ixs, &[&mint]).unwrap();
    mint.pubkey()
}

fn create_ata_ix(payer: &Pubkey, wallet: &Pubkey, mint: &Pubkey) -> Instruction {
    Instruction {
        program_id: ATA_PROGRAM,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(ata(wallet, mint), false),
            AccountMeta::new_readonly(*wallet, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM, false),
        ],
        data: vec![1],
    }
}

fn create_ata(svm: &mut LiteSVM, payer: &Keypair, wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    send(svm, payer, &[create_ata_ix(&payer.pubkey(), wallet, mint)], &[]).unwrap();
    ata(wallet, mint)
}

fn create_plain_token_account(svm: &mut LiteSVM, payer: &Keypair, mint: &Pubkey, owner: &Pubkey) -> Pubkey {
    let acc = Keypair::new();
    let rent = svm.minimum_balance_for_rent_exemption(TOKEN_ACC_LEN);

    let mut data = vec![18u8];
    data.extend_from_slice(owner.as_ref());
    let ixs = [
        system_ix::create_account(&payer.pubkey(), &acc.pubkey(), rent, TOKEN_ACC_LEN as u64, &TOKEN_PROGRAM),
        Instruction {
            program_id: TOKEN_PROGRAM,
            accounts: vec![
                AccountMeta::new(acc.pubkey(), false),
                AccountMeta::new_readonly(*mint, false),
            ],
            data,
        },
    ];
    send(svm, payer, &ixs, &[&acc]).unwrap();
    acc.pubkey()
}

fn mint_to(svm: &mut LiteSVM, authority: &Keypair, mint: &Pubkey, dest: &Pubkey, amount: u64) {
    let mut data = vec![7u8];
    data.extend_from_slice(&amount.to_le_bytes());
    let ix = Instruction {
        program_id: TOKEN_PROGRAM,
        accounts: vec![
            AccountMeta::new(*mint, false),
            AccountMeta::new(*dest, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        data,
    };
    send(svm, authority, &[ix], &[]).unwrap();
}

fn transfer(svm: &mut LiteSVM, owner: &Keypair, from: &Pubkey, to: &Pubkey, amount: u64) {
    let mut data = vec![3u8];
    data.extend_from_slice(&amount.to_le_bytes());
    let ix = Instruction {
        program_id: TOKEN_PROGRAM,
        accounts: vec![
            AccountMeta::new(*from, false),
            AccountMeta::new(*to, false),
            AccountMeta::new_readonly(owner.pubkey(), true),
        ],
        data,
    };
    send(svm, owner, &[ix], &[]).unwrap();
}

fn err_code(e: token_vault::error::VaultError) -> u32 {
    anchor_lang::error::ERROR_CODE_OFFSET + e as u32
}

fn assert_custom(res: Result<(), String>, code: u32) {
    match res {
        Err(e) if e.contains(&format!("Custom({code})")) => {}
        other => panic!("expected Custom({code}), got {other:?}"),
    }
}

fn ix_initialize(owner: &Pubkey, mint: &Pubkey) -> Instruction {
    let vault = vault_pda(owner, mint);
    Instruction {
        program_id: pid(),
        accounts: token_vault::accounts::Initialize {
            owner: *owner,
            mint: *mint,
            vault,
            vault_token: ata(&vault, mint),
            associated_token_program: ATA_PROGRAM,
            token_program: TOKEN_PROGRAM,
            system_program: SYSTEM_PROGRAM,
        }
        .to_account_metas(None),
        data: token_vault::instruction::Initialize {}.data(),
    }
}

fn ix_deposit(
    depositor: &Pubkey,
    depositor_token: &Pubkey,
    vault_owner: &Pubkey,
    mint: &Pubkey,
    amount: u64,
) -> Instruction {
    let vault = vault_pda(vault_owner, mint);
    Instruction {
        program_id: pid(),
        accounts: token_vault::accounts::Deposit {
            depositor: *depositor,
            mint: *mint,
            vault,
            vault_token: ata(&vault, mint),
            depositor_token: *depositor_token,
            associated_token_program: ATA_PROGRAM,
            token_program: TOKEN_PROGRAM,
        }
        .to_account_metas(None),
        data: token_vault::instruction::Deposit { amount }.data(),
    }
}

fn ix_withdraw(
    signer: &Pubkey,
    dest: &Pubkey,
    vault_of: &Pubkey,
    mint: &Pubkey,
    amount: u64,
) -> Instruction {
    let vault = vault_pda(vault_of, mint);
    Instruction {
        program_id: pid(),
        accounts: token_vault::accounts::Withdraw {
            owner: *signer,
            mint: *mint,
            vault,
            vault_token: ata(&vault, mint),
            owner_token: *dest,
            associated_token_program: ATA_PROGRAM,
            token_program: TOKEN_PROGRAM,
        }
        .to_account_metas(None),
        data: token_vault::instruction::Withdraw { amount }.data(),
    }
}

fn ix_close(signer: &Pubkey, dest: &Pubkey, vault_of: &Pubkey, mint: &Pubkey) -> Instruction {
    let vault = vault_pda(vault_of, mint);
    Instruction {
        program_id: pid(),
        accounts: token_vault::accounts::CloseVault {
            owner: *signer,
            mint: *mint,
            vault,
            vault_token: ata(&vault, mint),
            owner_token: *dest,
            associated_token_program: ATA_PROGRAM,
            token_program: TOKEN_PROGRAM,
        }
        .to_account_metas(None),
        data: token_vault::instruction::Close {}.data(),
    }
}
struct Env {
    svm: LiteSVM,
    owner: Keypair,
    mint: Pubkey,
    owner_token: Pubkey,
}

impl Env {
    fn owner_pk(&self) -> Pubkey {
        self.owner.pubkey()
    }
    fn vault(&self) -> Pubkey {
        vault_pda(&self.owner_pk(), &self.mint)
    }
    fn vault_token(&self) -> Pubkey {
        ata(&self.vault(), &self.mint)
    }
    fn run(&mut self, ix: Instruction) -> Result<(), String> {
        send(&mut self.svm, &self.owner, &[ix], &[])
    }
    fn init(&mut self) {
        let ix = ix_initialize(&self.owner_pk(), &self.mint);
        self.run(ix).unwrap();
    }
    fn deposit(&mut self, amount: u64) -> Result<(), String> {
        let ix = ix_deposit(&self.owner_pk(), &self.owner_token, &self.owner_pk(), &self.mint, amount);
        self.run(ix)
    }
    fn withdraw(&mut self, amount: u64) -> Result<(), String> {
        let ix = ix_withdraw(&self.owner_pk(), &self.owner_token, &self.owner_pk(), &self.mint, amount);
        self.run(ix)
    }
    fn close(&mut self) -> Result<(), String> {
        let ix = ix_close(&self.owner_pk(), &self.owner_token, &self.owner_pk(), &self.mint);
        self.run(ix)
    }
    fn new_user(&mut self, mint_amount: u64) -> (Keypair, Pubkey) {
        let user = Keypair::new();
        self.svm.airdrop(&user.pubkey(), SOL).unwrap();
        let user_ata = create_ata(&mut self.svm, &user, &user.pubkey(), &self.mint);
        if mint_amount > 0 {
            mint_to(&mut self.svm, &self.owner, &self.mint, &user_ata, mint_amount);
        }
        (user, user_ata)
    }
}

fn setup() -> Env {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(pid(), find_so()).unwrap();

    let owner = Keypair::new();
    svm.airdrop(&owner.pubkey(), 10 * SOL).unwrap();
    let mint = create_mint(&mut svm, &owner, 6);
    let owner_token = create_ata(&mut svm, &owner, &owner.pubkey(), &mint);
    mint_to(&mut svm, &owner, &mint, &owner_token, INITIAL);
    Env { svm, owner, mint, owner_token }
}

#[test]
fn initialize_creates_vault_and_ata() {
    let mut e = setup();
    e.init();

    let vault = e.svm.get_account(&e.vault()).unwrap();
    assert_eq!(vault.owner, pid());
    assert_eq!(vault.data.len(), VAULT_LEN);
    assert_eq!(&vault.data[8..40], e.owner_pk().as_ref());
    assert_eq!(&vault.data[40..72], e.mint.as_ref());

    let vt = e.vault_token();
    assert_eq!(e.svm.get_account(&vt).unwrap().owner, TOKEN_PROGRAM);
    assert_eq!(token_mint(&e.svm, &vt), e.mint);
    assert_eq!(token_owner(&e.svm, &vt), e.vault());
    assert_eq!(token_balance(&e.svm, &vt), 0);
}

#[test]
fn initialize_twice_fails() {
    let mut e = setup();
    e.init();
    let ix = ix_initialize(&e.owner_pk(), &e.mint);
    assert!(e.run(ix).is_err());
}

#[test]
fn separate_vaults_per_mint() {
    let mut e = setup();
    e.init();
    let mint2 = create_mint(&mut e.svm, &e.owner, 9);
    let ix = ix_initialize(&e.owner_pk(), &mint2);
    e.run(ix).unwrap();
    assert!(exists(&e.svm, &vault_pda(&e.owner_pk(), &mint2)));
    assert!(exists(&e.svm, &ata(&vault_pda(&e.owner_pk(), &mint2), &mint2)));
}

#[test]
fn separate_vaults_per_owner() {
    let mut e = setup();
    e.init();
    let (user, _) = e.new_user(0);
    let ix = ix_initialize(&user.pubkey(), &e.mint);
    send(&mut e.svm, &user, &[ix], &[]).unwrap();
    assert_ne!(vault_pda(&user.pubkey(), &e.mint), e.vault());
    assert!(exists(&e.svm, &vault_pda(&user.pubkey(), &e.mint)));
}

#[test]
fn deposit_moves_tokens() {
    let mut e = setup();
    e.init();
    e.deposit(400).unwrap();
    assert_eq!(token_balance(&e.svm, &e.vault_token()), 400);
    assert_eq!(token_balance(&e.svm, &e.owner_token), INITIAL - 400);
}

#[test]
fn deposit_accumulates() {
    let mut e = setup();
    e.init();
    e.deposit(100).unwrap();
    e.deposit(250).unwrap();
    assert_eq!(token_balance(&e.svm, &e.vault_token()), 350);
}

#[test]
fn third_party_can_deposit() {
    let mut e = setup();
    e.init();
    let (donor, donor_ata) = e.new_user(50);

    let ix = ix_deposit(&donor.pubkey(), &donor_ata, &e.owner_pk(), &e.mint, 50);
    send(&mut e.svm, &donor, &[ix], &[]).unwrap();
    assert_eq!(token_balance(&e.svm, &e.vault_token()), 50);
    assert_eq!(token_balance(&e.svm, &donor_ata), 0);
}

#[test]
fn deposit_zero_fails() {
    let mut e = setup();
    e.init();
    assert_custom(e.deposit(0), err_code(token_vault::error::VaultError::InvalidAmount));
}

#[test]
fn deposit_more_than_balance_fails() {
    let mut e = setup();
    e.init();
    assert!(e.deposit(INITIAL + 1).is_err());
    assert_eq!(token_balance(&e.svm, &e.vault_token()), 0);
}

#[test]
fn deposit_from_non_ata_token_account_fails() {
    let mut e = setup();
    e.init();

    let owner = &e.owner;
    let mint = &e.mint;
    let owner_pk = e.owner_pk();

    let plain = create_plain_token_account(&mut e.svm, &owner, &mint, &owner_pk);
    transfer(&mut e.svm, &e.owner, &e.owner_token, &plain, 100);

    let ix = ix_deposit(&e.owner_pk(), &plain, &e.owner_pk(), &e.mint, 100);
    assert!(e.run(ix).is_err());
    assert_eq!(token_balance(&e.svm, &e.vault_token()), 0);
}

#[test]
fn deposit_from_someone_elses_ata_fails() {
    let mut e = setup();
    e.init();
    let (_victim, victim_ata) = e.new_user(100);

    let ix = ix_deposit(&e.owner_pk(), &victim_ata, &e.owner_pk(), &e.mint, 100);
    assert!(e.run(ix).is_err());
    assert_eq!(token_balance(&e.svm, &victim_ata), 100);
}

#[test]
fn deposit_with_wrong_mint_fails() {
    let mut e = setup();
    e.init();

    let owner = &e.owner;
    let owner_pk = e.owner_pk();
    
    let other_mint = create_mint(&mut e.svm, &e.owner, 6);
    let other_ata = create_ata(&mut e.svm, &owner, &owner_pk, &other_mint);
    mint_to(&mut e.svm, &e.owner, &other_mint, &other_ata, 100);

    let ix = ix_deposit(&e.owner_pk(), &other_ata, &e.owner_pk(), &other_mint, 100);
    assert!(e.run(ix).is_err());
    assert_eq!(token_balance(&e.svm, &e.vault_token()), 0);
}

#[test]
fn withdraw_partial_succeeds() {
    let mut e = setup();
    e.init();
    e.deposit(1_000).unwrap();
    e.withdraw(300).unwrap();
    assert_eq!(token_balance(&e.svm, &e.vault_token()), 700);
    assert_eq!(token_balance(&e.svm, &e.owner_token), INITIAL - 700);
}

#[test]
fn withdraw_full_leaves_empty_vault_open() {
    let mut e = setup();
    e.init();
    e.deposit(1_000).unwrap();
    e.withdraw(1_000).unwrap();
    assert_eq!(token_balance(&e.svm, &e.vault_token()), 0);
    assert_eq!(token_balance(&e.svm, &e.owner_token), INITIAL);
    assert!(exists(&e.svm, &e.vault()));
    e.deposit(5).unwrap();
}

#[test]
fn withdraw_more_than_balance_fails() {
    let mut e = setup();
    e.init();
    e.deposit(100).unwrap();
    assert_custom(e.withdraw(101), err_code(token_vault::error::VaultError::InsufficientFunds));
}

#[test]
fn withdraw_zero_fails() {
    let mut e = setup();
    e.init();
    e.deposit(100).unwrap();
    assert_custom(e.withdraw(0), err_code(token_vault::error::VaultError::InvalidAmount));
}

#[test]
fn withdraw_by_non_owner_fails() {
    let mut e = setup();
    e.init();
    e.deposit(500).unwrap();
    let (attacker, attacker_ata) = e.new_user(0);

    let ix = ix_withdraw(&attacker.pubkey(), &attacker_ata, &e.owner_pk(), &e.mint, 500);
    assert!(send(&mut e.svm, &attacker, &[ix], &[]).is_err());
    assert_eq!(token_balance(&e.svm, &e.vault_token()), 500);
    assert_eq!(token_balance(&e.svm, &attacker_ata), 0);
}

#[test]
fn withdraw_to_someone_elses_ata_fails() {
    let mut e = setup();
    e.init();
    e.deposit(500).unwrap();
    let (_other, other_ata) = e.new_user(0);

    let ix = ix_withdraw(&e.owner_pk(), &other_ata, &e.owner_pk(), &e.mint, 500);
    assert!(e.run(ix).is_err());
    assert_eq!(token_balance(&e.svm, &e.vault_token()), 500);
    assert_eq!(token_balance(&e.svm, &other_ata), 0);
}

#[test]
fn withdraw_to_owners_non_ata_account_fails() {
    let mut e = setup();
    e.init();
    e.deposit(500).unwrap();

    let owner = &e.owner;
    let mint = &e.mint;
    let owner_pk = e.owner_pk();

    let plain = create_plain_token_account(&mut e.svm, &owner, &mint, &owner_pk);

    let ix = ix_withdraw(&e.owner_pk(), &plain, &owner_pk, &mint, 500);
    assert!(e.run(ix).is_err());
    assert_eq!(token_balance(&e.svm, &e.vault_token()), 500);
}

#[test]
fn withdraw_with_non_ata_vault_token_fails() {
    let mut e = setup();
    e.init();
    e.deposit(500).unwrap();

    let owner = &e.owner;
    let mint = &e.mint;
    let vault = &e.vault();

    let fake = create_plain_token_account(&mut e.svm, &owner, &mint, &vault);
    mint_to(&mut e.svm, &owner, &mint, &fake, 999);

    let mut ix = ix_withdraw(&e.owner_pk(), &e.owner_token, &e.owner_pk(), &e.mint, 100);
    ix.accounts[3].pubkey = fake;
    assert!(e.run(ix).is_err());
}

#[test]
fn close_sweeps_tokens_and_refunds_rent() {
    let mut e = setup();
    e.init();
    e.deposit(750).unwrap();

    let vault_rent = e.svm.minimum_balance_for_rent_exemption(VAULT_LEN);
    let ata_rent = e.svm.minimum_balance_for_rent_exemption(TOKEN_ACC_LEN);
    let sol_before = lamports(&e.svm, &e.owner_pk());

    e.close().unwrap();

    assert!(!exists(&e.svm, &e.vault()));
    assert!(!exists(&e.svm, &e.vault_token()));
    assert_eq!(token_balance(&e.svm, &e.owner_token), INITIAL);
    assert_eq!(
        lamports(&e.svm, &e.owner_pk()),
        sol_before + vault_rent + ata_rent - FEE
    );
}

#[test]
fn close_empty_vault_succeeds() {
    let mut e = setup();
    e.init();
    e.close().unwrap();
    assert!(!exists(&e.svm, &e.vault()));
    assert!(!exists(&e.svm, &e.vault_token()));
    assert_eq!(token_balance(&e.svm, &e.owner_token), INITIAL);
}

#[test]
fn close_recovers_tokens_sent_directly_to_vault_ata() {
    let mut e = setup();
    e.init();

    let vt = e.vault_token();
    transfer(&mut e.svm, &e.owner, &e.owner_token, &vt, 300);
    assert_eq!(token_balance(&e.svm, &vt), 300);

    e.close().unwrap();
    assert_eq!(token_balance(&e.svm, &e.owner_token), INITIAL);
    assert!(!exists(&e.svm, &vt));
}

#[test]
fn close_by_non_owner_fails() {
    let mut e = setup();
    e.init();
    e.deposit(200).unwrap();
    let (attacker, attacker_ata) = e.new_user(0);

    let ix = ix_close(&attacker.pubkey(), &attacker_ata, &e.owner_pk(), &e.mint);
    assert!(send(&mut e.svm, &attacker, &[ix], &[]).is_err());
    assert!(exists(&e.svm, &e.vault()));
    assert_eq!(token_balance(&e.svm, &e.vault_token()), 200);
    assert_eq!(token_balance(&e.svm, &attacker_ata), 0);
}

#[test]
fn close_to_someone_elses_ata_fails() {
    let mut e = setup();
    e.init();
    e.deposit(200).unwrap();
    let (_other, other_ata) = e.new_user(0);

    let ix = ix_close(&e.owner_pk(), &other_ata, &e.owner_pk(), &e.mint);
    assert!(e.run(ix).is_err());
    assert!(exists(&e.svm, &e.vault()));
    assert_eq!(token_balance(&e.svm, &e.vault_token()), 200);
}

#[test]
fn operations_fail_after_close_and_vault_can_be_recreated() {
    let mut e = setup();
    e.init();
    e.close().unwrap();

    assert!(e.deposit(1).is_err());
    assert!(e.withdraw(1).is_err());

    e.init();
    e.deposit(10).unwrap();
    assert_eq!(token_balance(&e.svm, &e.vault_token()), 10);
}
