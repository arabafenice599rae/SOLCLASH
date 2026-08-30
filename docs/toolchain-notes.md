# Note dal `solana-dev-skill` (solana-foundation)

> Documento di sola estrazione, come `docs/pyth-reference.md`. **Non
> sostituisce la Fase 0** (il verdetto sulla toolchain resta interamente
> demandato all'utente, in locale) — dice solo quale combinazione tentare
> per prima, con provenienza `file:riga` su questo repository.

**Repository:** `https://github.com/solana-foundation/solana-dev-skill`
**Commit:** `68ee828a6c25af0d834d07559c3b4a7fc3343321`
**Data del commit:** 2026-08-24 10:08:41 -0700
**Metodo:** `git clone` via proxy (GitHub raggiungibile in questo ambiente;
`npx skills add` non usato, richiede npm registry, bloccato).

Percorso base dei file citati sotto:
`skills/solana-dev/references/`.

---

## 1. `compatibility-matrix.md` — quale combinazione tentare per prima

Master Compatibility Table (`compatibility-matrix.md:22-33`): la riga più
recente è **Anchor 1.1.x** (`compatibility-matrix.md:26`), rilasciata giu
2026, Solana CLI 3.1.x (CI-testato su 3.1.10), Rust MSRV 1.89, Node ≥20.18.
Subito sotto, **Anchor 1.0.x** (`compatibility-matrix.md:27`), apr 2026,
Solana CLI 3.x, Rust 1.79–1.85+, Node ≥17 — è la riga rilevante per questo
progetto, dato che la spec originale indica esplicitamente "Anchor 1.x" (non
1.1 specificamente) e "Solana 3.x".

Combinazione **raccomandata per un progetto nuovo** (sezione "Known Working
Combinations (Tested)", `compatibility-matrix.md:192-207`):

```
Anchor CLI: 1.1.2
anchor-lang: 1.1.2
solana-* crates: ^3
litesvm (dev): 0.14.0  (Agave 4.1-based)
mollusk-svm (dev): 0.14.0
Solana CLI: 3.1.10 (Anchor CI-tested pairing)
Rust: ≥1.89
Node.js: ≥20.18
OS: Ubuntu 24.04+ (GLIBC ≥2.39) o macOS 14+
```

Combinazione alternativa per **"Anchor 1.0.x esistenti"**
(`compatibility-matrix.md:209-224`), rilevante se il ramo A/B della Fase 0
(requirement `anchor-lang` portato da `pyth-solana-receiver-sdk`, che
`docs/pyth-reference.md` §4 riporta come caret `"1.0.2"` — cioè
`^1.0.2` — sul commit `465e8dcb...` di `pyth-crosschain`) forzasse in
pratica a restare su 1.0.x invece che salire a 1.1.x:

```
Anchor CLI: 1.0.3
anchor-lang: 1.0.3
solana-* crates: ^3
litesvm (dev): 0.8.2 (o 0.9.1 se solana-hash 4.0 / solana-vote-interface 5.0)
Solana CLI: 3.x
Rust: 1.79–1.85+
Node.js: 20.x LTS
```

**Correzione (verificata su richiesta, riga testuale con `cat -A`)**: il
requirement di `pyth-solana-receiver-sdk` è `anchor-lang = "1.0.2"`
(via `{ workspace = true }` in
`target_chains/solana/pyth_solana_receiver_sdk/Cargo.toml:18`, risolto in
`target_chains/solana/Cargo.toml:31` sul commit `465e8dcb...` di
`pyth-crosschain`) — **senza `=` davanti**: in Cargo è un caret
requirement (`^1.0.2`, cioè `>= 1.0.2, < 2.0.0`), **non un pin esatto**.
`anchor-lang` 1.1.2 lo soddisfa a livello di resolver. Una versione
precedente di questo documento lo descriveva erroneamente come pin esatto
e ne derivava la raccomandazione di partire da Anchor 1.0.x: quella
inferenza cade. Il primo tentativo per il ramo A della Fase 0 può quindi
essere direttamente la riga **Anchor 1.1.x raccomandata**
(`compatibility-matrix.md:192-207`), con l'avvertenza che la
compatibilità del resolver non garantisce che l'SDK compili davvero
contro le API di 1.1.x — se il build fallisce, il ramo B della spec
(leggere la versione risolta nel `Cargo.lock`, poi
`cargo update -p anchor-lang@... --precise ...`) resta la via indicata
dalla spec, non una scelta di questo documento.

Nota GLIBC (`compatibility-matrix.md:73-92`): Ubuntu 24.04 (Noble, GLIBC
2.39) o Debian 13 (Trixie, GLIBC 2.40) coprono ogni versione di Anchor
citata qui; Ubuntu 22.04/Debian 12 richiedono build da sorgente per Anchor
0.31+.

Verified Test Environment del gen 2026 (`compatibility-matrix.md:289-296`):
su Debian 12 (GLIBC 2.36) `litesvm` npm 0.5.0 fallisce
(`__isoc23_strtol` non trovato, richiede GLIBC ≥2.38), ma `cargo build-sbf`
e Anchor 0.30.1 costruito da sorgente funzionano. Rilevante solo se
l'ambiente locale dell'utente è Debian 12 o simile.

---

## 2. `common-errors.md` — conflitti di versione Anchor

Tutti i seguenti sono in `common-errors.md`, sezione "Anchor Version
Migration Issues" (righe 288-371) salvo indicato:

- **0.29 → 0.30** (`common-errors.md:290-319`): `.accounts({...})` in
  TypeScript va cambiato in `.accountsPartial({...})`; serve la feature
  `idl-build` nel `Cargo.toml` di ogni programma; `overflow-checks = true`
  va dichiarato esplicitamente nel `Cargo.toml` di workspace — **rilevante
  per noi**: `SECURITY.md`/`DEPLOY.md` non menzionano ancora questo flag,
  da verificare in Fase 0 se il workspace lo richiede esplicitamente o se è
  già il default su Anchor 1.x.
- **0.30 → 0.31** (`common-errors.md:321-341`): conflitti fra
  `solana_program::Pubkey` e `solana_sdk::Pubkey` se si importano
  direttamente `solana-program`/`solana-sdk` invece di passare da
  `anchor_lang::prelude::*` — coerente con quanto già fatto in questa
  bozza (ogni file usa `anchor_lang::prelude::*`, nessun import diretto di
  `solana-program`).
- **0.31 → 0.32** (`common-errors.md:343-370`): `solana-program` viene
  rimosso del tutto come dipendenza diretta (va sostituito con le crate
  granulari, es. `solana_pubkey::Pubkey`, o con il re-export di
  `anchor_lang::prelude::*`); **account mutabili duplicati non sono più
  permessi di default**, serve il vincolo `dup = account_a` esplicito se
  davvero si vuole passare lo stesso account due volte come mutabile. **Non
  rilevante per questa bozza**: nessuna `Accounts` struct qui passa lo
  stesso account due volte.
- **1.0.x TS package rename** (`compatibility-matrix.md:166-188`, non
  `common-errors.md`, ma strettamente un "conflitto di versione Anchor" e
  già annotato nella spec originale): `@coral-xyz/anchor` → `@anchor-lang/core`.
- **Mismatch CLI/crate non fatale** (`common-errors.md:561-582`): un
  `anchor-lang` più recente della CLI installata produce solo un warning,
  il build riesce comunque — utile da sapere se in Fase 0 il pin esatto
  `1.0.2` dell'SDK Pyth non combacia esattamente con la versione della CLI
  installata localmente.
- **`error[E0603]: module inner is private`** (`common-errors.md:90-93`):
  causato da un disallineamento fra la versione della crate `anchor-lang`
  in `Cargo.toml` e la versione della CLI `anchor --version` — stesso
  consiglio: allinearle.

Non strettamente "conflitto di versione Anchor" ma segnalato perché
rilevante per un workspace con dipendenze Pyth: la sezione
`edition2024 Crate Incompatibility` (`common-errors.md:586-662`) elenca
`indexmap ≥2.13.0` come portato da `toml_edit → proc-macro-crate →
borsh-derive → anchor-lang`, che richiede `edition2024` non supportato dal
cargo imbottito in platform-tools v1.48. Se la Fase 0 usa platform-tools
v1.48, questo può bloccare il build per una dipendenza transitiva di
`anchor-lang` stesso, non solo del codice Pyth.

---

## 3. `testing.md` — Mollusk vs LiteSVM per unit test di funzioni pure

`testing.md:129-138` (sezione "Unit Tests: Mollusk"):

> Mollusk è un harness di test leggero (`mollusk-svm` 0.14.x) che fornisce
> un'interfaccia diretta all'esecuzione di un'istruzione senza il runtime
> completo del validator. Migliore per test solo-Rust con controllo
> fine-grained.

Quando preferirlo (`testing.md:133-138`): esecuzione rapida per cicli di
sviluppo veloci, manipolazione precisa dello stato degli account per casi
limite, metriche di performance dettagliate e benchmark delle CU, test di
syscall personalizzate.

`testing.md:28-38` (sezione "Unit Tests: LiteSVM"): LiteSVM è una SVM
leggera che gira in-process nel test stesso; Surfpool (l'integration
testing centerpiece di questa fonte) è costruito sopra LiteSVM, quindi unit
e integration test condividono la stessa semantica SVM. Preferirla per:
esecuzione senza overhead di validator, manipolazione diretta dello stato
degli account, CU reporting integrato, supporto multi-linguaggio (Rust, TS,
Python).

**Quando conviene l'uno o l'altro, secondo questa fonte:** entrambi
eseguono un'istruzione senza validator completo; la differenza che questa
fonte enfatizza è che Mollusk è pensato per **benchmark di CU precisi e
controllo fine-grained sullo stato degli account passati a una singola
istruzione** (`testing.md:198-205`, `MolluskComputeUnitBencher`, genera un
report markdown di uso CU), mentre LiteSVM è la scelta quando serve
**condividere semantica con i test di integrazione** basati su Surfpool
(entrambi SVM), supporto multi-linguaggio, e funzionalità aggiuntive come
avanzamento di slot/clock (`testing.md:109-127`,
`svm.warp_to_slot`/`svm.set_sysvar`).

**Per `math.rs` di questo progetto, nessuno dei due è la scelta più
diretta**: `math.rs` è deliberatamente Rust puro senza `Context`/`Account`
(vedi il commento in testa al file), quindi non ha bisogno né di un'SVM
in-process (LiteSVM) né di un harness a livello di istruzione (Mollusk) —
`cargo test` semplice, come già impostato in questa bozza, resta la scelta
più diretta per quel file specifico. `testing.md:385` cita anche
`cargo-fuzz`/`libFuzzer` esplicitamente come adatto a "pure helpers (math,
parsing)" senza il runtime Solana completo — coerente con l'approccio già
scelto per `math.rs` (fixture Python indipendenti, Task B) e un candidato
naturale per un fuzz target futuro su `normalize_to_e8`/
`resolve_confidence_band` una volta che un toolchain esiste.

Per la **Fase 2** della spec originale (test end-to-end con `oracle-mock`,
account fittizi `PriceUpdateV2`-shaped), LiteSVM resta la scelta indicata
dal prompt originale ("template LiteSVM") — questa fonte non contraddice
quella scelta, la conferma come pattern comune (`testing.md:389-403`,
struttura consigliata `tests/unit/*.rs` con LiteSVM o Mollusk).

---

## 4. `security.md` — confronto con le nostre invarianti I1-I13

Il file `security.md` (749 righe) è una checklist molto più ampia delle
nostre 13 invarianti, perché copre l'intera superficie di un programma
Solana generico (inclusi Token-2022, CPI arbitrarie, fuzzing di
`remaining_accounts`, ecc. — molto di questo è esplicitamente fuori scope
per SOLCLASH-EVENTS, che non tocca token SPL né `remaining_accounts` per
disegno). Confronto voce per voce con quello che la checklist copre e noi
**non** abbiamo ancora, esplicitamente o come test dedicato:

| Voce della checklist (`security.md`) | Copertura in SOLCLASH-EVENTS | Nota |
|---|---|---|
| **Missing Owner Checks** (righe 34-58) | Coperta implicitamente da `Account<'info, T>` su ogni account tipizzato (`Event`, `BetEntry`, `MockPriceUpdate`) | Nessun `UncheckedAccount` nel percorso dati tranne `fee_wallet`/`creator`, entrambi vincolati per indirizzo, non per owner (sono wallet, non account di dati) |
| **Missing Signer Checks** (righe 61-89) | Coperta: ogni istruzione con un effetto economico ha un campo `Signer<'info>` | — |
| **Reinitialization Attacks / `init_if_needed`** (righe 121-150) | Coperta: nessuna istruzione usa `init_if_needed` in questa bozza | Da riconfermare in Fase 0 quando il codice compila davvero |
| **PDA Sharing Vulnerabilities** (righe 154-172) | Coperta: seed di `Event` includono `creator` + `event_id`, seed di `BetEntry` includono `event` + `bettor` — nessun PDA condiviso fra utenti diversi | — |
| **Type Cosplay Attacks** (righe 176-194) | Coperta dal discriminator automatico di `#[account]` | — |
| **Duplicate Mutable Accounts** (righe 198-216) | **Non applicabile per costruzione**: nessuna `Accounts` struct in questa bozza passa lo stesso account due volte come mutabile — non testato esplicitamente, ma la mancanza stessa dello scenario lo rende non necessario |
| **Revival Attacks** (righe 220-241) | Coperta dal vincolo Anchor `close = ...` usato ovunque un account si chiude (`cancel_bet`, `claim`, `claim_refund`, `close_event`) | Questo è il pattern esatto raccomandato da `security.md:229-231` |
| **Lamport Griefing (Pre-funded PDA)** (righe 315-337) | **Parzialmente coperta ma non allineata alla stessa tecnica**: la nostra I7 (`>=`, mai `==`) accetta dust pre-esistente invece di richiedere il deficit esatto come fa questa checklist per l'`init`. Il pattern qui descritto (righe 321-337, calcolare `required - existing` prima di trasferire) riguarda l'*inizializzazione* di un PDA con `allocate`+`transfer`+`assign` manuali — **`create_event` in questa bozza usa `init` di Anchor**, che alloca ed esegue il `transfer` del rent-exempt minimum internamente; se un attaccante pre-finanzia l'indirizzo dell'`Event` PDA prima di `create_event`, il vincolo `init` di Anchor stesso può fallire (Anchor calcola il trasferimento come `required - existing`, coerente con questa fonte) — comportamento delegato ad Anchor, non gestito a mano da noi. **Non abbiamo un test dedicato per questo scenario** (creare un evento la cui PDA è già stata pre-finanziata con dust) |
| **Donation Attacks** (righe 597-601) | **Esplicitamente il nostro I7/I19**: "chiunque può inviare lamport a un account arbitrario" è testualmente lo scenario E19 della spec originale (dust prima della risoluzione) | La nostra soluzione (`>=` mai `==`) è coerente con "never derive protocol state from raw account balances" di questa fonte, dato che il nostro stato (`pot`, `payout_pool`) non è mai derivato dal saldo lamport dell'account, solo confrontato con esso come limite inferiore |
| **Rounding Direction** (righe 613-617) | Coperta per costruzione: ogni divisione (`claim`, `refund`, `protocol_fee`) arrotonda per difetto, sempre a favore del protocollo/pot residuo, mai a favore del singolo pagamento | Coerente con "round down on amounts the protocol pays out" |
| **Unchecked Type Casts** (righe 621-625) | **Gap parzialmente aperto**: `math.rs` usa `u64::try_from`/`i128 as i64` solo dove serve restringere; ma `instructions/*.rs` usa alcuni cast `as u32`/`as u8` **non presenti** in questa bozza (verificato: nessun cast `as` di restringimento nei file scritti) — nessuna azione necessaria, solo da riconfermare quando il codice compila |
| **TOCTOU (Bait-and-Switch)** (righe 581-585) | **Non applicabile per il percorso principale**: `resolve_event`/`challenge_resolution` leggono lo stato dell'account `price_update` passato nella stessa transazione, non un riferimento a stato "attuale" esterno | — |
| **Slot / Epoch Boundary Exploitation** (righe 573-577) | **Gap non coperto esplicitamente**: `resolution_time`, `finalized_at`, `RESOLUTION_TIMEOUT_SECS` sono tutti basati su `Clock::unix_timestamp`, non su boundary di slot/epoch — il rischio descritto da questa fonte non si applica direttamente, ma non abbiamo un'analisi esplicita di on chi possa influenzare l'ordinamento delle transazioni attorno a `resolution_time` (es. un resolver che ritarda la propria `resolve_event` aspettando un prezzo più favorevole) |
| **Malicious / Observing RPC** (righe 653-659) | **Fuori scope dichiarato**: riguarda il client, non il programma; menzionato qui solo per completezza |
| **Hidden Backdoors / Trust Minimization** (righe 685-691) | Coperta dal disegno: nessuna funzione admin/pausa, `DEPLOY.md` già prevede burn dell'upgrade authority | — |
| **`Agent-Assisted Development Safety`** (righe 693-703) | **Non applicabile a questo repository**: è codice mai compilato, nessuna transazione reale è mai stata proposta o firmata in questa sessione | Rilevante invece per un futuro agente che orchestrasse un vero deploy — da tenere presente in `DEPLOY.md` se in futuro un agente eseguirà quei passi |

**Il gap più concreto trovato**: il pattern di lamport-griefing sull'`init`
(righe 315-337) descrive esplicitamente perché un `init` naive può fallire
se l'account è pre-finanziato — non abbiamo ancora un test Fase-2 dedicato
a "creare un `Event` la cui PDA ha già ricevuto dust prima di
`create_event`" per confermare che il comportamento di Anchor `init` in
quello scenario sia quello atteso (creazione riuscita, non un fallimento
silenzioso). Aggiunto come nota in `DEVIATIONS.md`.

---

## 5. `concepts.md` — rent, PDA, e `close` + lamport manuale

`concepts.md` (69 righe totali) copre:

- **Rent come deposito interamente rimborsabile** (`concepts.md:12`): "Rent
  is a fully-redeemable deposit, not a recurring charge... The full
  deposit comes back when the account is closed." Nessuna carica
  periodica dalla feature *Disable rent fees collection* — coerente con
  come questa bozza tratta `rent_exempt_minimum` come valore fisso salvato
  una tantum in `create_event`, mai ricalcolato.
- **PDA off-curve** (`concepts.md:18-24`): un PDA è un indirizzo
  deliberatamente fuori dalla curva Ed25519, nessuno scalare vi corrisponde,
  quindi nessuna keypair può mai firmare per esso — solo il programma
  proprietario può autorizzarlo fornendo i seed. `find_program_address`
  cerca il bump canonico decrescendo finché il risultato è off-curve;
  raccomandazione esplicita: salvare il bump canonico e usare
  `create_program_address` sul percorso caldo — esattamente il pattern già
  usato in questa bozza (`event.bump`/`bet_entry.bump` salvati a `init`,
  riletti con `bump = event.bump` nelle istruzioni successive).

**Non presente in `concepts.md`**: nessuna menzione di `close` combinato
con manipolazione manuale dei lamport (`sub_lamports`/`try_borrow_mut_lamports`)
nella stessa istruzione — cercato per intero il file, zero occorrenze. Il
contenuto realmente pertinente a questa domanda vive altrove in questo
stesso skill repository, non in `concepts.md`:

- **`security.md:220-241`** (sezione "Revival Attacks", già vista al punto
  4 sopra): il pattern sicuro per Anchor è il vincolo dichiarativo
  `#[account(mut, close = destination)]`, non una chiusura manuale — quello
  che questa bozza usa ovunque (`cancel_bet`, `claim`, `claim_refund`,
  `close_event`, tutti con `close = bettor` o `close = creator`). Per un
  programma **Pinocchio** (non il nostro caso, ma mostrato come confronto),
  il pattern manuale sicuro è: prima sommare i lamport alla destinazione
  (`destination.set_lamports(destination.lamports() + account.lamports())`),
  poi chiudere l'account sorgente (azzerare i dati, riassegnare al System
  Program) — mai il contrario, e mai solo azzerare i lamport senza anche
  azzerare/riassegnare i dati (altrimenti l'account può essere "resuscitato"
  nella stessa transazione rifinanziandolo).
- **`programs/design-patterns.md:123`**: "Close accounts properly (Anchor
  `close` constraint): zero data, assign to system program, realloc to 0.
  Don't just zero lamports." — conferma che il vincolo `close` di Anchor fa
  automaticamente tutti e tre i passaggi (dati, owner, lamport), non solo i
  lamport.
- **`programs/design-patterns.md:118`**, un dettaglio distinto ma
  adiacente e potenzialmente rilevante per questa bozza: *"When doing
  direct lamport changes before a CPI, include all changed-lamport
  accounts in the CPI (or none) or the runtime's balance check fails."*
  Questa bozza usa manipolazione diretta dei lamport
  (`instructions::transfer_from_pda`, vedi `DEVIATIONS.md`) in istruzioni
  che **non** eseguono anche una CPI nella stessa istruzione (l'unica CPI
  del programma, il `system_program::transfer` di `place_bet`, è in
  un'istruzione che non fa alcuna manipolazione diretta di lamport) — quindi
  questo specifico gotcha non sembra applicarsi al disegno attuale, ma vale
  la pena ricontrollarlo in Fase 0 se l'ordine delle operazioni cambia.
- Il termine letterale `sub_lamports` non compare in nessun file di questo
  skill repository (verificato con una ricerca sull'intero albero) — i nomi
  usati qui sono `set_lamports`/`try_borrow_mut_lamports`, non
  `sub_lamports`.
