# SEC-015: Freighter Wallet Integration — XSS Review

| Field | Value |
|---|---|
| Record type | Security review |
| Date | 2026-10-21 |
| Status | Resolved |
| Severity | Low (defence-in-depth) |
| Affected components | `frontend/src/components/FreighterWallet.tsx`, `frontend/src/utils/stellarAddress.ts` |
| Issue | #319 |

## 1. Scope

`FreighterWallet.tsx` renders two pieces of data sourced from the Freighter
browser extension, which must be treated as an untrusted third party:

- the connected wallet's public key (`freighter.getPublicKey()`)
- the connected network name (`freighter.getNetwork()`)

## 2. Findings

1. **Rendering path is already safe from classic DOM-XSS.** All wallet data
   is rendered through plain React JSX text nodes and the `title` attribute
   (`<span title={wallet.address}>{truncate(wallet.address)}</span>`).
   `dangerouslySetInnerHTML` is not used anywhere in this component. React
   HTML-escapes text nodes and attribute values, so a string such as
   `<img src=x onerror=alert(1)>` returned by a malicious/buggy extension is
   rendered as literal text, not executed. Grepped the file and confirmed no
   `dangerouslySetInnerHTML`, `innerHTML`, or `eval` usage.
2. **Gap (now fixed):** before this change, `truncate()` performed no
   validation on the address string at all — any value returned by
   `getPublicKey()` would be truncated and displayed as if it were a valid
   G-address, and would also be accepted into `connect()`'s component state.
   This is not an XSS vector given finding 1, but it is a spoofing /
   confusing-UI risk (e.g., a compromised or fake "Freighter" object
   injected by another extension could report an address that isn't a real
   Stellar key, or an overlong/control-character string that breaks the
   truncation display).
3. **Pre-existing bug found during review:** the mismatch-dismiss button
   referenced `mismatchDismissed` / `setMismatchDismissed` that were never
   declared as component state, which would throw at render time once a
   network mismatch occurred. Fixed alongside this review by adding the
   missing `useState` declaration.

## 3. Remediation

- `truncate()` now calls `isValidStellarAddress()` from
  `frontend/src/utils/stellarAddress.ts` and returns a safe
  `"invalid-address"` placeholder for anything that is not a structurally
  valid, checksum-correct Stellar G-address.
- `connect()` validates the address returned by `getPublicKey()` with the
  same utility and rejects the connection (surfacing an error) instead of
  adopting a non-standard address into wallet state.
- Added the missing `mismatchDismissed` state declaration.
- Added regression tests in `FreighterWallet.test.tsx` covering: a crafted
  non-address string returned by `getPublicKey()` during `connect()` is
  rejected and never rendered, and a stale/auto-restored session carrying a
  non-standard address is displayed as `"invalid-address"` rather than the
  raw string.

## 4. Verdict

No exploitable XSS was found — React's JSX escaping already prevents script
execution via text/attribute rendering. The fix in this issue is
defence-in-depth per the acceptance criteria: wallet data is validated with
`stellarAddress.ts` before display, closing the spoofing/confusing-UI gap
and the unrelated `mismatchDismissed` runtime bug found during the review.
