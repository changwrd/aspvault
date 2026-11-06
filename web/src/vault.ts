import { FailoverRpcProvider, JsonRpcProvider } from 'near-api-js'

export const VAULT_ID = 'vault.allowance-owner-hp.testnet'
export const OWNER_ID = 'allowance-owner-hp.testnet'
export const AGENT_ID = 'agent.allowance-owner-hp.testnet'

export type Attempt = {
  agent: string
  recipient: string
  amount: string
  allowed: boolean
  reason: string
  timestamp_ms: number
}

export type Policy = {
  max_per_tx: string
  daily_budget: string
  allowed_recipients: string[]
  paused: boolean
  window_start_ms: number
  spent_in_window: string
}

export type VaultState = {
  balance: string
  vaultPaused: boolean
  policy: Policy | null
  attempts: Attempt[]
  attemptsCount: number
  fetchedAt: number
}

const provider = new FailoverRpcProvider([
  new JsonRpcProvider({ url: 'https://test.rpc.fastnear.com' }),
  new JsonRpcProvider({ url: 'https://rpc.testnet.near.org' }),
  new JsonRpcProvider({ url: 'https://archival-rpc.testnet.fastnear.com' }),
])

async function view<T>(method: string, args: Record<string, unknown> = {}): Promise<T> {
  return (await provider.callFunction({
    contractId: VAULT_ID,
    method,
    args,
  })) as unknown as T
}

export async function fetchVaultState(): Promise<VaultState> {
  const attemptsCount = await view<number>('get_attempts_count')
  const from_index = Math.max(0, attemptsCount - 50)
  const [balance, vaultPaused, policy, attempts] = await Promise.all([
    view<string>('get_balance'),
    view<boolean>('is_paused'),
    view<Policy | null>('get_policy', { agent: AGENT_ID }),
    view<Attempt[]>('get_attempts', { from_index, limit: 50 }),
  ])
  return {
    balance,
    vaultPaused,
    policy,
    attempts: [...attempts].reverse(),
    attemptsCount,
    fetchedAt: Date.now(),
  }
}

const YOCTO = 10n ** 24n

/** Format a yoctoNEAR amount as a short NEAR string, e.g. "0.1" or "4.85". */
export function formatNear(yocto: string, decimals = 3): string {
  const value = BigInt(yocto)
  const whole = value / YOCTO
  const frac = (value % YOCTO).toString().padStart(24, '0').slice(0, decimals)
  const trimmed = frac.replace(/0+$/, '')
  return trimmed ? `${whole}.${trimmed}` : `${whole}`
}

export const DAY_MS = 86_400_000

/** How much of today's budget the agent has used, based on the stored window. */
export function spentToday(policy: Policy, now = Date.now()): bigint {
  if (now - policy.window_start_ms >= DAY_MS) return 0n
  return BigInt(policy.spent_in_window)
}
