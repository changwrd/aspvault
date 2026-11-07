import { useEffect, useState } from 'react'
import { AGENT_ID, OWNER_ID, type VaultState } from './vault'
import { callVault, connector, fundVault, nearToYocto, type Wallet } from './wallet'

type Props = { state: VaultState | null; onChanged: () => Promise<void> }
type Message = { kind: 'ok' | 'err'; text: string }

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))
const btn = 'rounded-lg px-4 py-2.5 text-sm font-semibold transition disabled:cursor-not-allowed disabled:opacity-50'

export default function OwnerControls({ state, onChanged }: Props) {
  const [wallet, setWallet] = useState<Wallet | null>(null)
  const [accountId, setAccountId] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState<Message | null>(null)
  const [fundAmount, setFundAmount] = useState('1')

  useEffect(() => {
    let alive = true
    connector
      .getConnectedWallet()
      .then(({ wallet, accounts }) => {
        if (alive && accounts[0]) {
          setWallet(wallet)
          setAccountId(accounts[0].accountId)
        }
      })
      .catch(() => {})
    const onIn = (p: { wallet: Wallet; accounts: { accountId: string }[] }) => {
      if (p.accounts[0]) {
        setWallet(p.wallet)
        setAccountId(p.accounts[0].accountId)
      }
    }
    const onOut = () => {
      setWallet(null)
      setAccountId(null)
    }
    connector.on('wallet:signIn', onIn)
    connector.on('wallet:signOut', onOut)
    return () => {
      alive = false
      connector.off('wallet:signIn', onIn)
      connector.off('wallet:signOut', onOut)
    }
  }, [])

  async function run(label: string, fn: () => Promise<unknown>) {
    setBusy(true)
    setMessage(null)
    try {
      await fn()
      await sleep(1500)
      await onChanged()
      setMessage({ kind: 'ok', text: `${label}: done.` })
    } catch (e) {
      setMessage({ kind: 'err', text: e instanceof Error ? e.message : 'The wallet did not complete the request.' })
    } finally {
      setBusy(false)
    }
  }

  const isOwner = accountId === OWNER_ID
  const agentPaused = !!state?.policy?.paused

  return (
    <section className="mt-10 rounded-2xl border border-line p-6 sm:p-8">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h2 className="text-xl font-bold tracking-tight">Owner controls</h2>
          <p className="mt-1 max-w-xl text-sm text-muted">
            Anyone can view this dashboard. Only the vault owner can pause the agent or add funds.
          </p>
        </div>
        {accountId ? (
          <div className="text-right text-xs text-muted">
            <p>Connected as</p>
            <p className="font-mono text-sm font-medium text-ink">{accountId}</p>
            <button
              className="mt-1 text-brand underline underline-offset-2"
              onClick={() => connector.disconnect().catch(() => {})}
            >
              Disconnect
            </button>
          </div>
        ) : (
          <button
            className={`${btn} bg-brand text-white hover:opacity-90`}
            onClick={() => connector.connect().catch((e) => setMessage({ kind: 'err', text: String(e?.message ?? e) }))}
          >
            Connect wallet
          </button>
        )}
      </div>

      {accountId && !isOwner && (
        <p className="mt-5 rounded-lg bg-warn-soft px-4 py-3 text-sm text-warn">
          This account is not the vault owner. Connect <span className="font-mono">{OWNER_ID}</span> to make changes.
        </p>
      )}

      {accountId && isOwner && wallet && (
        <div className="mt-6 grid gap-6 sm:grid-cols-2">
          <div className="rounded-xl bg-tint p-5">
            <p className="text-sm font-semibold">Kill switch</p>
            <p className="mt-1 text-sm text-muted">
              {agentPaused
                ? 'The agent is paused. Every payment it tries will be blocked.'
                : 'The agent is active. Pause it to block every payment immediately.'}
            </p>
            <button
              disabled={busy || !state?.policy}
              className={`${btn} mt-4 text-white hover:opacity-90 ${agentPaused ? 'bg-ok' : 'bg-bad'}`}
              onClick={() =>
                run(agentPaused ? 'Agent resumed' : 'Agent paused', () =>
                  callVault(wallet, accountId, 'set_agent_paused', { agent: AGENT_ID, paused: !agentPaused }),
                )
              }
            >
              {busy ? 'Waiting for wallet…' : agentPaused ? 'Resume agent' : 'Pause agent'}
            </button>
          </div>

          <div className="rounded-xl bg-tint p-5">
            <p className="text-sm font-semibold">Fund the vault</p>
            <p className="mt-1 text-sm text-muted">Add NEAR from your wallet for the agent to spend within its limits.</p>
            <div className="mt-4 flex gap-2">
              <input
                value={fundAmount}
                onChange={(e) => setFundAmount(e.target.value)}
                inputMode="decimal"
                className="w-28 rounded-lg border border-line bg-white px-3 py-2.5 text-sm"
                aria-label="Amount in NEAR"
              />
              <button
                disabled={busy}
                className={`${btn} bg-brand text-white hover:opacity-90`}
                onClick={() => {
                  const yocto = nearToYocto(fundAmount)
                  if (!yocto) {
                    setMessage({ kind: 'err', text: 'Enter a valid amount of NEAR, for example 1 or 0.5.' })
                    return
                  }
                  run(`Added ${fundAmount} NEAR`, () => fundVault(wallet, accountId, yocto))
                }}
              >
                Add NEAR
              </button>
            </div>
          </div>
        </div>
      )}

      {message && (
        <p
          className={`mt-5 rounded-lg px-4 py-3 text-sm ${
            message.kind === 'ok' ? 'bg-ok-soft text-ok' : 'bg-bad-soft text-bad'
          }`}
        >
          {message.text}
        </p>
      )}
    </section>
  )
}
