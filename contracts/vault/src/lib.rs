use near_sdk::store::{IterableMap, Vector};
use near_sdk::{
    env, near, require, AccountId, NearToken, PanicOnDefault, Promise, PromiseOrValue,
};

const DAY_MS: u64 = 86_400_000;

/// Spending rules the owner sets for one agent account.
#[near(serializers = [json, borsh])]
#[derive(Clone)]
pub struct Policy {
    pub max_per_tx: NearToken,
    pub daily_budget: NearToken,
    pub allowed_recipients: Vec<AccountId>,
    pub paused: bool,
    pub window_start_ms: u64,
    pub spent_in_window: NearToken,
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
    paused: bool,
    policies: IterableMap<AccountId, Policy>,
    attempts: Vector<Attempt>,
}

#[near]
impl Vault {
    #[init]
    pub fn new(owner: AccountId) -> Self {
        Self {
            owner,
            paused: false,
            policies: IterableMap::new(b"p"),
            attempts: Vector::new(b"a"),
        }
    }

    // ---------- owner actions ----------

    pub fn set_policy(
        &mut self,
        agent: AccountId,
        max_per_tx: NearToken,
        daily_budget: NearToken,
        allowed_recipients: Vec<AccountId>,
    ) {
        self.assert_owner();
        self.policies.insert(
            agent,
            Policy {
                max_per_tx,
                daily_budget,
                allowed_recipients,
                paused: false,
                window_start_ms: 0,
                spent_in_window: NearToken::from_yoctonear(0),
            },
        );
    }

    pub fn remove_policy(&mut self, agent: AccountId) {
        self.assert_owner();
        self.policies.remove(&agent);
    }

    /// Kill switch for one agent.
    pub fn set_agent_paused(&mut self, agent: AccountId, paused: bool) {
        self.assert_owner();
        let policy = self.policies.get_mut(&agent);
        require!(policy.is_some(), "agent has no policy");
        policy.unwrap().paused = paused;
    }

    /// Kill switch for the whole vault.
    pub fn set_vault_paused(&mut self, paused: bool) {
        self.assert_owner();
        self.paused = paused;
    }

    /// Owner takes funds back out of the vault.
    pub fn withdraw(&mut self, amount: NearToken) -> Promise {
        self.assert_owner();
        require!(env::account_balance() >= amount, "insufficient vault balance");
        Promise::new(self.owner.clone()).transfer(amount)
    }

    // ---------- agent action ----------

    /// Called by the agent account. Pays from the vault if the policy allows it.
    /// Blocked attempts do not panic: they are logged and `false` is returned.
    pub fn pay(&mut self, recipient: AccountId, amount: NearToken) -> PromiseOrValue<bool> {
        let agent = env::predecessor_account_id();
        let now = env::block_timestamp_ms();
        let verdict = self.check(&agent, &recipient, amount, now);

        if verdict.is_ok() {
            if let Some(policy) = self.policies.get_mut(&agent) {
                if now.saturating_sub(policy.window_start_ms) >= DAY_MS {
                    policy.window_start_ms = now;
                    policy.spent_in_window = NearToken::from_yoctonear(0);
                }
                policy.spent_in_window = NearToken::from_yoctonear(
                    policy
                        .spent_in_window
                        .as_yoctonear()
                        .saturating_add(amount.as_yoctonear()),
                );
            }
        }

        self.attempts.push(Attempt {
            agent,
            recipient: recipient.clone(),
            amount,
            allowed: verdict.is_ok(),
            reason: match &verdict {
                Ok(()) => "ok".to_string(),
                Err(reason) => reason.clone(),
            },
            timestamp_ms: now,
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

    pub fn is_paused(&self) -> bool {
        self.paused
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
        now: u64,
    ) -> Result<(), String> {
        if self.paused {
            return Err("vault is paused".to_string());
        }
        let policy = self
            .policies
            .get(agent)
            .ok_or_else(|| "agent has no policy".to_string())?;
        if policy.paused {
            return Err("agent is paused".to_string());
        }
        if amount > policy.max_per_tx {
            return Err("amount exceeds per-transaction cap".to_string());
        }
        if !policy.allowed_recipients.contains(recipient) {
            return Err("recipient not allowlisted".to_string());
        }
        let spent = if now.saturating_sub(policy.window_start_ms) >= DAY_MS {
            0
        } else {
            policy.spent_in_window.as_yoctonear()
        };
        if spent.saturating_add(amount.as_yoctonear()) > policy.daily_budget.as_yoctonear() {
            return Err("daily budget exceeded".to_string());
        }
        if env::account_balance() < amount {
            return Err("insufficient vault balance".to_string());
        }
        Ok(())
    }
}
