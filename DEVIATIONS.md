# Deviazioni e ambiguità

Ogni punto sotto è una decisione presa dove la spec non specificava un
valore o un comportamento, o era ambigua. Sono decisioni di disegno: la
Fase 0 (vedi `TOOLCHAIN.md`) ne ha verificato la *compilabilità*, non la
correttezza — quella richiede i test on-chain della Fase 2.

## Ambiente di lavoro

Queste due voci descrivevano l'ambiente in cui la bozza fu scritta e non
valgono più; restano qui perché spiegano perché il codice aveva la forma
che aveva.

- ~~**Nessun accesso di rete a crates.io, npmjs.org, o `mcp.solana.com`.**
  Confermato con probe diretti (Fase −1)... GitHub è raggiungibile ed è
  stato usato per clonare `pyth-crosschain` (Task C).~~ Nell'ambiente della
  Fase 0 crates.io, npmjs.org, `release.anza.xyz` e i download di release
  di GitHub sono raggiungibili. Resta bloccata `api.github.com`, il che ha
  un effetto concreto: `avm install` non riesce a verificare la *build
  provenance* del binario Anchor precompilato e rifiuta di installarlo. È
  stato compilato da sorgente (`--from-source`) invece di saltare il
  controllo. Vedi `TOOLCHAIN.md`.
- ~~Per istruzione esplicita dell'utente, questa bozza non installa nulla,
  non crea `TOOLCHAIN.md`, non dichiara compatibilità con nessuna versione
  di Anchor/Solana, e non scrive un `Cargo.toml`. La Fase 0 resta demandata
  all'utente.~~ La Fase 0 è stata eseguita: i manifest esistono, le versioni
  sono dichiarate e verificate in `TOOLCHAIN.md`.

## Program ID placeholder

`lib.rs`: `declare_id!` richiede una stringa base58 valida a livello
sintattico. Per evitare un placeholder "plausibile" che potesse sembrare un
indirizzo reale, ho generato `sha256("SOLCLASH_EVENTS_PLACEHOLDER_DO_NOT_DEPLOY_FASE0_PENDING")`
reinterpretato come Pubkey (nessuna chiave privata esiste per questo
indirizzo — è un hash, non una keypair, generato offline con Python
standard library, nessuna rete coinvolta). Va rigenerato per davvero con
`solana-keygen new` + `anchor keys sync`, come annotato nel commento sopra
`declare_id!` e in `DEPLOY.md`.

**Aggiornamento Fase 0:** il placeholder è stato deliberatamente *lasciato*
dov'è. `anchor build` genera un keypair proprio in `target/deploy/` e
segnala il disallineamento, ma la build va a termine. Sincronizzare
scriverebbe nel repository un indirizzo la cui chiave privata vive solo in
una `target/` non versionata: un id che nessun altro può firmare e che il
repository lascerebbe intendere come proprio. Il placeholder dice la
verità. Conseguenza da tenere presente: un programma deployato così
rifiuta ogni istruzione con `DeclaredProgramIdMismatch`, quindi la Fase 2
dovrà generare una chiave vera prima di qualunque test on-chain.

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
| `FEED_WHITELIST_DEV` | `[0u8;32]`, `[1u8;32]`, `[2u8;32]` | I feed id reali non sono stati ancora sourced dal registro pubblicato di Pyth: è lavoro di Fase 3, insieme al percorso `PriceUpdateV2` che li userebbe — vedi `docs/pyth-reference.md` §7 |

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

## Assunzioni API Anchor — esito della verifica (Fase 0)

Le quattro assunzioni che la bozza aveva dichiarato, ciascuna con l'esito
del confronto con `anchor-lang 1.1.2`. Tre su quattro erano corrette.

| Assunzione | Esito |
|---|---|
| `ctx.bumps.<nome_account>` come accessor del bump canonico di una PDA (pattern post-0.29), usato in `create_event` e `place_bet` | **Corretta.** |
| `#[derive(PartialEq, Eq)]` impilato sopra `#[error_code]` per rendere `SolclashError` confrontabile con `==`/`assert_eq!` | **Corretta, e necessaria.** `#[error_code]` preserva i derive impilati e non aggiunge `PartialEq` di suo: togliere l'attributo rompe i test puri di `math.rs`. |
| La forma di `anchor_lang::system_program::{transfer, Transfer}` per la CPI in `place_bet` | **Corretta sui nomi, sbagliata sulla firma.** `Transfer { from, to }` con due `AccountInfo` è giusto, ma `CpiContext::new` in 1.x prende il program **id** (`Pubkey`), non l'`AccountInfo` del System Program come in 0.x. Era l'unico errore di compilazione dell'intera bozza. |
| `#[derive(InitSpace)]` di esistenza/forma incerta in Anchor 1.x, da cui il calcolo a mano di `Event::SPACE` e `BetEntry::SPACE` | **Il macro esiste** in anchor-lang 1.1.2. Il calcolo a mano è stato mantenuto lo stesso, ora come scelta e non come ripiego: la scomposizione termine per termine è verificabile riga per riga contro i campi della struct, un numero derivato da macro no. Resta però **non confrontato** con la dimensione borsh reale — vedi `TOOLCHAIN.md`. |

Un quinto punto, non dichiarato dalla bozza ma emerso alla prima
compilazione: il letterale di `FEE_WALLET_DEV` aveva 41 caratteri `'1'`
dove l'indirizzo del System Program ne ha 32. `pubkey!` lo decodificava
comunque al valore giusto (32 byte a zero), ma solo per la tolleranza del
decoder base58 sugli zeri iniziali. Normalizzato alla forma canonica.

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
