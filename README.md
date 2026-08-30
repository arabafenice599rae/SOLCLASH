# SOLCLASH-EVENTS

Prediction market binario Sì/No, P2P, non-custodial, parimutuale a stake
variabile su Solana, risolto leggendo un price update Pyth.

## Stato attuale: bozza offline, mai compilata

Questo branch (`wip/offline-draft`) contiene il disegno della Fase 1 della
spec, scritto **senza un toolchain Solana/Anchor funzionante** in questo
ambiente (crates.io, npmjs.org e `mcp.solana.com` non sono raggiungibili
qui — vedi la sonda d'ambiente nella cronologia di questa sessione).
**Ogni file `.rs` in questo repository apre con un header
`STATUS: NEVER COMPILED`.** Niente qui è stato verificato da un
compilatore, un test, o un audit. Non deployare, non fidarsi di nessun
valore numerico senza prima rileggere `constants.rs` e `DEVIATIONS.md`.

La Fase 0 (verdetto sulla toolchain: Anchor/Solana/pin dell'SDK Pyth) è
demandata all'utente, in locale — non è presente in questo repository e
non deve esserlo finché non è stata davvero eseguita.

## Come leggere questo repository

| Percorso | Contenuto |
|---|---|
| `programs/solclash_events/src/` | Bozza del programma Anchor (10 file, mai compilati) |
| `programs/solclash_events/src/math.rs` | Le uniche funzioni pure: normalizzazione e8, confidence band, payout/refund. L'unico file testabile con `cargo test` puro, senza toolchain Solana |
| `programs/solclash_events/src/oracle.rs` | L'unico confine Pyth del programma. Fase 1: solo `oracle-mock`, nessuna dipendenza Pyth reale |
| `tests/fixtures/*.json` | Vettori di test generati da `tests/fixtures/generate_fixtures.py` (Python puro, nessuna dipendenza), pensati per essere confrontati con l'output di `cargo test` su `math.rs` una volta che un toolchain esiste |
| `docs/pyth-reference.md` | Estrazione sourced (commit hash + `file:riga`) dal repository `pyth-network/pyth-crosschain`: layout byte di `PriceUpdateV2`, semantica di `VerificationLevel`, versione di `anchor-lang` pinnata dall'SDK Pyth |
| `SECURITY.md` | Modello di trust e invarianti (I1-I13) |
| `DEPLOY.md` | Piano di deploy (Squads v4 + timelock + burn, `solana-verify`, whitelist CPI) — non ancora eseguito |
| `DEVIATIONS.md` | Ogni decisione presa dove la spec era ambigua o silente, e ogni assunzione non verificabile in questo ambiente |

## Modello economico (riassunto)

```
pot = Σ stake_i

Alla finalizzazione:
  RESOLVER_REWARD                 → al primo risolutore
  protocol_fee = 10% del residuo  → FEE_WALLET   (solo se esito YES/NO)
  payout_pool                     → ai partecipanti

claim_i  = payout_pool × stake_i / winning_stake   (u128, floor)
refund_i = payout_pool × stake_i / pot             (u128, floor)
```

Il floor garantisce `Σ pagamenti ≤ payout_pool`: l'ultimo a reclamare non
può mai fallire per mancanza di fondi. Il resto lo raccoglie `close_event`.

## Macchina a stati

```
OPEN ──cancel_bet consentito solo prima di betting_close_time
 │ betting_close_time → lock_event() permissionless
 ▼
LOCKED
 ├── book monolaterale → REFUNDABLE (payout_pool = pot, rimborso 100%)
 │ resolution_time → resolve_event() + Pyth Full
 ▼
RESOLVING ◄── challenge_resolution() (solo publish_time MAGGIORE, e <= resolution_time)
 │ finalized_at IMMUTABILE → finalize_resolution()
 ├── candidato ambiguo → REFUNDABLE
 └── YES/NO → RESOLVED
 ▼
claim × N / claim_refund × N → close_event

LOCKED oltre resolution_time + RESOLUTION_TIMEOUT_SECS senza candidato → REFUNDABLE
```

Due precisazioni rispetto alla spec originale (motivate dal security
review del 2026-08-30, dettagli in `DEVIATIONS.md`):

- **`cancel_bet`** è consentito in OPEN **e prima di `betting_close_time`**.
  È sicuro perché prima della chiusura nessuno ha informazione sul prezzo
  di risoluzione. Il controllo sul solo `status == Open` non basta:
  `lock_event` è un crank permissionless e la finestra fra chiusura e lock
  può estendersi indefinitamente.
- **`claim`** non è "il vincitore incassa": è "il partecipante chiude la
  propria posizione, incassando se ha vinto". Su un evento RESOLVED anche
  i perdenti chiamano `claim` — la loro quota è zero, il `BetEntry` si
  chiude comunque restituendo il rent, e `close_event` resta raggiungibile.

Dettagli completi (istruzioni, invarianti, formula di normalizzazione
del prezzo, confidence band) in `SECURITY.md`.

## Dipendenze dirette previste

Solo tre, per esplicito vincolo di progetto (minimizzare le crate
importate, massimizzare il riuso di constraint Anchor e programmi già
deployati):

- `anchor-lang`
- `pyth-solana-receiver-sdk` (se il ramo A della Fase 0 passa — vedi
  `docs/pyth-reference.md` §4 per il pin di `anchor-lang` che questo SDK
  porta con sé)
- `solana-security-txt`

Nessuna crate matematica: `u128` e aritmetica `checked_*` bastano per
tutta la normalizzazione di prezzo e i calcoli di payout.

## Prossimi passi

1. Fase 0 in locale (utente): verdetto sulla toolchain, `TOOLCHAIN.md` reale.
2. Portare questa bozza a compilare, poi verde su `cargo test` (`math.rs`
   contro `tests/fixtures/*.json`) e sui test LiteSVM elencati nella spec
   (Fase 2).
3. Fase 3: `oracle.rs` reale contro Pyth, feed id reali, fixture da devnet.
4. Sostituire ogni costante `_DEV` in `constants.rs` con un valore reale
   e ragionato prima di qualunque build con la feature `mainnet`.
