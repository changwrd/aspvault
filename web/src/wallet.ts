import { NearConnector } from '@hot-labs/near-connect'
import { VAULT_ID } from './vault'

export type Wallet = Awaited<ReturnType<NearConnector['connect']>>

export const connector = new NearConnector({ network: 'testnet' })

const GAS = '30000000000000' // 30 Tgas

/** Convert a decimal NEAR string like "1.5" to yoctoNEAR, or null if invalid. */
export function nearToYocto(input: string): string | null {
  const s = input.trim()
  if (!/^\d+(\.\d{1,24})?$/.test(s)) return null
  const [whole, frac = ''] = s.split('.')
  const yocto = BigInt(whole) * 10n ** 24n + BigInt(frac.padEnd(24, '0'))
  return yocto > 0n ? yocto.toString() : null
}

export function callVault(
  wallet: Wallet,
  accountId: string,
  methodName: string,
  args: Record<string, unknown>,
) {
  return wallet.signAndSendTransaction({
    network: 'testnet',
    signerId: accountId,
    receiverId: VAULT_ID,
    actions: [{ type: 'FunctionCall', params: { methodName, args, gas: GAS, deposit: '0' } }],
  })
}

export function fundVault(wallet: Wallet, accountId: string, yocto: string) {
  return wallet.signAndSendTransaction({
    network: 'testnet',
    signerId: accountId,
    receiverId: VAULT_ID,
    actions: [{ type: 'Transfer', params: { deposit: yocto } }],
  })
}
