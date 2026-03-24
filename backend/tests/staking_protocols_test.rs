use omni_backend::{
    indexers::{LidoIndexer, MarinadeIndexer, JitoIndexer, RocketPoolIndexer},
    models::{Chain, OperationType},
};

#[tokio::test]
async fn test_all_staking_protocols() {
    // Test Lido Ethereum
    let lido = LidoIndexer::new();
    let lido_eth = lido.fetch_rates(&Chain::Ethereum).await.unwrap();
    assert_eq!(lido_eth.len(), 2, "Lido should return 2 products on Ethereum (stETH + wstETH)");
    for rate in &lido_eth {
        assert_eq!(rate.operation_type, OperationType::Staking);
        assert!(rate.supply_apy > 0.0);
    }

    // Test Lido Solana
    let lido_sol = lido.fetch_rates(&Chain::Solana).await.unwrap();
    assert_eq!(lido_sol.len(), 1, "Lido should return 1 product on Solana (stSOL)");
    assert_eq!(lido_sol[0].operation_type, OperationType::Staking);

    // Test Marinade
    let marinade = MarinadeIndexer::new();
    let marinade_rates = marinade.fetch_rates(&Chain::Solana).await.unwrap();
    assert_eq!(marinade_rates.len(), 1, "Marinade should return 1 product (mSOL)");
    assert_eq!(marinade_rates[0].operation_type, OperationType::Staking);
    assert!(marinade_rates[0].supply_apy > 0.0);

    // Test Jito
    let jito = JitoIndexer::new();
    let jito_rates = jito.fetch_rates(&Chain::Solana).await.unwrap();
    assert_eq!(jito_rates.len(), 1, "Jito should return 1 product (JitoSOL)");
    assert_eq!(jito_rates[0].operation_type, OperationType::Staking);
    assert!(jito_rates[0].supply_apy > 0.0);
    assert!(jito_rates[0].rewards > 0.0, "Jito should have MEV rewards");

    // Test Rocket Pool
    let rocketpool = RocketPoolIndexer::new();
    let rp_rates = rocketpool.fetch_rates(&Chain::Ethereum).await.unwrap();
    assert_eq!(rp_rates.len(), 1, "Rocket Pool should return 1 product (rETH)");
    assert_eq!(rp_rates[0].operation_type, OperationType::Staking);
    assert!(rp_rates[0].supply_apy > 0.0);
}

#[tokio::test]
async fn test_staking_protocols_wrong_chain() {
    let marinade = MarinadeIndexer::new();
    let wrong_chain = marinade.fetch_rates(&Chain::Ethereum).await.unwrap();
    assert_eq!(wrong_chain.len(), 0, "Marinade should return 0 results on Ethereum");

    let rocketpool = RocketPoolIndexer::new();
    let wrong_chain2 = rocketpool.fetch_rates(&Chain::Solana).await.unwrap();
    assert_eq!(wrong_chain2.len(), 0, "Rocket Pool should return 0 results on Solana");
}

#[tokio::test]
async fn test_staking_apy_ranges() {
    // Sanity check APYs are in reasonable ranges
    let lido = LidoIndexer::new();
    let lido_eth = lido.fetch_rates(&Chain::Ethereum).await.unwrap();
    for rate in lido_eth {
        assert!(rate.supply_apy > 0.0 && rate.supply_apy < 20.0, "ETH staking APY should be 0-20%");
    }

    let marinade = MarinadeIndexer::new();
    let marinade_rates = marinade.fetch_rates(&Chain::Solana).await.unwrap();
    for rate in marinade_rates {
        assert!(rate.supply_apy > 0.0 && rate.supply_apy < 15.0, "SOL staking APY should be 0-15%");
    }
}
