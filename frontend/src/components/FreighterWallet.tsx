import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { isValidStellarAddress } from "../utils/stellarAddress";

export const STELLAR_NETWORK: "TESTNET" | "MAINNET" =
  (import.meta.env.VITE_STELLAR_NETWORK ?? "TESTNET").toUpperCase() === "MAINNET"
    ? "MAINNET"
    : "TESTNET";
const FREIGHTER_DOWNLOAD = "https://www.freighter.app/";
/** Button stays disabled for this long after every click (debounce). */
export const CONNECT_DEBOUNCE_MS = 2000;
/** Consecutive failures before a cooldown is enforced. */
export const MAX_FAILED_ATTEMPTS = 3;
/** Cooldown applied after MAX_FAILED_ATTEMPTS consecutive failures. */
export const COOLDOWN_MS = 30000;

type FreighterApi = {
  isConnected: () => Promise<boolean>;
  getPublicKey: () => Promise<string>;
  getNetwork: () => Promise<string>;
  requestAccess: () => Promise<void>;
};

type WalletState = {
  address: string | null;
  network: string | null;
  connected: boolean;
};

/**
 * SEC (#319): Freighter is a third-party browser extension — its return
 * values must be treated as untrusted input. `getPublicKey()` is expected
 * to return a 56-character Stellar G-address, but a malicious or buggy
 * extension could return an arbitrary string. We validate with
 * `isValidStellarAddress` (frontend/src/utils/stellarAddress.ts) before
 * ever displaying it, falling back to a safe placeholder for anything
 * that is not a structurally valid address. React JSX text nodes are
 * HTML-escaped regardless, so this does not change the XSS surface, but
 * it prevents a crafted non-standard string (overlong, control
 * characters, homoglyphs, etc.) from being rendered as if it were a
 * trustworthy wallet address.
 */
function truncate(addr: string) {
  if (!isValidStellarAddress(addr)) {
    return "invalid-address";
  }
  return `${addr.slice(0, 6)}...${addr.slice(-4)}`;
}

export function FreighterWallet() {
  const { t } = useTranslation();
  const [wallet, setWallet] = useState<WalletState>({
    address: null,
    network: null,
    connected: false,
  });
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [throttled, setThrottled] = useState(false);
  const [cooldownUntil, setCooldownUntil] = useState<number | null>(null);
  const failedAttempts = useRef(0);
  const inFlight = useRef(false);
  const [mismatchDismissed, setMismatchDismissed] = useState(false);

  // Clear the cooldown once it expires.
  useEffect(() => {
    if (cooldownUntil === null) return;
    const id = setTimeout(() => {
      setCooldownUntil(null);
      failedAttempts.current = 0;
    }, Math.max(0, cooldownUntil - Date.now()));
    return () => clearTimeout(id);
  }, [cooldownUntil]);

  // Check if already connected on mount
  useEffect(() => {
    const freighter = (window as unknown as Record<string, unknown>).freighter as FreighterApi | undefined;
    if (!freighter) return;
    freighter.isConnected().then((connected: boolean) => {
      if (connected) {
        freighter.getPublicKey().then((address: string) => {
          freighter.getNetwork().then((network: string) => {
            setWallet({ address, network, connected: true });
            setMismatchDismissed(false);
          });
        });
      }
    });
  }, []);

  async function connect() {
    // Rate limiting: ignore clicks while a request is in flight, during the
    // debounce window, or during a failure cooldown. Never retried automatically.
    if (inFlight.current || throttled || cooldownUntil !== null) return;
    setThrottled(true);
    setTimeout(() => setThrottled(false), CONNECT_DEBOUNCE_MS);

    const freighter = (window as unknown as Record<string, unknown>).freighter as FreighterApi | undefined;
    if (!freighter) {
      setError(t("wallet.notFound"));
      return;
    }
    inFlight.current = true;
    setLoading(true);
    setError(null);
    try {
      await freighter.requestAccess();
      const address: string = await freighter.getPublicKey();
      const network: string = await freighter.getNetwork();
      // SEC (#319): reject a non-standard address from the extension up
      // front rather than storing/rendering it.
      if (!isValidStellarAddress(address)) {
        throw new Error(t("wallet.failed"));
      }
      setWallet({ address, network, connected: true });
      failedAttempts.current = 0;
    } catch (e: unknown) {
      setError((e as { message?: string })?.message ?? t("wallet.failed"));
      failedAttempts.current += 1;
      if (failedAttempts.current >= MAX_FAILED_ATTEMPTS) {
        setCooldownUntil(Date.now() + COOLDOWN_MS);
      }
    } finally {
      inFlight.current = false;
      setLoading(false);
    }
  }

  function disconnect() {
    setWallet({ address: null, network: null, connected: false });
    setError(null);
  }

  const walletNetwork = wallet.network?.toUpperCase() ?? null;
  const networkMismatch =
    wallet.connected && walletNetwork !== null && walletNetwork !== STELLAR_NETWORK;
  const showMismatch = networkMismatch && !mismatchDismissed;

  return (
    <div style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
      {!wallet.connected ? (
        <button
          onClick={connect}
          disabled={loading || throttled || cooldownUntil !== null}
          aria-busy={loading}
          aria-label={t("wallet.connectLabel")}
        >
          {loading ? t("wallet.connecting") : t("wallet.connect")}
        </button>
      ) : (
        <>
          <span title={wallet.address ?? ""} aria-label="Connected wallet address">
            {truncate(wallet.address!)}
          </span>
          <button onClick={disconnect} aria-label="Disconnect wallet">
            {t("wallet.disconnect")}
          </button>
        </>
      )}

      {showMismatch && (
        <span
          role="alert"
          data-testid="network-mismatch"
          style={{
            color: STELLAR_NETWORK === "MAINNET" ? "red" : "orange",
            fontWeight: STELLAR_NETWORK === "MAINNET" ? "bold" : undefined,
          }}
        >
          {STELLAR_NETWORK === "MAINNET"
            ? t("wallet.networkMismatchMainnet", { network: wallet.network })
            : walletNetwork === "PUBLIC" || walletNetwork === "MAINNET"
              ? t("wallet.networkMismatchTestnet", { network: wallet.network })
              : t("wallet.networkMismatch", { network: wallet.network, expected: STELLAR_NETWORK })}{" "}
          <button onClick={() => setMismatchDismissed(true)} aria-label={t("wallet.dismissWarning")}>
            ×
          </button>
        </span>
      )}

      {cooldownUntil !== null && (
        <span role="status" style={{ color: "orange" }}>
          {t("wallet.cooldown", { seconds: Math.ceil(COOLDOWN_MS / 1000) })}
        </span>
      )}

      {error && (
        <span role="alert" style={{ color: "red" }}>
          {error}{" "}
          {error.includes("not found") && (
            <a href={FREIGHTER_DOWNLOAD} target="_blank" rel="noreferrer">
              {t("wallet.install")}
            </a>
          )}
        </span>
      )}
    </div>
  );
}
