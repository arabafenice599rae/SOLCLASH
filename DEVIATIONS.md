# Deviazioni e ambiguità

Ogni punto sotto è una decisione presa da questa bozza dove la spec non
specificava un valore o un comportamento, o era ambigua. Nessuno di questi
punti è stato verificato compilando o testando il codice — sono decisioni
di disegno, da rivedere quando la Fase 0 sarà completata.

## Ambiente di lavoro

- **Nessun accesso di rete a crates.io, npmjs.org, o `mcp.solana.com` in
  questo ambiente.** Confermato con probe diretti (Fase −1) e ri-confermato
  con `--noproxy '*'` e `NO_PROXY` espliciti: entrambi gli host sono
  elencati nella allowlist `noProxy` del proxy stesso ma restano comunque
  bloccati a livello di policy di rete, con `403 host_not_allowed`. GitHub
  è raggiungibile ed è stato usato per clonare `pyth-crosschain` (Task C).
- Per istruzione esplicita dell'utente, questa bozza **non** installa nulla
  da GitHub releases, **non** crea `TOOLCHAIN.md`, **non** dichiara
  compatibilità con nessuna versione di Anchor/Solana, e **non** scrive un
  `Cargo.toml` di progetto. La Fase 0 resta interamente demandata all'utente,
  in locale.

## Program ID placeholder

`lib.rs`: `declare_id!` richiede una stringa base58 valida a livello
sintattico. Per evitare un placeholder "plausibile" che potesse sembrare un
indirizzo reale, ho generato `sha256("SOLCLASH_EVENTS_PLACEHOLDER_DO_NOT_DEPLOY_FASE0_PENDING")`
reinterpretato come Pubkey (nessuna chiave privata esiste per questo
indirizzo — è un hash, non una keypair, generato offline con Python
standard library, nessuna rete coinvolta). Va rigenerato per davvero con
`solana-keygen new` + `anchor keys sync` in Fase 0, come annotato nel
commento sopra `declare_id!` e in `DEPLOY.md`.

## Costanti `_DEV` (TBD)

Ogni valore in `constants.rs` con suffisso `_DEV` è un placeholder di
sviluppo, non derivato da dati reali (nessuna misura di fee-market
mainnet, nessun calcolo di rent contro un validatore in esecuzione,
nessuna misura di latenza reale di Pyth Benchmarks). Elenco e razionale
di ciascuno:

| Costante | Valore dev | Razionale |
|---|---|---|
| `RESOLVER_REWARD_DEV` | 1_830_000 lamport | 0.00182 SOL di rent (valore dato dalla spec) + 2×5.000 lamport di fee per firma (valore comune ma non verificato on-chain) |
| `MIN_STAKE_LAMPORTS_DEV` | 18_300_000 lamport | 10× `RESOLVER_REWARD_DEV`, per soddisfare ">> RESOLVER_REWARD" senza un margine specifico dato dalla spec |
| `MAX_STAKE_LAMPORTS_DEV` | 1_000_000_000_000 (1.000 SOL) | Tetto di sicurezza arbitrario, nessun razionale economico dato dalla spec |
| `MAX_POT_LAMPORTS_DEV` | 10_000_000_000_000_000 (10.000.000 SOL) | Scelto per restare lontano da overflow `u64`/`u128`, non per un limite economico specifico |
| `FEE_WALLET_DEV` | Indirizzo del System Program | Placeholder sintatticamente valido e pubblicamente noto, esplicitamente NON un vero fee wallet. L'utente ha chiesto esplicitamente di non essere interpellato su questo valore in questa fase |
| `MIN_RESOLUTION_GAP_SECS_DEV` | 300s | Scelto solo per essere comodamente maggiore di `PUBLISH_WINDOW_SECS` (60s) |
| `RESOLUTION_CHALLENGE_SECS_DEV` | 300s | Nessun razionale dato dalla spec |
| `RESOLUTION_TIMEOUT_SECS_DEV` | 7 giorni | La spec dice solo "giorni" |
| `CONF_MAX_RATIO_BPS_DEV` | 500 bps (5%) | Nessun razionale dato dalla spec |
| `FEED_WHITELIST_DEV` | `[0u8;32]`, `[1u8;32]`, `[2u8;32]` | Nessun accesso verificato ai feed id reali di Pyth in questo ambiente (rete bloccata) — vedi `docs/pyth-reference.md` §7 |

`PROTOCOL_FEE_BPS` (1.000, dato dalla spec), `PUBLISH_WINDOW_SECS` (60, tetto
dato dalla spec), e `PYTH_RECEIVER_PROGRAM` (dato dalla spec) **non** sono
`_DEV`: sono presi verbatim dalla spec, non sono placeholder.

## Tensione nella formula dell'invariante di escrow (I7)

La spec fornisce questa formula verbatim:

```rust
let outstanding = match status {
    Open | Locked | Resolving => event.pot,
    Resolved | Refundable     => event.payout_pool,
};
require!(lamports >= event.rent_exempt_minimum + outstanding, EscrowMismatch);
```

Ho implementato questa formula esattamente com'è (`Event::outstanding_liability`
in `state.rs`), ma **non** è auto-consistente in ogni punto in cui `event`
viene toccato, per due ragioni distinte:

1. **`resolve_event` paga `RESOLVER_REWARD` nella stessa istruzione in cui
   porta lo stato a `Resolving`.** Se il controllo venisse eseguito *dopo*
   il pagamento, `lamports < rent + pot` per costruzione (il reward è
   uscito, `pot` no). L'ho quindi collocato **subito dopo** la scrittura
   dello stato `Resolving` ma **prima** del pagamento del reward — in quel
   preciso istante il saldo è ancora esattamente `rent + pot`, quindi il
   controllo è valido. Il pagamento avviene subito dopo, senza un secondo
   controllo.
2. **Per il resto della fase `Resolving`** (cioè dentro
   `challenge_resolution`, che non muove lamport), il saldo reale è
   permanentemente `rent + pot - RESOLVER_REWARD_DEV`, quindi la formula
   letterale (`Resolving => pot`) non può più essere soddisfatta. Ho scelto
   di **non chiamare affatto** il controllo generico dentro
   `challenge_resolution`, piuttosto che alterare la formula data o
   inventare un secondo campo di stato non richiesto dalla spec. Una
   versione più precisa (`lamports >= rent + pot - RESOLVER_REWARD_DEV`)
   sarebbe l'equivalente corretto, ma non l'ho aggiunta per non allontanarmi
   dalla formula fornita testualmente.
3. **Dentro `claim`/`claim_refund`**, `payout_pool` rappresenta il totale
   originale al momento della transizione terminale, non il residuo dopo i
   pagamenti già effettuati. La formula letterale (`Resolved|Refundable =>
   payout_pool`) smette di essere vera dopo il primo claim riuscito (il
   saldo scende sotto `rent + payout_pool`). Ho quindi chiamato questo
   controllo **solo** ai punti di transizione (`lock_event`,
   `mark_refundable`, `finalize_resolution`), mai dentro `claim`/
   `claim_refund`/`close_event`, e ho documentato che la correttezza lì si
   appoggia invece sulla proprietà matematica della divisione floor (I11),
   non su un controllo di saldo ripetuto.

Questa è l'interpretazione più coerente che ho trovato con "il controllo va
eseguito al momento della transizione di stato, prima di ogni movimento di
lamport successivo nella stessa istruzione" — ma è una mia lettura, non un
fatto dato esplicitamente dalla spec, e andrebbe confermata.

## `close_event`: destinatario del residuo

La spec dice solo "il resto resta nel PDA e lo raccoglie `close_event`",
senza specificare il destinatario dei lamport residui (rent-exempt minimum
dell'account `Event` + eventuale dust da arrotondamento floor). Ho scelto
`event.creator` (il creatore del mercato), tramite il vincolo Anchor
`close = creator` con `address = event.creator` sull'account destinatario,
perché nessun bettor ha più diritti su quei fondi una volta che ogni
`BetEntry` è stato chiuso, ed è il creatore ad aver pagato la rent iniziale
dell'account. `close_event` resta permissionless (chiunque può chiamarlo
per liberare la rent), solo il destinatario dei fondi è fissato al
creatore. Un'alternativa ragionevole sarebbe stata dare il residuo al
chiamante come incentivo al cleanup — non scelta, per evitare di introdurre
un incentivo non richiesto dalla spec.

## Errori "impliciti" coperti da vincoli Anchor invece che da `require!` manuali

Per istruzione esplicita del prompt originale ("prima di scrivere una
verifica a mano, controlla se esiste un constraint Anchor che la
esprime"), ho rimosso due controlli che avevo inizialmente previsto come
errori dedicati:

- **"BetEntry non appartiene al signer"** e **"BetEntry di un altro
  evento"**: i seed della PDA `BetEntry` (`["bet", event.key(),
  bettor.key()]`) rendono strutturalmente impossibile che un `bettor`
  diverso da quello originale, o un `event` diverso, producano un
  `bet_entry` che superi il vincolo `seeds =`/`bump =` di Anchor. Non ho
  aggiunto `NotBetOwner`/`BetEntryEventMismatch` come error code, per non
  scrivere codice morto che duplica una garanzia già strutturale.
- **Doppia scommessa dallo stesso wallet**: il vincolo `init` sulla PDA
  `BetEntry` in `place_bet` fallisce automaticamente (errore Anchor
  generico "account already in use") se la PDA esiste già — nessun
  controllo applicativo dedicato aggiunto.

## Trasferimenti di lamport: CPI vs manipolazione diretta

La spec richiede esplicitamente una CPI al System Program per il
trasferimento dello stake in `place_bet` ("Trasferisce stake all'Event PDA
via System Program CPI"). Per ogni altro movimento di lamport in uscita
dall'Event PDA (reward al resolver, protocol fee, claim, refund, rimborso
di `cancel_bet`), ho usato manipolazione diretta del saldo lamport
(`instructions::transfer_from_pda`) invece di una seconda CPI, perché:
(a) l'Event PDA è posseduto dal nostro stesso programma, quindi la
manipolazione diretta è legittima e non richiede una CPI `invoke_signed`;
(b) evita di introdurre complessità aggiuntiva (seeds/signer per
`invoke_signed`) senza motivo, coerente con "minimizza le crate importate"
e con il vincolo di whitelist CPI in `DEPLOY.md` (solo System Program e
Pyth Receiver, e la CPI al System Program resta l'unica, in `place_bet`).
Non è dichiarato esplicitamente dalla spec, ma è lo standard idiomatico per
un programma che sposta lamport dal proprio PDA.

## Assunzioni API Anchor non verificabili in questo ambiente

Nessuna di queste è stata confermata da un compilatore. Ognuna è annotata
anche inline nel codice sorgente dove usata:

- `ctx.bumps.<nome_account>` come accessor del bump canonico di una PDA
  (pattern Anchor moderno, post-0.29). Usato in `create_event` e `place_bet`.
- Impilare `#[derive(PartialEq, Eq)]` sopra `#[error_code]` per rendere
  `SolclashError` confrontabile con `==`/`assert_eq!` nei test puri di
  `math.rs`. Non è stato possibile verificare cosa `#[error_code]` derivi
  già di suo, perché il sorgente di `anchor-lang` non è disponibile in
  questo ambiente (crates.io bloccato).
- La forma esatta di `anchor_lang::system_program::{transfer, Transfer}`
  per la CPI in `place_bet` — nome e shape assunti dal pattern comune
  Anchor, non confermati contro il sorgente di `anchor-lang` 1.0.2.
- Spazio degli account (`Event::SPACE`, `BetEntry::SPACE`) calcolato a
  mano termine per termine, invece di usare un eventuale
  `#[derive(InitSpace)]`, la cui esistenza/forma in Anchor 1.x non è stata
  verificata da questo ambiente.

## Percorso reale Pyth (Fase 3) non ancora disegnato

`instructions/resolution.rs` referenzia direttamente
`oracle::mock::MockPriceUpdate` come tipo di account per `price_update` in
`ResolveEvent`/`ChallengeResolution`, non dietro una feature flag,
perché in Fase 1 non esiste alcuna alternativa. Quando la Fase 3
aggiungerà un percorso reale verso `PriceUpdateV2`, servirà decidere come
le due istruzioni scelgono fra mock e reale (due varianti di istruzione,
un parametro generico, o un'astrazione a runtime) — non deciso qui,
esplicitamente fuori scope per questa bozza.

## Arrotondamento della protocol fee

La spec dà la formula `protocol_fee = (pot - RESOLVER_REWARD) × 10%` senza
specificare la direzione di arrotondamento. Ho usato lo stesso floor via
intermedio `u128` usato ovunque altrove nel sistema (claim, refund), per
coerenza interna — non un valore dato esplicitamente dalla spec.

## Gap trovati confrontando `SECURITY.md` con `solana-dev-skill/references/security.md`

Vedi `docs/toolchain-notes.md` §4 per il confronto completo, voce per
voce, con la checklist di sicurezza generica di `solana-dev-skill`. Due
punti concreti, non coperti da un test dedicato in questa bozza:

- **Lamport griefing sull'`init` di `Event`**: nessun test Fase-2 previsto
  per "creare un `Event` la cui PDA ha già ricevuto dust prima di
  `create_event`". Il vincolo `init` di Anchor dovrebbe gestirlo
  internamente (trasferisce solo il deficit, non l'intero rent-exempt
  minimum), ma non è un comportamento che questa bozza verifica
  esplicitamente. Da aggiungere alla lista di test della Fase 2 quando un
  toolchain esiste.
- **`overflow-checks = true` nel `Cargo.toml` di workspace**: alcune
  versioni di Anchor (0.30, per `common-errors.md:310-319` del
  `solana-dev-skill`) lo richiedono esplicitamente nel `[profile.release]`
  del workspace, altrimenti il build fallisce. Questa bozza non ha un
  `Cargo.toml` (per istruzione esplicita dell'utente), quindi non è
  ancora verificabile se Anchor 1.x lo richieda ancora esplicitamente o se
  sia ormai un default — promemoria per quando il `Cargo.toml` reale verrà
  scritto in Fase 0.

## Layout byte di `PriceUpdateV2` — dipendenza dalla variante di `verification_level`

Vedi `docs/pyth-reference.md` §1.1: la spec descrive il layout come "134
byte" fissi, ma il sorgente Pyth mostra che `VerificationLevel` è un enum
Borsh a lunghezza di serializzazione variabile (1 byte per `Full`, 2 byte
per `Partial`). La costante `LEN = 134` di Pyth assume il caso da 2 byte
(il massimo). Per il ramo C (deserializzazione manuale, non necessario in
Fase 1 ma preparatorio per un'eventuale Fase 0 col ramo C), ho documentato
la tabella di offset **condizionata alla variante**, non una tabella fissa
unica — un dettaglio che la spec non menziona e che sarebbe facile
implementare in modo silenziosamente sbagliato.
