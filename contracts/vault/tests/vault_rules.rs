use near_api::{AccountId, NearGas, NearToken};
use near_sdk::serde_json::{json, Value};

/// Call a contract method as `$from`. Evaluates to the execution result.
macro_rules! call {
    ($contract:expr, $net:expr, $signer:expr, $from:expr, $method:expr, $args:expr) => {
        $contract
            .call_function($method, $args)
            .transaction()
            .gas(NearGas::from_tgas(30))
            .with_signer($from.account_id().clone(), $signer.clone())
            .send_to(&$net)
            .await?
    };
}

/// Milli-NEAR as a yoctoNEAR decimal string, the JSON format the contract expects.
fn y(milli: u128) -> String {
    (milli * 10u128.pow(21)).to_string()
}

async fn create_subaccount(
    sandbox: &near_sandbox::Sandbox,
    name: &str,
) -> testresult::TestResult<near_api::Account> {
    let account_id: AccountId = name.parse().unwrap();
    sandbox
        .create_account(account_id.clone())
        .initial_balance(NearToken::from_near(10))
        .send()
        .await?;
    Ok(near_api::Account(account_id))
}

async fn balance(
    account: &near_api::Account,
    net: &near_api::NetworkConfig,
) -> testresult::TestResult<NearToken> {
    Ok(account.tokens().near_balance().fetch_from(net).await?.total)
}

async fn vault_balance(
    contract: &near_api::Contract,
    net: &near_api::NetworkConfig,
) -> testresult::TestResult<NearToken> {
    let b: NearToken = contract
        .call_function("get_balance", ())
        .read_only()
        .fetch_from(net)
        .await?
        .data;
    Ok(b)
}

async fn last_attempt(
    contract: &near_api::Contract,
    net: &near_api::NetworkConfig,
) -> testresult::TestResult<Value> {
    let attempts: Vec<Value> = contract
        .call_function("get_attempts", json!({ "from_index": 0, "limit": 100 }))
        .read_only()
        .fetch_from(net)
        .await?
        .data;
    Ok(attempts.last().cloned().expect("no attempts were logged"))
}

#[tokio::test]
async fn vault_enforces_every_rule() -> testresult::TestResult<()> {
    let wasm_path = cargo_near_build::build_with_cli(Default::default())?;
    let wasm = std::fs::read(wasm_path)?;

    let sandbox = near_sandbox::Sandbox::start_sandbox().await?;
    let net = near_api::NetworkConfig::from_rpc_url("sandbox", sandbox.rpc_addr.parse()?);

    let owner = create_subaccount(&sandbox, "owner.sandbox").await?;
    let agent = create_subaccount(&sandbox, "agent.sandbox").await?;
    let vendor = create_subaccount(&sandbox, "vendor.sandbox").await?;
    let attacker = create_subaccount(&sandbox, "attacker.sandbox").await?;
    let contract = create_subaccount(&sandbox, "vault.sandbox").await?.as_contract();

    let signer = near_api::Signer::from_secret_key(
        near_sandbox::config::DEFAULT_GENESIS_ACCOUNT_PRIVATE_KEY
            .parse()
            .unwrap(),
    )?;

    near_api::Contract::deploy(contract.account_id().clone())
        .use_code(wasm)
        .with_init_call("new", json!({ "owner": owner.account_id() }))?
        .with_signer(signer.clone())
        .send_to(&net)
        .await?
        .assert_success();

    let policy_args = json!({
        "agent": agent.account_id(),
        "max_per_tx": y(500),
        "daily_budget": y(1000),
        "allowed_recipients": [vendor.account_id()],
    });

    println!("1. only the owner can set a policy");
    call!(contract, net, signer, attacker, "set_policy", policy_args.clone()).assert_failure();
    call!(contract, net, signer, owner, "set_policy", policy_args.clone()).assert_success();

    println!("2. an allowed payment reaches the vendor");
    let vendor_before = balance(&vendor, &net).await?;
    call!(contract, net, signer, agent, "pay",
        json!({ "recipient": vendor.account_id(), "amount": y(100) })).assert_success();
    sandbox.fast_forward(5).await?;
    assert_eq!(
        balance(&vendor, &net).await?,
        vendor_before.saturating_add(NearToken::from_millinear(100)),
        "vendor should receive exactly 0.1 NEAR"
    );
    let a = last_attempt(&contract, &net).await?;
    assert_eq!(a["allowed"], json!(true));
    assert_eq!(a["reason"], json!("ok"));

    println!("3. a prompt-injected payment to an unknown recipient is blocked");
    let attacker_before = balance(&attacker, &net).await?;
    call!(contract, net, signer, agent, "pay",
        json!({ "recipient": attacker.account_id(), "amount": y(100) })).assert_success();
    sandbox.fast_forward(5).await?;
    assert_eq!(balance(&attacker, &net).await?, attacker_before, "attacker must receive nothing");
    let a = last_attempt(&contract, &net).await?;
    assert_eq!(a["allowed"], json!(false));
    assert_eq!(a["reason"], json!("recipient not allowlisted"));

    println!("4. a payment over the per-transaction cap is blocked");
    call!(contract, net, signer, agent, "pay",
        json!({ "recipient": vendor.account_id(), "amount": y(900) })).assert_success();
    let a = last_attempt(&contract, &net).await?;
    assert_eq!(a["allowed"], json!(false));
    assert_eq!(a["reason"], json!("amount exceeds per-transaction cap"));

    println!("5. the daily budget is enforced");
    call!(contract, net, signer, agent, "pay",
        json!({ "recipient": vendor.account_id(), "amount": y(500) })).assert_success();
    call!(contract, net, signer, agent, "pay",
        json!({ "recipient": vendor.account_id(), "amount": y(400) })).assert_success();
    let a = last_attempt(&contract, &net).await?;
    assert_eq!(a["allowed"], json!(true), "0.1 + 0.5 + 0.4 = exactly the 1 NEAR budget");
    call!(contract, net, signer, agent, "pay",
        json!({ "recipient": vendor.account_id(), "amount": y(100) })).assert_success();
    let a = last_attempt(&contract, &net).await?;
    assert_eq!(a["allowed"], json!(false));
    assert_eq!(a["reason"], json!("daily budget exceeded"));

    println!("6. an account with no policy cannot spend");
    call!(contract, net, signer, attacker, "pay",
        json!({ "recipient": vendor.account_id(), "amount": y(10) })).assert_success();
    let a = last_attempt(&contract, &net).await?;
    assert_eq!(a["allowed"], json!(false));
    assert_eq!(a["reason"], json!("agent has no policy"));

    println!("7. the per-agent kill switch");
    call!(contract, net, signer, attacker, "set_agent_paused",
        json!({ "agent": agent.account_id(), "paused": true })).assert_failure();
    call!(contract, net, signer, owner, "set_agent_paused",
        json!({ "agent": agent.account_id(), "paused": true })).assert_success();
    call!(contract, net, signer, agent, "pay",
        json!({ "recipient": vendor.account_id(), "amount": y(10) })).assert_success();
    let a = last_attempt(&contract, &net).await?;
    assert_eq!(a["allowed"], json!(false));
    assert_eq!(a["reason"], json!("agent is paused"));

    println!("8. the vault-wide kill switch");
    call!(contract, net, signer, owner, "set_agent_paused",
        json!({ "agent": agent.account_id(), "paused": false })).assert_success();
    call!(contract, net, signer, attacker, "set_vault_paused", json!({ "paused": true })).assert_failure();
    call!(contract, net, signer, owner, "set_vault_paused", json!({ "paused": true })).assert_success();
    call!(contract, net, signer, agent, "pay",
        json!({ "recipient": vendor.account_id(), "amount": y(10) })).assert_success();
    let a = last_attempt(&contract, &net).await?;
    assert_eq!(a["allowed"], json!(false));
    assert_eq!(a["reason"], json!("vault is paused"));

    println!("9. only the owner can withdraw");
    let before = vault_balance(&contract, &net).await?;
    call!(contract, net, signer, attacker, "withdraw", json!({ "amount": y(1000) })).assert_failure();
    call!(contract, net, signer, owner, "withdraw", json!({ "amount": y(1000) })).assert_success();
    sandbox.fast_forward(5).await?;
    let after = vault_balance(&contract, &net).await?;
    assert!(
        after <= before.saturating_sub(NearToken::from_millinear(990)),
        "the vault balance should drop by about 1 NEAR"
    );

    println!("All vault rules verified.");
    Ok(())
}
