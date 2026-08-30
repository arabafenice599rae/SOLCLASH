# Sicurezza — SOLCLASH-EVENTS

> **Stato di questo documento:** descrive il modello di sicurezza del
> *disegno* del programma (Fase 1, bozza mai compilata — vedi
> `programs/solclash_events/src/lib.rs`). Nessuna riga di codice qui
> descritta è stata verificata da un compilatore, un test, o un audit.
> Non è un attestato di sicurezza, è la specifica delle invarianti che il
> codice deve rispettare quando la Fase 0 (toolchain) sarà completata e la
> Fase 2 (test) sarà verde.

## Modello di trust

SOLCLASH-EVENTS è **non-custodial e permissionless al 100%**, tranne tre
istruzioni riservate al proprietario della propria posizione
(`cancel_bet`, `claim`, `claim_refund`). Non esiste:

- un'autorità di amministrazione capace di mettere in pausa, modificare, o
  drenare un evento;
- un oracolo diverso da Pyth (nessun prezzo può essere iniettato da un
  operatore);
- una via per un singolo attore — creatore dell'evento incluso — di
  alterare l'esito dopo che la finestra di sfida (`RESOLUTION_CHALLENGE_SECS`)
  è scaduta.

L'unica parte esterna di cui ci si fida è **Pyth**: la rete di guardiani
Wormhole che firma gli update di prezzo, e il programma Pyth Receiver
(`PYTH_RECEIVER_PROGRAM`) che li verifica on-chain. Il programma non si fida
di nessun singolo publisher Pyth: si fida della soglia di firme guardiane
verificata dal receiver (`VerificationLevel::Full`), la stessa soglia che
qualunque consumer Pyth su Solana eredita.

## Invarianti

Ogni invariante è enforced nel codice (Fase 1 draft) come indicato; il
riferimento file:funzione userà una numerazione stabile una volta che la
Fase 0 avrà prodotto un `Cargo.toml` reale e i percorsi potranno essere
citati con `cargo doc`-style link.

- **I1 — Nessun payout senza risoluzione Pyth a `VerificationLevel::Full`.**
  Asserito esplicitamente in `oracle::verify_price_update` (non delegato a
  `get_price_no_older_than`, che questo programma non chiama — vedi
  `docs/pyth-reference.md` §3 per perché quella funzione SDK non basta).
- **I2 — Nessun payout a un `BetEntry` non sull'esito vincente.**
  `instructions::settlement::claim` calcola la quota solo per il ramo
  vincente; per un `BetEntry` perdente la quota è zero e **nessun
  trasferimento avviene** (non un trasferimento di zero lamport: il ramo
  perdente non trasferisce affatto). Il perdente chiama comunque `claim`
  per chiudere la posizione e recuperare il rent del proprio account —
  `claim` significa "chiudi la posizione, incassando se hai vinto", non
  "il vincitore incassa". Senza questo, i `BetEntry` perdenti non
  avrebbero alcun percorso di chiusura e `close_event` sarebbe
  irraggiungibile su ogni evento RESOLVED (finding del security review,
  2026-08-30).
- **I3 — La protocol fee può andare solo a `FEE_WALLET`.** Espresso come
  vincolo Anchor `address = FEE_WALLET_DEV` sull'account `fee_wallet` in
  `ResolveEvent`, non come controllo manuale — un vincolo di account
  fallisce prima ancora che l'handler inizi a eseguire.
- **I4 — Nessuna scommessa dopo `betting_close_time`.**
  `instructions::market::place_bet` verifica `now < event.betting_close_time`
  prima di accettare qualunque trasferimento. Lo stesso vincolo temporale
  vale per `cancel_bet`: annullare dopo la chiusura, nella finestra in cui
  `lock_event` (crank permissionless, che nessuno è obbligato a chiamare)
  non è ancora stato eseguito, sarebbe una free option esercitata con
  informazione sul prezzo post-chiusura (finding del security review,
  2026-08-30 — la giustificazione originale della spec, "sicuro perché
  prima del lock l'esito è ignoto a tutti", confondeva "prima del lock"
  con "prima della chiusura").
- **I5 — `resolution_time - betting_close_time >= MIN_RESOLUTION_GAP_SECS`,
  sempre.** Verificato una sola volta, alla creazione
  (`instructions::market::create_event`) — i due timestamp sono immutabili
  per il resto della vita dell'evento, quindi non serve riverificarlo altrove.
- **I6 — Nessun claim né refund due volte.** Nessun flag booleano: il
  `BetEntry` PDA si chiude (`close = bettor`) alla prima `claim` o
  `claim_refund` riuscita. Un secondo tentativo non trova l'account (già
  chiuso, dati azzerati, lamport già restituiti) e fallisce strutturalmente,
  non per un controllo applicativo che potrebbe essere dimenticato altrove.
- **I7 — L'escrow deve essere sufficiente (`>=`, mai `==`).**
  `Event::outstanding_liability()` più un `require!` dedicato
  (`EscrowMismatch`), chiamato ai punti di transizione di stato dove la
  formula è auto-consistente — **non** dentro `claim`/`claim_refund` (vedi
  `DEVIATIONS.md` per il perché di questa scelta). In `resolve_event` il
  check è sullo stato terminale finale, dopo l'uscita di fee e reward,
  quando il PDA trattiene esattamente `rent + payout_pool`.
- **I8 — (ritirata, 2026-08-30).** Descriveva la monotonia di
  `candidate_publish_time` e l'immutabilità di `finalized_at` — entrambi
  concetti del meccanismo di sfida, rimosso. Con l'update di risoluzione
  reso unico dalla canonicità (vedi I10) non c'è più un candidato da far
  evolvere né una finestra di sfida da proteggere. Il numero è mantenuto
  ritirato per non rinumerare le altre invarianti.
- **I9 — Il settlement non richiede alcuna autorità centrale.** Tutte le
  istruzioni tranne `cancel_bet`/`claim`/`claim_refund` sono permissionless
  per costruzione: chiunque può pagare il gas per far avanzare lo stato.
- **I10 — Il risultato è ricostruibile e verificabile on-chain da
  chiunque.** L'update di risoluzione è quello **unico** con
  `prev_publish_time < resolution_time <= publish_time` — il primo update
  pubblicato a-o-dopo `resolution_time` per quel feed (semantica Pyth, vedi
  `docs/pyth-reference.md` §2). `resolve_event` **verifica** questa
  disuguaglianza on-chain, quindi l'esito non dipende più da chi sfida chi:
  il risolutore non ha discrezione, o l'update che invia è quello canonico
  o viene rifiutato. Se il feed ha un buco all'istante `resolution_time`
  (l'update canonico è più vecchio di `MAX_RESOLUTION_STALENESS_SECS`),
  l'esito è AMBIGUO → REFUNDABLE. Questa è la variante *verificabile*
  dell'intento originale della spec ("ultimo prima di T" diventa "primo a/
  dopo T"): la vecchia formulazione non era garantita on-chain, era una
  mitigazione a sfida dove ora c'è una verifica.
- **I11 — `Σ pagamenti <= payout_pool` per costruzione.** Non è un
  controllo a runtime: è una proprietà matematica della divisione floor in
  `math::pro_rata_share` (`u128` intermedio, `floor(payout_pool * stake /
  totale)`), verificata come proprietà nei fixture di
  `tests/fixtures/payout.json` e `refund.json` (Task B).
- **I12 — Nessun esito `Some` se `[price - conf, price + conf]` attraversa
  la soglia.** `math::resolve_confidence_band` restituisce `None`
  (AMBIGUO) in ogni caso che non soddisfi esplicitamente la condizione YES
  o esplicitamente la condizione NO — non c'è un default silenzioso. La
  normalizzazione di `conf` arrotonda **per eccesso** (ceiling,
  `math::normalize_conf_to_e8`), mentre il prezzo tronca (formula letterale
  della spec): l'errore di arrotondamento può quindi solo *allargare* la
  banda, mai restringerla — un caso di frontiera può degradare verso
  AMBIGUO, mai promuoversi a esito definito. Verificato anche come
  proprietà su terne pseudo-casuali nei test inline di `math.rs`.
- **I13 — (ritirata, 2026-08-30).** Descriveva "nessun pagamento ai
  partecipanti prima di `finalized_at`". Con la rimozione della fase di
  sfida `resolve_event` va direttamente a uno stato terminale: non esiste
  più una finestra pre-finalize da proteggere, quindi `finalized_at` non
  esiste e l'invariante è priva di oggetto. Il pagamento è gated
  semplicemente da `status == Resolved`/`Refundable`, cioè dal fatto che
  `resolve_event` (o `mark_refundable`/`lock_event`) sia già avvenuto.
  Numero mantenuto ritirato per non rinumerare.

## Principio di arrotondamento

> Ogni arrotondamento che agisce su una **soglia di rifiuto** arrotonda
> nella direzione che rende il rifiuto più probabile. Ogni arrotondamento
> che agisce su un **pagamento** arrotonda verso il basso, perché è ciò
> che rende `Σ pagamenti ≤ payout_pool` vera per costruzione.

Le due regole sembrano opposte e non lo sono: entrambe scelgono la
direzione che non può causare una perdita. Applicazioni correnti:

- `math::normalize_conf_to_e8` — ceiling: la banda di confidenza può solo
  allargarsi per arrotondamento, mai restringersi (I12);
- `math::confidence_ratio_bps` — ceiling: un feed marginalmente troppo
  largo non passa mai il gate `CONF_MAX_RATIO_BPS` per arrotondamento;
- `math::pro_rata_share` e la protocol fee — floor: `Σ pagamenti ≤
  payout_pool` per costruzione (I11);
- `math::normalize_price_to_e8` — troncatura, formula letterale della
  spec (il prezzo non è né una soglia di rifiuto né un pagamento; la
  conservatività della banda è già garantita dal ceiling su `conf`).

Il principio vale per ogni arrotondamento futuro, Fase 3 inclusa.

## Registro error code rispetto alla lista della spec

`errors.rs` è la fonte di verità. Divergenze deliberate dalla lista di
error code della spec originale, registrate qui perché la lista congelata
al deploy sia quella vera:

- **Rinominato**: `OraclePriceNonPositive` (la spec usava questo nome; una
  prima stesura del codice lo chiamava `OracleInvalidPrice` — allineato).
- **Rimosso**: `NotWinningOutcome` — su un evento RESOLVED il `claim` di
  un perdente non è un errore ma il modo in cui chiude la posizione (quota
  zero, rent restituito); vedi I2.
- **Rimossi con il meccanismo di sfida (2026-08-30)**: `EventNotResolving`,
  `ChallengeWindowClosed`, `ChallengeNotNewer`, `FinalizeTooEarly` (lo
  stato `Resolving`, `challenge_resolution` e `finalize_resolution` non
  esistono più), e le due sostituite dalla regola di canonicità
  (`OraclePublishTimeInFuture`, `OraclePublishTimeTooOld` → sostituite da
  `OracleUpdateBeforeResolution` e `OracleNotFirstAfterResolution`).
- **Rimosso con `finalized_at` (2026-08-30)**: `ClaimBeforeFinalized`
  (I13 ritirata, nessuna finestra pre-finalize).
- **Aggiunti (necessari, non previsti dalla spec)**:
  - `OracleExponentOutOfRange` — un exponent che renderebbe `10^n` fuori
    da `i128` viene rifiutato invece di panicare;
  - `OracleUpdateBeforeResolution` / `OracleNotFirstAfterResolution` — le
    due metà della regola di canonicità (`prev_publish_time <
    resolution_time <= publish_time`);
  - `ResolutionWindowClosed` — limite superiore della finestra di
    risoluzione (B-2): dopo `resolution_time + RESOLUTION_TIMEOUT_SECS`
    solo il rimborso è possibile;
  - `ZeroWinningStake` / `ZeroPot` — denominatore nullo in claim/refund,
    con codici distinti perché operativamente distinti;
  - `ShareExceedsTotal` — guardia difensiva in `math::pro_rata_share`:
    uno share maggiore del totale restituirebbe più di `payout_pool`
    (violazione I11); oggi impossibile per costruzione a monte, rifiutato
    comunque per costruzione anche qui.

La lista ha ormai divergito abbastanza dalla spec (rimossi ~7 codici,
aggiunti ~6) da rendere la **riconciliazione unica contro `errors.rs`** a
fine Fase 1 un prerequisito di freeze esplicito — vedi `DEPLOY.md`.

## Superficie di attacco esplicitamente esclusa (fuori scope)

- Nessun admin, nessuna pausa, nessun upgrade authority "amichevole": una
  volta deployato, il programma è quello che è. Vedi `DEPLOY.md` per il
  percorso di burn dell'upgrade authority.
- Nessun token SPL, nessun order book, nessun AMM, nessun batch settlement,
  nessun merkle tree, nessun uso di `remaining_accounts` — nessuno di
  questi esiste nel codice, quindi nessuna delle classi di bug tipiche di
  quei pattern (validazione di `remaining_accounts`, drift merkle, ecc.) si
  applica qui.

## Rischi noti e non ancora chiusi (Fase 1)

- **Costanti di sviluppo (`_DEV`)**: `MIN_STAKE_LAMPORTS`, `MAX_STAKE_LAMPORTS`,
  `MAX_POT_LAMPORTS`, `FEE_WALLET`, `RESOLVER_REWARD`, `MIN_RESOLUTION_GAP_SECS`,
  `MAX_RESOLUTION_STALENESS_SECS`, `RESOLUTION_TIMEOUT_SECS`, `CONF_MAX_RATIO_BPS`,
  `FEED_WHITELIST` sono tutti placeholder non derivati da dati reali (vedi
  `constants.rs` e `DEVIATIONS.md`). Un deploy mainnet con questi valori
  sarebbe uno deploy con parametri economici e di sicurezza non
  ragionati — bloccato esplicitamente dal test `mainnet_constants_are_frozen`.
- **`oracle-mock` è l'unico percorso implementato.** Il percorso reale verso
  `PriceUpdateV2` (Fase 3) non esiste ancora; l'intero programma, così com'è,
  non può leggere un prezzo Pyth vero.
- **Nessun toolchain ha mai compilato questo codice.** Ogni dettaglio di
  API Anchor (nomi di metodi, forma dei vincoli, versione delle macro) è
  un'assunzione dichiarata nei commenti del codice, non un fatto verificato.

## Segnalazione di vulnerabilità

Il file `security.txt` (embedded via `solana-security-txt`, vedi
`DEPLOY.md`) sarà il canale di contatto formale una volta che il programma
sarà deployato. Fino ad allora, questo repository è una bozza pubblica: non
sono presenti fondi reali da mettere a rischio.
