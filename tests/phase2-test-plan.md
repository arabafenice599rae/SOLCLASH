# Piano di test Fase 2

> **Stato: piano, nessun test implementato né eseguito.** Questo file
> materializza nel repository la tabella di test, aggiornata al disegno
> corrente (post-refactor canonicità, 2026-08-30: E20-E25 rimossi con il
> meccanismo di sfida, E29-E31 aggiunti). Da implementare su template
> LiteSVM, tutti su feature `oracle-mock`, quando la Fase 0 sarà conclusa.
> `math.rs` è escluso da questa suite: le sue funzioni pure si testano con
> `cargo test` semplice contro `tests/fixtures/*.json`.

| ID | Test | Aspettativa |
|---|---|---|
| E1 | `place_bet` dopo `betting_close_time` | Fallisce |
| E2 | **Late betting** fra `betting_close_time` e `resolution_time` | Fallisce |
| E3 | Gap sotto `MIN_RESOLUTION_GAP_SECS` | Fallisce |
| E4 | `feed_id` fuori whitelist | Fallisce |
| E5 | **Canonicità, lato superiore**: `publish_time < resolution_time` (update interamente prima di T) | Fallisce con `OracleUpdateBeforeResolution` |
| E6 | **Canonicità, lato inferiore**: `prev_publish_time >= resolution_time` (un update precedente è già a/dopo T, quindi questo non è il primo) | Fallisce con `OracleNotFirstAfterResolution` |
| E7 | `verification_level` parziale | Fallisce |
| E8 | `feed_id` di un altro asset | Fallisce |
| E9 | Account owned dal Price Feed program invece che dal receiver | Fallisce |
| E10 | Doppia `resolve_event` | La seconda fallisce (status non più `Locked`) |
| E11 | `resolve_event` prima di `resolution_time` | Fallisce |
| E12 | **Confidence band**: prezzo dentro la banda | AMBIGUO → REFUNDABLE, reward pagato |
| E13 | `conf/price` sopra soglia | `OracleConfidenceTooWide` |
| E14 | Prezzo negativo o zero | Fallisce |
| E15 | **Book monolaterale** al lock | REFUNDABLE immediato, rimborso 100% |
| E16 | `claim` da `BetEntry` di altro evento; doppio `claim` | Tutti falliscono. (Il `claim` del perdente NON è più un caso di errore: vedi E28) |
| E17 | **Ultimo claim a 50 vincitori** con stake eterogenei | Riesce, `Σ ≤ payout_pool` |
| E18 | **Ultimo refund a 50 bettor** dopo esito ambiguo | Riesce |
| E19 | Dust: 1 lamport al PDA prima della risoluzione | Tutto riesce |
| E26 | **Lamport griefing pre-init**: lamport inviati all'indirizzo PDA dell'`Event` PRIMA di `create_event` | `create_event` riesce comunque (il vincolo `init` di Anchor deve trasferire solo il deficit di rent, non fallire sull'account pre-finanziato); l'evento è poi utilizzabile end-to-end (`place_bet` → lock → resolve → claim), e l'invariante di escrow (`>=`, I7) assorbe il surplus come dust |
| E27 | **Free option chiusa**: `cancel_bet` dopo `betting_close_time` ma PRIMA che `lock_event` sia stato chiamato (status ancora OPEN) | Fallisce con `BettingClosed` |
| E28 | **Chiusura completa su RESOLVED**: evento con vincitori E perdenti, tutti chiamano `claim` | I vincitori incassano la propria quota; i perdenti non ricevono alcun trasferimento ma il loro `BetEntry` si chiude e recuperano il rent; al termine `close_event` riesce. Complemento di E17: copre il percorso felice che era rotto |
| E29 | **Finestra di risoluzione chiusa** (B-2): `resolve_event` dopo `resolution_time + RESOLUTION_TIMEOUT_SECS` | Fallisce con `ResolutionWindowClosed` (solo `mark_refundable` resta possibile) |
| E30 | **Confine di canonicità**: `publish_time == resolution_time` (con `prev_publish_time < resolution_time`) | **RIESCE** — l'update esattamente a T è quello canonico (`<=` sul lato superiore) |
| E31 | **Update stantìo**: canonico ma `publish_time - resolution_time > MAX_RESOLUTION_STALENESS_SECS` (buco del feed a T) | AMBIGUO → REFUNDABLE, reward pagato (non un errore) |

I test da E20 a E25 della spec (resolver grinding, sfida con `publish_time`
minore/uguale, sfida dopo `finalized_at`, sfida→ambiguo, `claim` prima di
`finalized_at`, monotonia di 20 sfide) sono stati **rimossi** con il
meccanismo di sfida (2026-08-30). L'esito che coprivano — "vince il
`publish_time` più alto" — è ora garantito per costruzione dalla canonicità
(E5/E6/E30), non da una corsa di sfide, quindi non c'è più niente da
testare lì.

E26 nasce dal confronto con la checklist di `solana-dev-skill`
(`references/security.md:315-337`, "Lamport Griefing (Pre-funded PDA)") —
vedi `docs/toolchain-notes.md` §4. È il complemento pre-creazione di E19
(che copre il dust *dopo* la creazione): insieme coprono l'intero ciclo di
vita dell'account rispetto a lamport non richiesti.

E27-E31 nascono dai finding dei due round di security review interno del
2026-08-30 (tutti confermati da validazione indipendente e corretti — vedi
`DEVIATIONS.md`): E27 il vincolo temporale su `cancel_bet`; E28 la chiusura
delle posizioni perdenti; E29 il limite superiore della finestra di
risoluzione (B-2); E30/E31 la nuova regola di canonicità e la sua politica
di staleness (B-1). E5/E6/E30 insieme sostituiscono E20-E25: verificano che
l'update di risoluzione sia provabilmente unico invece di doverlo far
emergere da una sfida.

## Oltre la tabella (dalla spec, invariati)

- Payout parimutuale con stake 1/2/3 su YES e 2/3 su NO.
- Conservazione dei lamport lungo l'intero ciclo di vita di un evento.
- Vettori di normalizzazione con exponent −6, −8, −9 e soglie al limite
  (già coperti come funzioni pure da `tests/fixtures/normalization.json` e
  `confidence_band.json`; qui vanno ripetuti end-to-end attraverso
  `resolve_event`).
- Overflow con `pot = MAX_POT_LAMPORTS`.
- End-to-end a 2, 10, 50 bettor.
- Fuzzing su `price_update` fasulli: owner, discriminator, dati troncati,
  `publish_time` estremi, `exponent` fuori range. **Nessun input deve
  produrre una risoluzione.**
