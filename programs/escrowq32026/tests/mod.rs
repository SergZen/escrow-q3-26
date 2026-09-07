mod escrow_test_builder;

#[cfg(test)]
mod tests {
    use crate::escrow_test_builder::EscrowTestBuilder;

    #[test]
    fn test_make() {
        let deposit = 10u64;
        let seed = 123u64;
        let receive = 10u64;
        let expiration = 10000;

        let builder = EscrowTestBuilder::new()
            .create_mints()
            .create_maker_ata_a()
            .mint_to_maker_ata_a(deposit)
            .execute_make(deposit, seed, receive, expiration);

        let vault_data = builder.get_vault_data();
        assert_eq!(vault_data.amount, deposit);
        assert_eq!(vault_data.owner, builder.escrow());
        assert_eq!(vault_data.mint, builder.mint_a());

        let escrow_data = builder.get_escrow_data();
        assert_eq!(escrow_data.seed, seed);
        assert_eq!(escrow_data.maker, builder.maker_pubkey());
        assert_eq!(escrow_data.mint_a, builder.mint_a());
        assert_eq!(escrow_data.mint_b, builder.mint_b());
        assert_eq!(escrow_data.receive, receive);
        assert_eq!(escrow_data.expiration, expiration);
    }

    #[test]
    fn test_update() {
        let deposit = 10u64;
        let seed = 123u64;
        let receive = 10u64;
        let expiration = 10000;
        let new_expiration = 20000;

        let builder = EscrowTestBuilder::new()
            .create_mints()
            .create_maker_ata_a()
            .mint_to_maker_ata_a(deposit)
            .execute_make(deposit, seed, receive, expiration)
            .execute_update(seed, new_expiration);

        let vault_data = builder.get_vault_data();
        assert_eq!(vault_data.amount, deposit);
        assert_eq!(vault_data.owner, builder.escrow());
        assert_eq!(vault_data.mint, builder.mint_a());

        let escrow_data = builder.get_escrow_data();
        assert_eq!(escrow_data.seed, seed);
        assert_eq!(escrow_data.maker, builder.maker_pubkey());
        assert_eq!(escrow_data.mint_a, builder.mint_a());
        assert_eq!(escrow_data.mint_b, builder.mint_b());
        assert_eq!(escrow_data.receive, receive);
        assert_eq!(escrow_data.expiration, new_expiration);
    }

    #[test]
    fn test_take_before_unlock() {
        let deposit = 20u64;
        let seed = 123u64;
        let receive = 30u64;
        let expiration = 10000;

        let builder = EscrowTestBuilder::new()
            .create_mints()
            .create_maker_ata_a()
            .mint_to_maker_ata_a(deposit)
            .execute_make(deposit, seed, receive, expiration)
            .setup_taker()
            .create_maker_ata_b()
            .create_taker_atas()
            .mint_to_taker_ata_b(receive)
            .execute_take();

        assert!(builder.last_tx_succeeded());

        let taker_ata_a_data = builder.get_taker_ata_a_data();
        assert_eq!(taker_ata_a_data.amount, deposit);

        let taker_ata_b_data = builder.get_taker_ata_b_data();
        assert_eq!(taker_ata_b_data.amount, 0);

        let maker_ata_b_data = builder.get_maker_ata_b_data();
        assert_eq!(maker_ata_b_data.amount, receive);
    }

    #[test]
    fn test_take_after_unlock() {
        let deposit = 20u64;
        let seed = 123u64;
        let receive = 30u64;
        let expiration = 10000;

        let builder = EscrowTestBuilder::new()
            .create_mints()
            .create_maker_ata_a()
            .mint_to_maker_ata_a(deposit)
            .execute_make(deposit, seed, receive, expiration)
            .advance_time(expiration + 1)
            .setup_taker()
            .create_maker_ata_b()
            .create_taker_atas()
            .mint_to_taker_ata_b(receive)
            .execute_take();

        assert!(
            builder.last_tx_failed(),
            "Take should fail before unlock time"
        );

        let taker_ata_a_data = builder.get_taker_ata_a_data();
        assert_eq!(taker_ata_a_data.amount, 0);

        let taker_ata_b_data = builder.get_taker_ata_b_data();
        assert_eq!(taker_ata_b_data.amount, receive);

        let maker_ata_b_data = builder.get_maker_ata_b_data();
        assert_eq!(maker_ata_b_data.amount, 0);
    }

    #[test]
    fn test_refund_before_unlock() {
        let deposit = 20u64;
        let seed = 123u64;
        let receive = 30u64;
        let expiration = 10000;

        let builder = EscrowTestBuilder::new()
            .create_mints()
            .create_maker_ata_a()
            .mint_to_maker_ata_a(deposit)
            .execute_make(deposit, seed, receive, expiration)
            .execute_refund();

        assert!(
            builder.last_tx_failed(),
            "Refund should fail before unlock time"
        );

        let maker_ata_a_data = builder.get_maker_ata_a_data();
        assert_eq!(maker_ata_a_data.amount, 0);
        assert_eq!(maker_ata_a_data.owner, builder.maker_pubkey());
        assert_eq!(maker_ata_a_data.mint, builder.mint_a());
    }

    #[test]
    fn test_refund_after_unlock() {
        let deposit = 20u64;
        let seed = 123u64;
        let receive = 30u64;
        let expiration = 10000;

        let builder = EscrowTestBuilder::new()
            .create_mints()
            .create_maker_ata_a()
            .mint_to_maker_ata_a(deposit)
            .execute_make(deposit, seed, receive, expiration)
            .advance_time(expiration + 1)
            .execute_refund();

        let maker_ata_a_data = builder.get_maker_ata_a_data();
        assert_eq!(maker_ata_a_data.amount, deposit);
        assert_eq!(maker_ata_a_data.owner, builder.maker_pubkey());
        assert_eq!(maker_ata_a_data.mint, builder.mint_a());
    }
}
