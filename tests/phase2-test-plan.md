# Piano di test Fase 2 (E1-E26)

> **Stato: piano, nessun test implementato né eseguito.** Questo file
> materializza nel repository la tabella di test della spec (E1-E25),
> più E26 aggiunto su richiesta, così che la suite abbia una sede
> versionata prima che esista un toolchain. Da implementare su template
> LiteSVM, tutti su feature `oracle-mock`, quando la Fase 0 sarà conclusa.
> `math.rs` è escluso da questa suite: le sue funzioni pure si testano con
> `cargo test` semplice contro `tests/fixtures/*.json`.

| ID | Test | Aspettativa |
|---|---|---|
| E1 | `place_bet` dopo `betting_close_time` | Fallisce |
| E2 | **Late betting** fra `betting_close_time` e `resolution_time` | Fallisce |
| E3 | Gap sotto `MIN_RESOLUTION_GAP_SECS` | Fallisce |
| E4 | `feed_id` fuori whitelist | Fallisce |
| E5 | `publish_time > resolution_time` | Fallisce |
| E6 | `publish_time < resolution_time - PUBLISH_WINDOW_SECS` | Fallisce |
| E7 | `verification_level` parziale | Fallisce |
| E8 | `feed_id` di un altro asset | Fallisce |
| E9 | Account owned dal Price Feed program invece che dal receiver | Fallisce |
| E10 | Doppia `resolve_event` | La seconda fallisce |
| E11 | `resolve_event` prima di `resolution_time` | Fallisce |
| E12 | **Confidence band**: prezzo dentro la banda | AMBIGUO → REFUNDABLE, reward pagato |
| E13 | `conf/price` sopra soglia | `OracleConfidenceTooWide` |
| E14 | Prezzo negativo o zero | Fallisce |
| E15 | **Book monolaterale** al lock | REFUNDABLE immediato, rimborso 100% |
| E16 | `claim` da `BetEntry` di altro evento; doppio `claim` | Tutti falliscono. (Il `claim` del perdente NON è più un caso di errore: vedi E28) |
| E17 | **Ultimo claim a 50 vincitori** con stake eterogenei | Riesce, `Σ ≤ payout_pool` |
| E18 | **Ultimo refund a 50 bettor** dopo esito ambiguo | Riesce |
| E19 | Dust: 1 lamport al PDA prima della risoluzione | Tutto riesce |
| E20 | **Resolver grinding**: due update validi con esiti opposti, sfida col più recente | Vince il `publish_time` maggiore |
| E21 | `challenge` con `publish_time` minore o uguale al candidato | Fallisce |
| E22 | `challenge` dopo `finalized_at` | Fallisce |
| E23 | Sfida che porta a candidato ambiguo | Dopo finalize → REFUNDABLE |
| E24 | `claim` prima di `finalized_at` | Fallisce (I13) |
| E25 | **Monotonia**: 20 sfide in ordine casuale | Stato finale = massimo dei `publish_time` validi |
| E26 | **Lamport griefing pre-init**: lamport inviati all'indirizzo PDA dell'`Event` PRIMA di `create_event` | `create_event` riesce comunque (il vincolo `init` di Anchor deve trasferire solo il deficit di rent, non fallire sull'account pre-finanziato); l'evento è poi utilizzabile end-to-end (`place_bet` → lock → resolve → claim), e l'invariante di escrow (`>=`, I7) assorbe il surplus come dust |
| E27 | **Free option chiusa**: `cancel_bet` dopo `betting_close_time` ma PRIMA che `lock_event` sia stato chiamato (status ancora OPEN) | Fallisce con `BettingClosed` |
| E28 | **Chiusura completa su RESOLVED**: evento con vincitori E perdenti, tutti chiamano `claim` | I vincitori incassano la propria quota; i perdenti non ricevono alcun trasferimento ma il loro `BetEntry` si chiude e recuperano il rent; al termine `close_event` riesce. Complemento di E17: copre il percorso felice che era rotto |

E26 nasce dal confronto con la checklist di `solana-dev-skill`
(`references/security.md:315-337`, "Lamport Griefing (Pre-funded PDA)") —
vedi `docs/toolchain-notes.md` §4. È il complemento pre-creazione di E19
(che copre il dust *dopo* la creazione): insieme coprono l'intero ciclo di
vita dell'account rispetto a lamport non richiesti.

E27 ed E28 nascono dai due finding del security review interno del
2026-08-30 (entrambi confermati da validazione indipendente e corretti —
vedi `DEVIATIONS.md`, "Fix del security review"): E27 verifica il vincolo
temporale aggiunto a `cancel_bet`, E28 verifica che `claim` chiuda anche
le posizioni perdenti e che `close_event` torni raggiungibile.

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
