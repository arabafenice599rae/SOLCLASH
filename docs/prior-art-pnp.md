# Prior art: PNP Protocol (Solana)

> Documento di sola estrazione fattuale, come `docs/pyth-reference.md` e
> `docs/toolchain-notes.md`. Nessun giudizio di merito, nessun confronto
> valutativo con SOLCLASH-EVENTS, nessuna riga di codice copiata — solo
> descrizione di come PNP è strutturato, con provenienza `file:riga`.
> Attenzione: questo repository è uno **skill per un client SDK** (`pnp-sdk`,
> pacchetto npm) che parla con un programma Solana già deployato, **non** il
> sorgente del programma on-chain di PNP. Ogni affermazione sotto descrive
> quello che il client espone/documenta, non il codice Rust del programma
> stesso, che non è in questo repository.

**Repository:** `https://github.com/pnp-protocol/solana-skill`
**Commit:** `cd43f8019b0966ca4c672d056cea416ab86a8180`
**Data del commit:** 2026-04-12 02:46:51 +0530
**Metodo:** `git clone` via proxy (GitHub raggiungibile in questo ambiente).

Program ID mainnet: `8PyE2dizL52ga7ytqLtqRyjwWp4yXEx8M5Z4BAHgHuTb`
(`SKILL.md:53`, `README.md:72`, `references/program-addresses.md:11`).

---

## 1. Struttura del mercato

PNP espone **tre architetture di mercato distinte**, tutte sotto lo stesso
program ID:

### 1.1 V2 AMM (mercato standard)

Mercato con market maker automatico "pAMM" (Prediction AMM) a liquidità
virtuale (`references/use-cases.md:132-139`). Alla creazione
(`market.createMarket`, `SKILL.md:254-268`) viene fornita una
`initialLiquidity` in un token collaterale SPL a scelta (`baseMint`), e il
programma conia due mint SPL separati per i token di esito YES e NO
(`account.yes_token_mint`, `account.no_token_mint`,
`SKILL.md:196-198`). Il prezzo è determinato da una formula a prodotto
costante sulle riserve/supply dei due token (`SKILL.md:219-224`):

```
yesPrice = (marketReserves * yesTokenSupply) / (yesTokenSupply^2 + noTokenSupply^2)
noPrice  = (marketReserves * noTokenSupply) / (yesTokenSupply^2 + noTokenSupply^2)
```

Gli utenti comprano/vendono i token YES/NO contro il collaterale
(`trading.buyTokensUsdc`/`trading.sellTokensBase`, `SKILL.md:416-442`) — è
quindi un mercato a **trading continuo con prezzo variabile per trade**,
non a stake fisso raccolto in un pot comune fino alla risoluzione.

### 1.2 P2P / V3 (scommessa diretta)

`createP2PMarketGeneral` (`SKILL.md:372-386`): il creatore prende un lato
("side": 'yes' o 'no') con un `initialAmount`, e fissa un
`creatorSideCap` — l'importo massimo che il creatore è disposto a
rischiare su quel lato. Il PDA di posizione per-utente-per-mercato ha seed
`["position", market, owner]` (`references/program-addresses.md:83-89`,
tabella riassuntiva riga `references/program-addresses.md:174`) — struttura
concettualmente analoga al nostro `BetEntry` PDA (seed
`["bet", event.key(), bettor.key()]`), ma qui il market PDA stesso è
derivato dai due mint YES/NO (`["market", yesMint, noMint]`,
`references/program-addresses.md:57-63`) o in alternativa da una coppia
base/quote mint (`["market", baseMint, quoteMint]`,
`references/program-addresses.md:70-76`), non da creatore+id numerico come
il nostro `Event` PDA.

### 1.3 Mercati social (Twitter/YouTube/DeFiLlama)

Varianti di creazione che associano automaticamente la domanda a un URL
esterno (`createMarketTwitter`, `createMarketYoutube`,
`createMarketDefiLlama`, `SKILL.md:322-366`) — stessa architettura V2 AMM
sottostante, solo con metadati aggiuntivi e rilevamento automatico
dell'URL nella domanda.

---

## 2. Gestione della risoluzione oracle

**Nessun oracolo on-chain basato su price feed (tipo Pyth) è documentato in
questo skill.** La risoluzione è un **flag booleano fornito da un wallet
designato**, non il risultato della lettura di un prezzo verificato
crittograficamente on-chain:

- **V2 AMM standard**: usa "PNP's global oracle" (`SKILL.md:54`,
  `references/api-reference.md:54`) — il testo non specifica ulteriormente
  chi controlli questo oracolo globale, oltre a dire che è "PNP's" (del
  protocollo stesso, non del creatore del singolo mercato).
- **Mercato a oracolo personalizzato** (`createMarketWithCustomOracle`,
  `SKILL.md:277-304`): un parametro esplicito `settlerAddress` designa il
  wallet che potrà risolvere quel mercato specifico — "Your agent's wallet
  becomes the oracle" (`SKILL.md:279`). Nessuna verifica crittografica di
  un prezzo esterno è coinvolta: è discrezione del wallet designato.
- La risoluzione vera e propria è una singola chiamata,
  `anchorMarket.settleMarket({market, yesWinner: boolean})`
  (`references/api-reference.md:427-434`, `SKILL.md:490-493`): l'esito è
  un booleano scelto dal chiamante, non calcolato dal programma stesso a
  partire da un input verificabile come un price update.
- Il valore di `yesWinner` è tipicamente ottenuto da un **servizio proxy
  esterno** che fornisce un suggerimento generato da un modello (non
  descritto ulteriormente nel codice di questo skill, che lo tratta come
  black box): `client.waitForSettlementCriteria(marketAddress)` restituisce
  `{resolvable, answer, criteria}` interrogando quel proxy ogni 2 secondi
  per fino a 15 minuti (`SKILL.md:481-495`); `fetchSettlementCriteria`
  restituisce `{category, reasoning, resolvable, resolution_sources,
  settlement_criteria}` e `fetchSettlementData` restituisce
  `{answer: 'YES'|'NO', reasoning}` (`SKILL.md:226-238`). Il codice
  cliente stesso commenta questo passaggio come "AI-suggested resolution"
  (`SKILL.md:481`).
- Errore documentato `Oracle mismatch — Wrong wallet trying to settle: Only
  designated oracle can settle` (`SKILL.md:534`) conferma che l'accesso a
  `settleMarket` è ristretto a un singolo wallet designato per mercato, non
  permissionless.

**Nessuna menzione, in nessuno dei file letti** (`SKILL.md`,
`README.md`, `references/api-reference.md`,
`references/program-addresses.md`, `references/use-cases.md`,
`references/types-and-precision.md`, `references/examples.md`,
`scripts/settle.ts`), di Pyth, di un price feed on-chain, o di una
verifica crittografica del prezzo di risoluzione. La risoluzione in PNP è
un atto discrezionale di un wallet autorizzato, non la lettura di un dato
oracolare verificabile da chiunque in modo indipendente.

---

## 3. Gap fra chiusura delle puntate e risoluzione

Lo state machine documentato (`SKILL.md:130-165`) mostra:

```
CREATED ──────► ACTIVE ──────► ENDED ──────► RESOLVED ──────► CLAIMED
```

- **ACTIVE → ENDED**: automatico, "Unix timestamp reaches `endTime`"
  (`SKILL.md:163`) — un solo timestamp (`endTime`) governa sia la fine del
  trading sia l'inizio della finestra in cui la risoluzione diventa
  possibile. **Non esistono due timestamp distinti** (l'equivalente dei
  nostri `betting_close_time`/`resolution_time`) nella documentazione
  letta.
- **ENDED → RESOLVED**: `settleMarket(...)`, "Oracle-only; can only be
  called after `endTime`" (`SKILL.md:164`). Nessun gap minimo è
  documentato fra `endTime` e la chiamata effettiva a `settleMarket`: lo
  script `scripts/settle.ts:140,164` verifica lato client solo
  `now >= endTime` (più `marketInfo.resolvable`), senza alcun margine
  ulteriore. Un oracolo può quindi, per quanto risulta da questa fonte,
  risolvere un mercato nell'istante immediatamente successivo a `endTime`.
- Per i **mercati a oracolo personalizzato** esiste un secondo gap, ma
  posizionato **prima** dell'apertura del trading, non fra chiusura e
  risoluzione: un buffer di **15 minuti** dopo la creazione entro cui va
  chiamato `setMarketResolvable(market, true)`, altrimenti il mercato resta
  "PERMANENTLY FROZEN" (`SKILL.md:146-156`, ripetuto come avviso esplicito
  in `SKILL.md:303-304` e come voce di errore in `SKILL.md:532`). Questo è
  concettualmente un gap di *attivazione*, non un gap fra chiusura delle
  puntate e risoluzione.

---

## 4. Come sceglie il prezzo di settlement

**Non sceglie un prezzo.** L'unità di risoluzione in PNP non è mai un
prezzo numerico: è un booleano YES/NO scelto dal wallet oracolo designato
(vedi §2), tipicamente informato da un suggerimento prodotto da un servizio
esterno ("AI-suggested resolution", `SKILL.md:481`) che non è parte del
codice on-chain né di questo skill repository. Non esiste, in questa
fonte, un concetto equivalente al nostro `publish_time <= resolution_time`
o a una finestra di validità (`PUBLISH_WINDOW_SECS`) per un dato di prezzo:
non essendoci un price feed on-chain coinvolto nel percorso di
risoluzione documentato qui, la nozione stessa non si applica a questa
architettura.

---

## 5. Altri dettagli fattuali raccolti (non richiesti esplicitamente ma
   attinenti)

- Program ID Devnet: `pnpkv2qnh4bfpGvTugGDSEhvZC7DP4pVxTuDykV3BGz`
  (`SKILL.md:54`). L'SDK sceglie automaticamente fra i due in base all'URL
  RPC (`references/program-addresses.md:14-16`).
- Collaterale: qualunque token SPL o Token-2022, non solo SOL/USDC
  (`SKILL.md:20`, `references/program-addresses.md:20-28`).
- Redenzione: `redeemPosition`/`redeemP2PPosition`, "available forever
  after resolution" (`SKILL.md:165`) — nessun analogo esplicito del nostro
  `close_event` (raccolta del residuo) è documentato in questo skill.
- Limiti di compute unit dichiarati per operazione (non verificati
  indipendentemente, solo riportati dalla documentazione):
  creazione mercato V2 ~400.000 CU, trading ~600.000 CU, settlement
  ~800.000 CU, `setMarketResolvable` ~800.000 CU
  (`SKILL.md:614-621`).
