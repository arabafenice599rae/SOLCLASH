# SOLCLASH-EVENTS

Prediction market binario Sì/No, P2P, non-custodial, parimutuale a stake
variabile su Solana, risolto leggendo un price update Pyth.

## Stato attuale: compila, test verdi, mai deployato

La Fase 0 è stata eseguita: il workspace ha manifest reali, il programma
compila, `cargo test` è verde e `anchor build` produce un `.so` che il
loader di Solana accetta. Il verdetto completo — versioni esatte, cosa è
stato verificato comando per comando, e cosa resta fuori — è in
[`TOOLCHAIN.md`](TOOLCHAIN.md).

Quello che **non** è stato verificato resta la parte più grossa:

- nessun test on-chain esercita le 11 istruzioni (Fase 2, LiteSVM/Surfpool);
- il percorso Pyth è ancora solo `oracle-mock`, senza feed id reali (Fase 3);
- ogni costante `_DEV` in `constants.rs` è un placeholder;
- gli invarianti di `SECURITY.md` sono argomentati nel codice, non eseguiti;
- nessun audit.

Il program id in `declare_id!` è ancora un placeholder senza chiave
privata, di proposito: vedi la sezione dedicata in `TOOLCHAIN.md`. **Il
programma non è deployabile e non va deployato.**

### Comandi

```sh
cargo build                    # programma, host
cargo test                     # 21 test: math.rs + i vettori di tests/fixtures/
cargo clippy --all-targets
cargo fmt --check
anchor build                   # .so + IDL + tipi TypeScript in target/
yarn install && yarn test      # 6 test: l'IDL generato contro il sorgente Rust
```

## Come leggere questo repository

| Percorso | Contenuto |
|---|---|
| `programs/solclash_events/src/` | Il programma Anchor (10 file) |
| `programs/solclash_events/src/math.rs` | Le uniche funzioni pure: normalizzazione e8, confidence band, payout/refund. Non tocca `Account`, `Context` o tipi Pyth, quindi si testa con `cargo test` e basta |
| `programs/solclash_events/src/oracle.rs` | L'unico confine Pyth del programma. Oggi solo `oracle-mock`, nessuna dipendenza Pyth reale |
| `tests/fixtures/*.json` | Vettori generati da `tests/fixtures/generate_fixtures.py` (Python puro, nessuna dipendenza) e confrontati con `math.rs` da `programs/solclash_events/tests/fixtures.rs` a ogni `cargo test` |
| `docs/pyth-reference.md` | Estrazione sourced (commit hash + `file:riga`) dal repository `pyth-network/pyth-crosschain`: layout byte di `PriceUpdateV2`, semantica di `VerificationLevel`, versione di `anchor-lang` pinnata dall'SDK Pyth |
| `TOOLCHAIN.md` | Verdetto della Fase 0: versioni esatte, cosa compila, cosa no, e perché |
| `tests/idl.ts` | Controlla che l'IDL prodotto da `anchor build` descriva ancora il programma che il sorgente Rust dichiara |
| `SECURITY.md` | Modello di trust e invarianti (I1-I13) |
| `DEPLOY.md` | Piano di deploy (Squads v4 + timelock + burn, `solana-verify`, whitelist CPI) — non ancora eseguito |
| `DEVIATIONS.md` | Ogni decisione presa dove la spec era ambigua o silente, e ogni assunzione della bozza (con l'esito della verifica in Fase 0) |

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
OPEN ──cancel_bet sempre consentito
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

Dettagli completi (istruzioni, invarianti, formula di normalizzazione
del prezzo, confidence band) in `SECURITY.md`.

## Dipendenze dirette previste

Solo tre, per esplicito vincolo di progetto (minimizzare le crate
importate, massimizzare il riuso di constraint Anchor e programmi già
deployati):

- `anchor-lang` — **presente**, versione 1.1.2, unica dipendenza on-chain
  di oggi
- `pyth-solana-receiver-sdk` — non ancora introdotto: la versione da usare
  in Fase 3 è la **2.0.0**, l'unica che pinna `anchor-lang ^1.0.2` e quindi
  convive con la 1.1.2 (vedi `TOOLCHAIN.md` e `docs/pyth-reference.md` §4)
- `solana-security-txt` — non ancora introdotto: nessun `security_txt!` è
  invocato nel sorgente, e va aggiunto insieme al blocco vero

Fuori dal binario on-chain c'è una sola dev-dependency, `serde_json`, usata
esclusivamente per rileggere `tests/fixtures/*.json` nei test.

Nessuna crate matematica: `u128` e aritmetica `checked_*` bastano per
tutta la normalizzazione di prezzo e i calcoli di payout.

## Prossimi passi

1. ~~Fase 0: verdetto sulla toolchain, `TOOLCHAIN.md` reale.~~ Fatto.
2. ~~Portare la bozza a compilare, poi verde su `cargo test` (`math.rs`
   contro `tests/fixtures/*.json`).~~ Fatto.
3. Fase 2: i test LiteSVM elencati nella spec, che esercitino davvero le 11
   istruzioni. Serve prima un program id con una chiave vera
   (`anchor keys sync`) — vedi `TOOLCHAIN.md`.
4. Fase 3: `oracle.rs` reale contro Pyth (`pyth-solana-receiver-sdk 2.0.0`),
   feed id reali, fixture da devnet.
5. Sostituire ogni costante `_DEV` in `constants.rs` con un valore reale
   e ragionato prima di qualunque build con la feature `mainnet`.
