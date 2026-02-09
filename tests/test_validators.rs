mod validators;

#[test]
fn test_ticker_validator_valid() {
    use ccxt::types::Ticker;
    use rust_decimal::Decimal;

    let ticker = Ticker {
        symbol: "BTC/USDT".to_string(),
        timestamp: 1700000000000,
        datetime: "2023-11-14T22:13:20.000Z".to_string(),
        high: Some(Decimal::new(51000, 0)),
        low: Some(Decimal::new(49000, 0)),
        bid: Some(Decimal::new(50000, 0)),
        bid_volume: Some(Decimal::new(10, 0)),
        ask: Some(Decimal::new(50100, 0)),
        ask_volume: Some(Decimal::new(5, 0)),
        vwap: Some(Decimal::new(50000, 0)),
        open: Some(Decimal::new(49500, 0)),
        close: Some(Decimal::new(50050, 0)),
        last: Some(Decimal::new(50050, 0)),
        previous_close: None,
        change: Some(Decimal::new(550, 0)),
        percentage: Some(Decimal::new(111, 2)),
        average: Some(Decimal::new(50000, 0)),
        base_volume: Some(Decimal::new(1000, 0)),
        quote_volume: Some(Decimal::new(50000000, 0)),
        index_price: None,
        mark_price: None,
        info: None,
    };
    let errors = validators::validate_ticker(&ticker);
    assert!(errors.is_empty(), "Validation errors: {:?}", errors);
}

#[test]
fn test_orderbook_validator_valid() {
    use ccxt::types::OrderBook;
    use rust_decimal::Decimal;

    let ob = OrderBook {
        symbol: "BTC/USDT".to_string(),
        timestamp: 1700000000000,
        datetime: "2023-11-14T22:13:20.000Z".to_string(),
        nonce: Some(12345),
        bids: vec![
            (Decimal::new(50000, 0), Decimal::new(1, 0)),
            (Decimal::new(49990, 0), Decimal::new(2, 0)),
        ],
        asks: vec![
            (Decimal::new(50010, 0), Decimal::new(1, 0)),
            (Decimal::new(50020, 0), Decimal::new(2, 0)),
        ],
        info: None,
    };
    let errors = validators::validate_order_book(&ob);
    assert!(errors.is_empty(), "Validation errors: {:?}", errors);
}

#[test]
fn test_trade_validator_valid() {
    use ccxt::types::common::OrderSide;
    use ccxt::types::Trade;
    use rust_decimal::Decimal;

    let trade = Trade {
        id: "12345".to_string(),
        symbol: "BTC/USDT".to_string(),
        order: None,
        timestamp: 1700000000000,
        datetime: "2023-11-14T22:13:20.000Z".to_string(),
        side: OrderSide::Buy,
        price: Decimal::new(50000, 0),
        amount: Decimal::new(1, 0),
        cost: Decimal::new(50000, 0),
        fee: None,
        taker_or_maker: Some("taker".to_string()),
        info: None,
    };
    let errors = validators::validate_trade(&trade);
    assert!(errors.is_empty(), "Validation errors: {:?}", errors);
}

#[test]
fn test_ohlcv_validator_valid() {
    use ccxt::types::OHLCV;
    use rust_decimal::Decimal;

    let ohlcv = OHLCV {
        timestamp: 1700000000000,
        open: Decimal::new(50000, 0),
        high: Decimal::new(51000, 0),
        low: Decimal::new(49000, 0),
        close: Decimal::new(50500, 0),
        volume: Decimal::new(1000, 0),
        info: None,
    };
    let errors = validators::validate_ohlcv(&ohlcv);
    assert!(errors.is_empty(), "Validation errors: {:?}", errors);
}

#[test]
fn test_market_validator_valid() {
    use ccxt::types::{Market, MarketLimits, MarketPrecision, MinMax};
    use rust_decimal::Decimal;

    let market = Market {
        id: "BTCUSDT".to_string(),
        symbol: "BTC/USDT".to_string(),
        base: "BTC".to_string(),
        quote: "USDT".to_string(),
        settle: None,
        base_id: "BTC".to_string(),
        quote_id: "USDT".to_string(),
        settle_id: None,
        market_type: "spot".to_string(),
        spot: true,
        margin: false,
        swap: false,
        future: false,
        option: false,
        active: true,
        contract: None,
        linear: None,
        inverse: None,
        taker: Some(Decimal::new(1, 3)),
        maker: Some(Decimal::new(1, 3)),
        contract_size: None,
        expiry: None,
        expiry_datetime: None,
        strike: None,
        option_type: None,
        created: None,
        margin_modes: None,
        precision: MarketPrecision {
            price: Some(2),
            amount: Some(5),
            cost: None,
            base: None,
            quote: None,
        },
        limits: MarketLimits {
            amount: Some(MinMax {
                min: Some(Decimal::new(1, 5)),
                max: Some(Decimal::new(9000, 0)),
            }),
            price: Some(MinMax {
                min: Some(Decimal::new(1, 2)),
                max: Some(Decimal::new(1000000, 0)),
            }),
            cost: Some(MinMax {
                min: Some(Decimal::new(10, 0)),
                max: None,
            }),
            leverage: None,
        },
        info: None,
    };
    let errors = validators::validate_market(&market);
    assert!(errors.is_empty(), "Validation errors: {:?}", errors);
}

#[test]
fn test_balance_validator_valid() {
    use ccxt::types::{Balance, Balances};
    use rust_decimal::Decimal;
    use std::collections::HashMap;

    let mut bals = HashMap::new();
    bals.insert(
        "BTC".to_string(),
        Balance::new("BTC".to_string(), Decimal::new(1, 0), Decimal::new(5, 1)),
    );
    let balances = Balances {
        timestamp: 1700000000000,
        datetime: "2023-11-14T22:13:20.000Z".to_string(),
        balances: bals,
        free: HashMap::new(),
        used: HashMap::new(),
        total: HashMap::new(),
        info: None,
    };
    let errors = validators::validate_balances(&balances);
    assert!(errors.is_empty(), "Validation errors: {:?}", errors);
}

#[test]
fn test_position_validator_valid() {
    use ccxt::types::common::{MarginMode, PositionSide};
    use ccxt::types::Position;
    use rust_decimal::Decimal;

    let position = Position {
        symbol: "BTC/USDT:USDT".to_string(),
        id: None,
        timestamp: 1700000000000,
        datetime: "2023-11-14T22:13:20.000Z".to_string(),
        side: PositionSide::Long,
        margin_mode: MarginMode::Cross,
        contracts: Decimal::new(1, 0),
        contract_size: Some(Decimal::ONE),
        notional: Some(Decimal::new(50000, 0)),
        leverage: Some(Decimal::new(10, 0)),
        entry_price: Some(Decimal::new(50000, 0)),
        mark_price: Some(Decimal::new(50100, 0)),
        unrealized_pnl: Some(Decimal::new(100, 0)),
        realized_pnl: None,
        collateral: None,
        initial_margin: Some(Decimal::new(5000, 0)),
        maintenance_margin: Some(Decimal::new(250, 0)),
        liquidation_price: Some(Decimal::new(45000, 0)),
        margin_ratio: None,
        percentage: None,
        stop_loss_price: None,
        take_profit_price: None,
        hedged: None,
        maintenance_margin_percentage: None,
        initial_margin_percentage: None,
        last_update_timestamp: None,
        last_price: None,
        info: None,
    };
    let errors = validators::validate_position(&position);
    assert!(errors.is_empty(), "Validation errors: {:?}", errors);
}

#[test]
fn test_funding_rate_validator_valid() {
    use ccxt::types::FundingRate;
    use rust_decimal::Decimal;

    let fr = FundingRate {
        symbol: "BTC/USDT:USDT".to_string(),
        timestamp: 1700000000000,
        datetime: "2023-11-14T22:13:20.000Z".to_string(),
        funding_rate: Decimal::new(1, 4),
        funding_timestamp: Some(1700028800000),
        funding_datetime: Some("2023-11-15T06:13:20.000Z".to_string()),
        mark_price: Some(Decimal::new(50000, 0)),
        index_price: Some(Decimal::new(50010, 0)),
        interest_rate: Some(Decimal::new(1, 4)),
        estimated_settle_price: None,
        interval: Some("8h".to_string()),
        previous_funding_rate: None,
        previous_funding_timestamp: None,
        previous_funding_datetime: None,
        next_funding_rate: None,
        next_funding_timestamp: None,
        next_funding_datetime: None,
        info: None,
    };
    let errors = validators::validate_funding_rate(&fr);
    assert!(errors.is_empty(), "Validation errors: {:?}", errors);
}
