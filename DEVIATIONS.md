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
| `MIN_RESOLUTION_GAP_SECS_DEV` | 300s | 5 minuti, scelto senza un razionale specifico dalla spec |
| `MAX_EVENT_HORIZON_SECS_DEV` | 30 giorni | Policy di prodotto (2026-08-30, utente): il prodotto è una fabbrica di micro-eventi; un mese copre ogni caso legittimo e tiene il lockup peggiore a 37 giorni (30 + RESOLUTION_TIMEOUT 7). Da confermare col posizionamento del prodotto |
| `MAX_RESOLUTION_STALENESS_SECS_DEV` | 120s | Pyth aggiorna ~1/s; un buco di 2 minuti a `resolution_time` è un'outage reale, non jitter → esito AMBIGUO |
| `RESOLUTION_TIMEOUT_SECS_DEV` | 7 giorni | La spec dice solo "giorni". È anche il limite superiore della finestra di risoluzione (B-2) |
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

**Aggiornamento 2026-08-30**: con la rimozione del meccanismo di sfida
(vedi sotto) lo stato `Resolving` non esiste più, quindi
`outstanding_liability` ha solo due rami: `Open | Locked => pot`,
`Resolved | Refundable => payout_pool`. La tensione descritta sotto si
semplifica di conseguenza. Il testo originale è mantenuto per traccia
storica, con le correzioni inline.

Ho implementato la formula (`Event::outstanding_liability` in `state.rs`),
ma **non** è auto-consistente in ogni punto in cui `event` viene toccato:

1. **`resolve_event` paga `RESOLVER_REWARD` (e la fee) nella stessa
   istruzione in cui porta lo stato a un terminale.** Nel disegno attuale
   il controllo di escrow è collocato **alla fine**, sullo stato terminale
   di riposo: dopo che fee e reward sono usciti, il PDA trattiene
   esattamente `rent + payout_pool`, e `outstanding_liability` per
   `Resolved|Refundable` è `payout_pool`, quindi `lamports >= rent +
   payout_pool` vale (con `==`, più l'eventuale dust come slack per il
   `>=`). Non serve più il collocamento delicato "prima del pagamento del
   reward" che il vecchio disegno a `Resolving` richiedeva.
2. ~~Per il resto della fase `Resolving` (dentro `challenge_resolution`)~~
   **Obsoleto**: non c'è più fase `Resolving` né `challenge_resolution`.
3. **Dentro `claim`/`claim_refund`**, `payout_pool` rappresenta il totale
   originale al momento della transizione terminale, non il residuo dopo i
   pagamenti già effettuati. La formula letterale (`Resolved|Refundable =>
   payout_pool`) smette di essere vera dopo il primo claim riuscito. Ho
   quindi chiamato questo controllo **solo** ai punti di transizione
   (`lock_event`, `mark_refundable`, `resolve_event`), mai dentro `claim`/
   `claim_refund`/`close_event`, e la correttezza lì si appoggia sulla
   proprietà matematica della divisione floor (I11), non su un controllo di
   saldo ripetuto.

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
`ResolveEvent`, non dietro una feature flag, perché in Fase 1 non esiste
alcuna alternativa. Quando la Fase 3 aggiungerà un percorso reale verso
`PriceUpdateV2`, servirà decidere come `resolve_event` sceglie fra mock e
reale (due varianti di istruzione, un parametro generico, o un'astrazione
a runtime) — non deciso qui, esplicitamente fuori scope per questa bozza.

## Rimozione del meccanismo di sfida (2026-08-30) e assunzione Fase 3

Su indicazione dell'utente, dopo il finding B-1 del security review, il
disegno a tre istruzioni della spec (`resolve_event` →
`challenge_resolution` → `finalize_resolution`, con stato `Resolving`,
campi `candidate_*`, `finalized_at`, `PUBLISH_WINDOW_SECS`,
`RESOLUTION_CHALLENGE_SECS`, e le invarianti I8/I13) è stato **rimosso**,
non riparametrato. Al suo posto `resolve_event` verifica on-chain la
canonicità dell'update (`prev_publish_time < resolution_time <=
publish_time`) e va direttamente a un terminale. Razionale: il disegno a
sfida non era sbagliato, era una *mitigazione* di un problema — "quale dei
~60 update in finestra è quello giusto" — dove in realtà è disponibile una
*verifica*, perché Pyth pubblica `prev_publish_time` e quella disuguaglianza
individua l'update in modo univoco. Una mitigazione che dipende da qualcuno
che sfida entro una finestra breve e non finanziata è strettamente peggiore
di un controllo che rende l'update provabilmente unico. Dettaglio del
cambiamento in README, SECURITY.md (I10 riscritta, I8/I13 ritirate) e nel
registro error code.

**B-2, corretto nello stesso passaggio** (race resolve/refund dopo il
timeout): `resolve_event` non aveva un limite temporale superiore, quindi
dopo `resolution_time + RESOLUTION_TIMEOUT_SECS` era simultaneamente
valido con `mark_refundable` dallo stato `Locked`, e l'ordinamento delle
transazioni decideva il regime di payout (winner-take-all + fee vs rimborso
pro-rata). Aggiunto `require!(now < resolution_time +
RESOLUTION_TIMEOUT_SECS_DEV, ResolutionWindowClosed)` in `resolve_event`:
le due finestre sono ora disgiunte (`<` vs `>=` sul secondo esatto),
stesso stile di confine di `place_bet`/`cancel_bet`/`lock_event`. Test E29.

**Assunzione da verificare in Fase 3, VINCOLANTE prima di qualunque
deploy:** che `prev_publish_time` in un `PriceUpdateV2`/`PriceFeedMessage`
reale sia effettivamente il `publish_time` dell'update **immediatamente
precedente per lo stesso feed** — cioè che non esista un update pubblicato
fra `prev_publish_time` e `publish_time` per quel feed. Solo così la
disuguaglianza `prev_publish_time < resolution_time <= publish_time`
individua un update **unico**. `docs/pyth-reference.md` §2 riporta la
semantica dichiarata da Pyth ("per ogni t, l'update unico è quello con
`prev_publish_time < t <= publish_time`"), ma nota anche che durante
migrazioni `prev_publish_time` può essere uguale a `publish_time` o saltare
update. **Da confermare su byte reali di devnet: due update consecutivi
dello stesso feed, verificando che `update_2.prev_publish_time ==
update_1.publish_time`.** Se questa proprietà non regge in pratica, la
canonicità non è garantita e il disegno a sfida va ripristinato — questa è
la condizione esplicita a cui è appesa la rimozione della sfida.

## F1 (terza passata): tetto su `resolution_time`, due controlli distinti

Il terzo round di security review ha trovato un lockup permanente in
versione *aritmetica* dello stesso pattern che ci accompagna: il backstop
di timeout (`mark_refundable`) esiste **solo** per garantire che ogni
evento `Locked` restituisca i fondi, e un `resolution_time` estremo lo
annullava in silenzio. Se `resolution_time` è entro
`RESOLUTION_TIMEOUT_SECS_DEV` da `i64::MAX`, il
`resolution_time.checked_add(RESOLUTION_TIMEOUT_SECS_DEV)` va in overflow:
`mark_refundable` restituisce `MathOverflow` per sempre, `resolve_event`
fallisce prima con `ResolveTooEarly`, e l'evento non raggiunge mai un
terminale. `create_event` non aveva alcun tetto superiore su
`resolution_time`.

Su indicazione dell'utente sono stati aggiunti **due** controlli, non uno,
perché rispondono a problemi diversi:

1. **Correttezza aritmetica** (difesa in profondità, come `overflow-checks`):
   `resolution_time.checked_add(RESOLUTION_TIMEOUT_SECS_DEV).ok_or(
   ResolutionTimeoutNotComputable)?`. Rende l'overflow impossibile
   indipendentemente da quale valore assuma il tetto di prodotto domani.
2. **Policy di prodotto**: `resolution_time <= now +
   MAX_EVENT_HORIZON_SECS_DEV` (`EventHorizonTooFar`). La sola (1) lascia
   passare assurdità: un mercato che si risolve nell'anno 300 milioni
   supera la guardia di overflow e immobilizza rent e stake per un tempo
   che nessuno vedrà mai finire. Il tetto è calcolato su `now`, non in
   assoluto, così non invecchia.

Due error code distinti perché dicono cose diverse a chi legge il
fallimento: "questo numero è nonsense" vs "questo mercato è troppo lontano
per la nostra policy". Test E32. Valore del tetto: 30 giorni dev (vedi
tabella `_DEV`).

## Chiusura delle revisioni di sicurezza (Fase 1)

Su decisione dell'utente, le revisioni di sicurezza su questa bozza si
fermano dopo quattro passate. Il razionale: quattro letture su codice mai
compilato hanno dato quello che potevano — i finding trovati (chiusura dei
`BetEntry` perdenti, free option su `cancel_bet`, race resolve/refund,
finestra di selezione del candidato, e questo F1) sono stati tutti
corretti; il prossimo finding utile arriva dal primo `cargo build` reale
(Fase 0/1 in locale), non da una quinta lettura. Una nuova passata è
prevista a fine Fase 3, quando l'oracolo reale cambia di nuovo la forma del
codice.

## Normalizzazione di `conf`: ceiling, in deviazione dallo step 7 della spec

Lo step 7 della spec dice testualmente "stessa normalizzazione per conf:
usa lo stesso exponent", e la formula data usa `checked_div` (troncatura).
Su indicazione esplicita dell'utente in revisione (2026-08-30), `conf`
ora arrotonda **per eccesso** sul ramo di scale-down
(`math::normalize_conf_to_e8`), separata da `normalize_price_to_e8` che
resta troncante come da formula: troncare `conf` restringe la banda di
confidenza e rende il protocollo più incline a dichiarare un esito
definito vicino alla soglia — l'opposto della direzione conservativa di
I12. Lo stesso exponent resta condiviso (quella parte dello step 7 vale
ancora); cambia solo la direzione di arrotondamento. Magnitudine massima
dell'effetto: <1 unità e8 a exponent −9.

**Nota correlata, decisa (2026-08-30, utente)**: la formula dello step 8
(`conf_e8 * 10_000 / price_e8`, in `math::confidence_ratio_bps`)
troncava anch'essa, nella stessa direzione anti-conservativa (un feed
marginalmente troppo largo passava il gate `CONF_MAX_RATIO_BPS` per
arrotondamento). Ora **anche il ratio usa il ceiling**, su indicazione
dell'utente: un arrotondamento che agisce su una soglia di rifiuto
arrotonda verso il rifiuto. Il principio generale è formalizzato in
`SECURITY.md` ("Principio di arrotondamento") e vale per ogni
arrotondamento futuro.

## Fix del security review (2026-08-30), entrambi approvati dall'utente

Il security review interno (pipeline a due passaggi con validazione
indipendente dei finding) ha trovato due difetti reali, entrambi
confermati e corretti. Ciò che hanno in comune: entrambi nascono da una
transizione di stato che la spec presentava come automatica e che invece
richiede una transazione di qualcuno — `lock_event` per il primo,
la chiusura dei `BetEntry` per il secondo. Su Solana niente accade da
solo: ogni volta che la spec dice "quando accade X" bisogna chiedersi chi
manda la transazione e cosa ci guadagna.

1. **`cancel_bet` dopo `betting_close_time` era una free option** (High).
   Il solo controllo `status == Open` lasciava annullabile una puntata
   nella finestra fra chiusura e primo `lock_event` — crank permissionless
   che nessuno è obbligato a chiamare — cioè con informazione sul prezzo
   post-chiusura. La giustificazione originale della spec ("sicuro perché
   prima del lock l'esito è ignoto a tutti") confondeva "prima del lock"
   con "prima della chiusura"; l'utente, autore di quella frase, ha
   confermato l'errore di ragionamento e riscritto la regola. Fix: `now <
   betting_close_time` in `cancel_bet`, speculare a `place_bet`,
   riusando l'error code esistente `BettingClosed` (semanticamente esatto,
   nessun codice nuovo). Testo corretto della regola in `README.md`.
   Test: E27.

2. **I `BetEntry` perdenti non avevano alcun percorso di chiusura**
   (Medium: si attiva sul 100% dei mercati risolti, perdita limitata a
   rent e dust ma permanente, irreversibile dopo il burn dell'authority).
   `claim` rifiutava i perdenti e `claim_refund` richiede REFUNDABLE,
   quindi su RESOLVED `bets_closed == bettor_count` era insoddisfacibile
   e `close_event` falliva per sempre. Fix (variante A, scelta
   dall'utente): `claim` ora è "il partecipante chiude la propria
   posizione, incassando se ha vinto" — quota zero per i perdenti, e il
   ramo perdente **non trasferisce affatto** (non un trasferimento di
   zero lamport, che qualcuno un giorno leggerebbe come un pagamento);
   il `BetEntry` si chiude in ogni caso via `close = bettor` e
   `bets_closed` avanza. `NotWinningOutcome` rimosso dagli error code.
   Test: E28.

## Asimmetria accettata consapevolmente: uscita gratuita del lato unico

Variante segnalata dal review e NON chiusa: un bettor che sia l'unico sul
proprio lato può annullare **anche dentro la finestra legittima** (prima
di `betting_close_time`), collassando il book a monolaterale → REFUNDABLE
al lock, e negando ai bettor dell'altro lato la vincita. È un'uscita a
costo zero da una posizione percepita come perdente, ogni volta che sei
l'unico su un lato. Non è la stessa vulnerabilità del punto 1 (non c'è
informazione sul prezzo di risoluzione), ed è accettata deliberatamente:
in un mercato a micro-eventi con pochi partecipanti il diritto di uscire
prima della chiusura vale più del rischio. Decisione dell'utente,
2026-08-30.

## Arrotondamento della protocol fee

La spec dà la formula `protocol_fee = (pot - RESOLVER_REWARD) × 10%` senza
specificare la direzione di arrotondamento. Ho usato lo stesso floor via
intermedio `u128` usato ovunque altrove nel sistema (claim, refund), per
coerenza interna — non un valore dato esplicitamente dalla spec.

## Gap trovati confrontando `SECURITY.md` con `solana-dev-skill/references/security.md`

Vedi `docs/toolchain-notes.md` §4 per il confronto completo, voce per
voce, con la checklist di sicurezza generica di `solana-dev-skill`. I due
gap concreti emersi sono stati entrambi chiusi come segue (registrati qui
solo come traccia della decisione):

- **Lamport griefing sull'`init` di `Event`** → aggiunto come test **E26**
  in `tests/phase2-test-plan.md`: lamport inviati all'indirizzo PDA
  dell'`Event` prima di `create_event`; atteso che il vincolo `init` di
  Anchor trasferisca solo il deficit di rent e la creazione riesca.
- **`overflow-checks = true` nel `[profile.release]` del workspace** →
  promosso a **requisito di Fase 0** in `DEPLOY.md` (sezione
  Prerequisiti), non più una nota aperta: va nel `Cargo.toml` di workspace
  quando verrà scritto.

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
