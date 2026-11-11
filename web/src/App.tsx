import OwnerControls from "./OwnerControls"
import { useCallback, useEffect, useState, type ReactNode } from 'react'
import {
  AGENT_ID,
  VAULT_ID,
  fetchVaultState,
  formatNear,
  spentToday,
  type VaultState,
} from './vault'

const REFRESH_MS = 5000

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-baseline justify-between gap-4 border-b border-line py-3 last:border-b-0">
      <dt className="text-sm text-muted">{label}</dt>
      <dd className="text-right text-sm font-semibold">{children}</dd>
    </div>
  )
}

function formatTime(ms: number) {
  return new Date(ms).toLocaleString([], {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

export default function App() {
  const [state, setState] = useState<VaultState | null>(null)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(async () => {
    try {
      setState(await fetchVaultState())
      setError(null)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Could not reach the NEAR testnet')
    }
  }, [])

  useEffect(() => {
    load()
    const id = setInterval(load, REFRESH_MS)
    return () => clearInterval(id)
  }, [load])

  const policy = state?.policy ?? null
  const paused = state ? state.vaultPaused || !!policy?.paused : false
  const budget = policy ? BigInt(policy.daily_budget) : 0n
  const spent = policy ? spentToday(policy) : 0n
  const remaining = budget > spent ? budget - spent : 0n
  const pct = budget > 0n ? Math.min(100, Number((spent * 10000n) / budget) / 100) : 0
  const barColor = pct >= 100 ? 'bg-bad' : pct >= 80 ? 'bg-warn' : 'bg-brand'

  let pill = { text: 'Loading', cls: 'bg-tint text-muted', dot: 'bg-muted' }
  if (state && !policy) {
    pill = { text: 'No policy set', cls: 'bg-warn-soft text-warn', dot: 'bg-warn' }
  } else if (state && paused) {
    pill = { text: 'Agent paused', cls: 'bg-bad-soft text-bad', dot: 'bg-bad' }
  } else if (state) {
    pill = { text: 'Agent active', cls: 'bg-ok-soft text-ok', dot: 'bg-ok' }
  }

  return (
    <div className="min-h-screen bg-white text-ink">
      <header className="border-b border-line">
        <div className="mx-auto flex max-w-5xl items-center justify-between gap-4 px-6 py-5">
          <div className="flex items-center gap-3">
            <img src="/favicon.svg" alt="Aspvault logo" className="h-10 w-10" />
            <div>
              <p className="text-lg font-extrabold leading-none tracking-tight">Aspvault</p>
              <p className="mt-1 text-xs text-muted">A budget for your AI agent, not your wallet</p>
            </div>
          </div>
          <div className="flex items-center gap-3 text-xs text-muted">
            <span className="rounded-full border border-line px-3 py-1">NEAR testnet</span>
            <span className="hidden sm:inline">
              {state ? `Updated ${new Date(state.fetchedAt).toLocaleTimeString()}` : 'Loading…'}
            </span>
          </div>
        </div>
      </header>

      <main className="mx-auto max-w-5xl px-6 py-10">
        {error && (
          <p className="mb-6 rounded-lg bg-bad-soft px-4 py-3 text-sm text-bad">
            Could not reach the network: {error}. Trying again automatically.
          </p>
        )}

        <section className="grid gap-8 rounded-2xl border border-line bg-tint p-6 sm:p-8 lg:grid-cols-[1.4fr_1fr]">
          <div>
            <span className={`inline-flex items-center gap-2 rounded-full px-3 py-1 text-xs font-semibold ${pill.cls}`}>
              <span className={`h-2 w-2 rounded-full ${pill.dot}`} />
              {pill.text}
            </span>
            <p className="mt-6 text-sm text-muted">Left to spend today</p>
            <p className="mt-1 text-5xl font-extrabold tracking-tight sm:text-6xl">
              {policy ? formatNear(remaining.toString()) : '–'}
              <span className="ml-2 text-xl font-semibold text-muted">NEAR</span>
            </p>
            <div className="mt-6 h-3 overflow-hidden rounded-full border border-line bg-white">
              <div className={`h-full rounded-full transition-all duration-500 ${barColor}`} style={{ width: `${pct}%` }} />
            </div>
            <p className="mt-3 text-sm text-muted">
              {policy
                ? `${formatNear(spent.toString())} of ${formatNear(policy.daily_budget)} NEAR spent in the last 24 hours.`
                : 'Set a policy to let the agent spend.'}
            </p>
          </div>

          <dl className="self-center">
            <Row label="Vault balance">{state ? `${formatNear(state.balance, 2)} NEAR` : '–'}</Row>
            <Row label="Limit per payment">{policy ? `${formatNear(policy.max_per_tx)} NEAR` : '–'}</Row>
            <Row label="Daily budget">{policy ? `${formatNear(policy.daily_budget)} NEAR` : '–'}</Row>
            <Row label="Agent account">
              <span className="break-all font-mono text-xs font-medium">{AGENT_ID}</span>
            </Row>
            <Row label="Allowed recipients">
              {policy && policy.allowed_recipients.length > 0 ? (
                <span className="flex flex-col items-end gap-1">
                  {policy.allowed_recipients.map((r) => (
                    <span key={r} className="whitespace-nowrap rounded-md bg-brand-soft px-2 py-1 font-mono text-xs font-medium text-brand">
                      {r}
                    </span>
                  ))}
                </span>
              ) : (
                'None'
              )}
            </Row>
          </dl>
        </section>

        <OwnerControls state={state} onChanged={load} />

        <section className="mt-12">
          <h2 className="text-xl font-bold tracking-tight">Payment feed</h2>
          <p className="mt-1 text-sm text-muted">
            Every payment the agent attempts, recorded on-chain by the vault. Newest first.
          </p>
          <div className="mt-5 overflow-x-auto rounded-2xl border border-line">
            <table className="w-full text-left text-sm">
              <thead className="bg-tint text-xs text-muted">
                <tr>
                  <th className="px-5 py-3 font-semibold">Time</th>
                  <th className="px-5 py-3 font-semibold">Recipient</th>
                  <th className="px-5 py-3 font-semibold">Amount</th>
                  <th className="px-5 py-3 font-semibold">Result</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-line">
                {state && state.attempts.length === 0 && (
                  <tr>
                    <td colSpan={4} className="px-5 py-10 text-center text-muted">
                      No payments yet. Run the demo agent to see activity here.
                    </td>
                  </tr>
                )}
                {state?.attempts.map((a, i) => (
                  <tr key={`${a.timestamp_ms}-${i}`} className={a.allowed ? '' : 'bg-bad-soft/40'}>
                    <td className="whitespace-nowrap px-5 py-3.5 text-muted">{formatTime(a.timestamp_ms)}</td>
                    <td className="px-5 py-3.5 font-mono text-xs">{a.recipient}</td>
                    <td className="whitespace-nowrap px-5 py-3.5 font-semibold">{formatNear(a.amount)} NEAR</td>
                    <td className="px-5 py-3.5">
                      <span
                        className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-semibold ${
                          a.allowed ? 'bg-ok-soft text-ok' : 'bg-bad-soft text-bad'
                        }`}
                      >
                        <span className={`h-1.5 w-1.5 rounded-full ${a.allowed ? 'bg-ok' : 'bg-bad'}`} />
                        {a.allowed ? 'Allowed' : 'Blocked'}
                      </span>
                      {!a.allowed && <span className="ml-2 text-xs text-muted">{a.reason}</span>}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        <footer className="mt-12 border-t border-line pt-6 text-xs text-muted">
          Vault contract{' '}
          <a
            className="font-mono text-brand underline underline-offset-2"
            href={`https://explorer.testnet.near.org/accounts/${VAULT_ID}`}
            target="_blank"
            rel="noreferrer"
          >
            {VAULT_ID}
          </a>
        </footer>
      </main>
    </div>
  )
}
