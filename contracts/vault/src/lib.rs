use near_sdk::store::{IterableMap, Vector};
use near_sdk::{
    env, near, require, AccountId, NearToken, PanicOnDefault, Promise, PromiseOrValue,
};

/// Spending rules the owner sets for one agent account.
#[near(serializers = [json, borsh])]
#[derive(Clone)]
pub struct Policy {
    pub max_per_tx: NearToken,
    pub allowed_recipients: Vec<AccountId>,
}

/// One payment attempt by an agent, allowed or blocked.
#[near(serializers = [json, borsh])]
#[derive(Clone)]
pub struct Attempt {
    pub agent: AccountId,
    pub recipient: AccountId,
    pub amount: NearToken,
    pub allowed: bool,
    pub reason: String,
    pub timestamp_ms: u64,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Vault {
    owner: AccountId,
    policies: IterableMap<AccountId, Policy>,
    attempts: Vector<Attempt>,
}

#[near]
impl Vault {
    #[init]
    pub fn new(owner: AccountId) -> Self {
        Self {
            owner,
            policies: IterableMap::new(b"p"),
            attempts: Vector::new(b"a"),
        }
    }

    // ---------- owner actions ----------

    pub fn set_policy(
        &mut self,
        agent: AccountId,
        max_per_tx: NearToken,
        allowed_recipients: Vec<AccountId>,
    ) {
        self.assert_owner();
        self.policies.insert(
            agent,
            Policy {
                max_per_tx,
                allowed_recipients,
            },
        );
    }

    pub fn remove_policy(&mut self, agent: AccountId) {
        self.assert_owner();
        self.policies.remove(&agent);
    }

    // ---------- agent action ----------

    /// Called by the agent account. Pays from the vault if the policy allows it.
    /// Blocked attempts do not panic: they are logged and `false` is returned.
    pub fn pay(&mut self, recipient: AccountId, amount: NearToken) -> PromiseOrValue<bool> {
        let agent = env::predecessor_account_id();
        let verdict = self.check(&agent, &recipient, amount);

        self.attempts.push(Attempt {
            agent,
            recipient: recipient.clone(),
            amount,
            allowed: verdict.is_ok(),
            reason: verdict.clone().err().unwrap_or_else(|| "ok".to_string()),
            timestamp_ms: env::block_timestamp_ms(),
        });

        match verdict {
            Ok(()) => PromiseOrValue::Promise(Promise::new(recipient).transfer(amount)),
            Err(_) => PromiseOrValue::Value(false),
        }
    }

    // ---------- views ----------

    pub fn get_owner(&self) -> AccountId {
        self.owner.clone()
    }

    pub fn get_balance(&self) -> NearToken {
        env::account_balance()
    }

    pub fn get_policy(&self, agent: AccountId) -> Option<Policy> {
        self.policies.get(&agent).cloned()
    }

    pub fn get_attempts_count(&self) -> u32 {
        self.attempts.len()
    }

    pub fn get_attempts(&self, from_index: Option<u32>, limit: Option<u32>) -> Vec<Attempt> {
        let from = from_index.unwrap_or(0) as usize;
        let limit = limit.unwrap_or(50) as usize;
        self.attempts.iter().skip(from).take(limit).cloned().collect()
    }
}

impl Vault {
    fn assert_owner(&self) {
        require!(
            env::predecessor_account_id() == self.owner,
            "Only the owner can do this"
        );
    }

    fn check(
        &self,
        agent: &AccountId,
        recipient: &AccountId,
        amount: NearToken,
    ) -> Result<(), String> {
        let policy = self
            .policies
            .get(agent)
            .ok_or_else(|| "agent has no policy".to_string())?;
        if amount > policy.max_per_tx {
            return Err("amount exceeds per-transaction cap".to_string());
        }
        if !policy.allowed_recipients.contains(recipient) {
            return Err("recipient not allowlisted".to_string());
        }
        if env::account_balance() < amount {
            return Err("insufficient vault balance".to_string());
        }
        Ok(())
    }
}
