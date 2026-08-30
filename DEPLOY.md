# Deploy — SOLCLASH-EVENTS

> **Stato:** questo documento descrive il *piano* di deploy dalla spec.
> Nessun deploy è mai avvenuto: non esiste ancora un `Cargo.toml`, un
> build funzionante, né un program ID reale. `declare_id!` in `lib.rs`
> contiene un placeholder esplicitamente marcato, non un indirizzo
> deployabile. Segui questo documento solo dopo che le Fasi 0-3 sono
> concluse e verdi.

## Prerequisiti (Fase 0, non ancora eseguita in questo ambiente)

- Toolchain risolto e congelato in `TOOLCHAIN.md` — **non presente in
  questo branch**: l'utente ha indicato di eseguire la Fase 0 in locale,
  non in questo ambiente (nessun accesso a crates.io/npmjs.org/Solana CLI
  qui). Non generare `TOOLCHAIN.md` finché quella fase non è stata
  effettivamente eseguita e verificata.
- Program ID reale, generato con `solana-keygen new`, sincronizzato nel
  codice con `anchor keys sync`. Il placeholder attuale in `lib.rs` non ha
  una chiave privata associata (è un hash, non una keypair) — va sostituito
  interamente, non riusato.
- `FEED_WHITELIST` popolato con i feed id reali di SOL/USD, BTC/USD,
  ETH/USD verificati dalla fonte Pyth ufficiale (non ancora fatto — vedi
  `docs/pyth-reference.md` §7 e `DEVIATIONS.md`).
- Tutte le costanti `_DEV` in `constants.rs` sostituite con valori reali,
  ragionati, e congelati sotto la feature `mainnet` (il test
  `mainnet_constants_are_frozen` deve passare).

## Verifica del build: `solana-verify`

Una volta che il programma compila in modo riproducibile:

1. Build riproducibile con `anchor build --verifiable` (o l'equivalente
   comando `solana-verify build`), così l'hash del `.so` pubblicato può
   essere ricalcolato da chiunque a partire dal sorgente su GitHub.
2. `solana-verify verify-from-repo` (o l'equivalente flusso corrente al
   momento del deploy — verificare la sintassi esatta sulla documentazione
   di `solana-verify` quando disponibile, non assunta qui) puntato a questo
   repository e al commit deployato, cosicché chiunque possa confermare che
   il bytecode on-chain corrisponde esattamente a questo sorgente pubblico.
3. Pubblicare l'hash di verifica risultante (nel `README.md` o in una
   release) cosicché non serva fidarsi della dichiarazione del deployer.

## `security.txt`

Il programma include (dipendenza diretta prevista: `solana-security-txt`,
vedi il vincolo sulle dipendenze nel prompt originale) un blocco
`security.txt` imbottito nel binario on-chain, con almeno:

- contatto per la segnalazione di vulnerabilità;
- URL di questo repository pubblico;
- policy di segnalazione (coordinata, tempo di embargo ragionevole prima
  della disclosure pubblica);
- hash del sorgente verificato (vedi `solana-verify` sopra), cosicché
  `security.txt` stesso serva anche da prova di quale versione del codice
  è effettivamente deployata.

## Upgrade authority: Squads v4 con timelock, poi burn

Il deploy iniziale **non** deve avvenire con una keypair singola come
upgrade authority. Percorso previsto:

1. **Deploy iniziale sotto un multisig Squads v4**, non sotto una singola
   chiave. Questo permette una finestra di correzione per bug critici
   scoperti subito dopo il deploy, senza il rischio di una singola chiave
   compromessa che potrebbe drenare o sostituire il programma
   arbitrariamente.
2. **Timelock sul multisig**: ogni upgrade proposto deve attraversare un
   periodo di preavviso pubblico (osservabile on-chain) prima di poter
   essere eseguito, cosicché un upgrade malevolo o un bug in un upgrade
   proposto sia visibile e contestabile prima di diventare effettivo.
3. **Burn dell'upgrade authority** dopo un periodo di stabilizzazione
   ragionevole (nessun bug critico osservato, il codice ha dimostrato di
   funzionare come da spec sotto carico reale). Da quel momento il
   programma è immutabile per costruzione — coerente con I9 ("il
   settlement non richiede alcuna autorità centrale") e con l'assenza
   dichiarata di qualunque funzione di pausa o admin nel codice stesso:
   bruciare l'authority chiude l'unico canale residuo (l'upgrade stesso)
   che altrimenti contraddirebbe quell'invariante nel tempo.

Non è specificato dal prompt originale per quanto tempo debba durare la
finestra di timelock prima del burn, né la composizione esatta del
multisig (soglia, membri) — vedi `DEVIATIONS.md`.

## Whitelist CPI

Il programma deve invocare, in tutto il suo ciclo di vita, **solo**:

- **System Program** (`11111111111111111111111111111111111111111`) — per
  `create_account` (via i vincoli `init` di Anchor) e per il trasferimento
  di stake in `place_bet` (`system_program::transfer`, l'unico
  trasferimento in entrata che passa per una CPI esplicita — ogni altro
  movimento di lamport è manipolazione diretta di un account posseduto dal
  programma stesso, vedi `instructions::transfer_from_pda`, non una CPI).
- **Pyth Receiver Program** (`PYTH_RECEIVER_PROGRAM`,
  `rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ`) — implicitamente, tramite
  il tipo `Account<'info, PriceUpdateV2>` che Anchor usa per leggere
  (non invocare) un account già scritto da quel programma. Fase 1 usa
  `oracle-mock` al suo posto: nessuna CPI verso il receiver esiste ancora
  nel codice.

**Nessun'altra CPI.** Nessun token SPL, nessun altro programma. Un
qualunque audit pre-mainnet dovrebbe includere una verifica statica che
nessuna terza dipendenza sia stata introdotta nel frattempo (vedi il
vincolo "minimizza le crate importate" nella spec originale) e che nessuna
nuova CPI compaia fuori da questi due programmi.

## Ordine di deploy (riassunto)

1. Fase 0 (locale, utente) → `TOOLCHAIN.md` reale.
2. Fase 1-2 completate e verdi in questo repository (bozza attuale, mai
   compilata) diventano codice reale, compilato, testato.
3. Fase 3: `oracle.rs` reale, fixture da devnet, feed id reali.
4. Costanti `_DEV` sostituite, `mainnet_constants_are_frozen` verde.
5. Deploy su devnet sotto Squads v4, verifica end-to-end con Pyth reale su
   devnet.
6. Deploy su mainnet-beta sotto Squads v4 con timelock.
7. `solana-verify` pubblicato, `security.txt` confermato on-chain.
8. Periodo di osservazione, poi burn dell'upgrade authority.
