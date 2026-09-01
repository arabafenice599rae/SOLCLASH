# TOOLCHAIN — verdetto della Fase 0

Questo file esiste perché la Fase 0 è stata eseguita davvero. Fino al
commit `86be53f` il repository conteneva solo sorgenti mai compilati e il
README rimandava la Fase 0 all'utente, in locale. Ora il toolchain esiste,
il programma compila, i test passano e il binario SBF si deploya. Quello
che segue è il verdetto, non un piano.

## Versioni verificate

Ogni versione qui sotto è stata letta da `--version` sulla macchina dove
la build è passata, non presa da una release note.

| Strumento | Versione | Provenienza |
|---|---|---|
| rustc | 1.94.1 (e408947bf 2026-03-25) | rustup, canale `stable` |
| cargo | 1.94.1 (29ea6fb6a 2026-03-24) | rustup, canale `stable` |
| rustup | 1.29.0 | preinstallato |
| Solana CLI (Agave) | 3.1.10 (src:7bc9c805) | `agave-install`, versione raccomandata da Anchor 1.1.2 |
| cargo-build-sbf | 3.1.10, platform-tools v1.52, rustc 1.89.0 | incluso nella Solana CLI |
| AVM | 1.1.2 | `cargo install --git https://github.com/coral-xyz/anchor avm --locked` |
| Anchor CLI | 1.1.2 | `avm install 1.1.2 --from-source` |
| anchor-lang (crate) | 1.1.2 | crates.io, unica dipendenza on-chain |
| Surfpool | 1.5.0 | binario di release GitHub (`surfpool-linux-x64.tar.gz`) |
| Node.js | 22.22.2 | preinstallato |
| npm | 10.9.7 | preinstallato |
| Yarn | 1.22.22 | preinstallato |
| Git | 2.43.0 | preinstallato |

`solana --version` riporta 3.1.10 e non l'ultima stabile perché è la
versione che Anchor 1.1.2 dichiara come raccomandata: con
`[toolchain] anchor_version = "1.1.2"` in `Anchor.toml`, `anchor` risolve e
attiva quella. Anche la 4.2.2 resta installata sotto `agave-install`, se
serve confrontare.

## Verdetto sulle dipendenze

- **`anchor-lang 1.1.2`** — compila. Due differenze rispetto alla 0.x che la
  bozza assumeva, entrambe risolte:
  - `CpiContext::new` prende il program **id** (`Pubkey`), non un
    `AccountInfo`. Era l'unico errore di compilazione dell'intera bozza.
  - `#[derive(PartialEq, Eq)]` impilato sopra `#[error_code]` funziona: le
    macro di Anchor preservano i derive, quindi `SolclashError` è
    confrontabile con `assert_eq!` nei test puri di `math.rs`, come la
    bozza sperava senza poterlo verificare.
  - `ctx.bumps.event` è corretto: è l'accessore Anchor post-0.29 ed è
    quello che la 1.x usa.
  - `#[account]`, `Account<'info, T>`, `close = ...`, `address = ... @ ...`,
    `seeds`/`bump`, `pubkey!`: tutti presenti e compilano come scritto.
- **`pyth-solana-receiver-sdk`** — **non ancora introdotto**, per scelta:
  la Fase 1 ha solo il percorso `oracle-mock`. La versione da usare quando
  arriverà la Fase 3 è la **2.0.0**, che è l'unica linea che pinna
  `anchor-lang ^1.0.2` — compatibile con la 1.1.2 usata qui. Tutte le
  versioni precedenti (1.2.0 e sotto) pinnano `anchor-lang ^0.32.1` o
  `>=0.28.0` con `solana-program` 1.x, e trascinerebbero indietro l'intero
  workspace. Vedi `docs/pyth-reference.md`.
- **`solana-security-txt`** — non ancora introdotto: nessun
  `security_txt!` è invocato nel sorgente. Va aggiunto insieme al blocco
  vero, non prima.

## Cosa è stato verificato, comando per comando

```
cargo build                     # pulito, zero warning
cargo clippy --all-targets      # pulito, zero warning
cargo fmt --check               # pulito
cargo test                      # 21/21 (17 unit + 4 fixture)
python3 tests/fixtures/generate_fixtures.py   # riproduce i 4 JSON byte per byte
anchor build                    # target/deploy/solclash_events.so, 280.608 byte
yarn install / typecheck / lint / test        # 6/6
surfpool start --offline --no-tui             # RPC risponde, gli slot avanzano
solana program deploy ...solclash_events.so   # il loader accetta l'ELF
```

Il deploy su Surfpool è la verifica che l'ELF SBF è valido e caricabile
dal `BPFLoaderUpgradeable`. **Non** è una verifica funzionale: vedi la
sezione sul program id qui sotto.

## Il program id è ancora un placeholder — di proposito

`declare_id!` in `lib.rs` contiene
`6aFse5Z9e6M97Hcro492hcb9b8sdkvZJ2zBHAGUdwBb1`, che è un hash
reinterpretato come `Pubkey`: **nessuna chiave privata esiste per quell
'indirizzo**. `anchor build` genera un keypair suo in
`target/deploy/solclash_events-keypair.json` (non versionato, come impone
il `.gitignore`) e segnala il disallineamento:

```
Program ID mismatch detected for program 'solclash_events':
  Keypair file has: ...
  Source code has:  6aFse5Z9e6M97Hcro492hcb9b8sdkvZJ2zBHAGUdwBb1
```

La build va comunque a termine. Il disallineamento **non è stato sanato**
con `anchor keys sync`, e la scelta è deliberata: sincronizzare
scriverebbe nel repository un indirizzo la cui chiave privata vive solo in
una `target/` non versionata, cioè un id che nessun altro può firmare e
che il repository lascerebbe intendere come proprio. Il placeholder dice
la verità; un id sincronizzato mentirebbe.

Conseguenza pratica: **un programma deployato così rifiuta ogni istruzione**
con `DeclaredProgramIdMismatch`. Prima di qualunque test on-chain
(Fase 2, LiteSVM o Surfpool) o di qualunque deploy serve:

```sh
solana-keygen new -o target/deploy/solclash_events-keypair.json
anchor keys sync      # riscrive declare_id! e Anchor.toml
```

e poi decidere consapevolmente se quell'id va versionato. Vedi `DEPLOY.md`.

## Feature Cargo

- `oracle-mock` è **di default**. La Fase 1 ha una sola sorgente di prezzo e
  `instructions/resolution.rs` nomina `MockPriceUpdate` senza gate: una
  build senza questa feature non compila. Di conseguenza `MockPriceUpdate`
  e `MockVerificationLevel` fanno parte dell'IDL pubblicato — `tests/idl.ts`
  lo asserisce esplicitamente, così la Fase 3 dovrà cambiarlo di proposito.
- `mainnet` è **volutamente non costruibile oggi**: collide con
  `oracle-mock` attraverso il `compile_error!` di `lib.rs`, e
  `--no-default-features --features mainnet` lascia `resolution.rs` senza
  alcun tipo di account per il price update. È la guardia che fa il suo
  lavoro. Il test `mainnet_constants_are_frozen` in `constants.rs` resta
  quindi non eseguibile finché la Fase 3 non introduce il percorso Pyth
  reale.

## Riprodurre questo ambiente da zero

```sh
# Rust (canale stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Solana CLI — anchor attiverà poi la versione che raccomanda
sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"

# AVM + Anchor
cargo install --git https://github.com/coral-xyz/anchor avm --locked --force
avm install 1.1.2 && avm use 1.1.2
export PATH="$HOME/.avm/bin:$PATH"

# Surfpool
curl -sSL -o surfpool.tar.gz \
  https://github.com/txtx/surfpool/releases/download/v1.5.0/surfpool-linux-x64.tar.gz
tar xzf surfpool.tar.gz && sudo install -m 755 surfpool /usr/local/bin/

# Dipendenze Node
yarn install
```

`avm install 1.1.2` scarica un binario precompilato e ne verifica la
*build provenance* contro `api.github.com`. In una rete che blocca quel
dominio la verifica fallisce e avm rifiuta l'installazione — correttamente,
va aggirata compilando, non saltando il controllo:

```sh
avm install 1.1.2 --from-source
```

## Cosa resta fuori da questa fase

La Fase 0 dice che il codice compila e che l'aritmetica pura è corretta
rispetto ai vettori di test. Non dice altro. In particolare **restano non
verificati**:

- ogni comportamento on-chain: nessun test LiteSVM o Surfpool esercita le
  11 istruzioni (Fase 2);
- il percorso Pyth reale, i feed id reali, le fixture da devnet (Fase 3);
- ogni costante `_DEV` in `constants.rs`, che resta un placeholder;
- l'invariante di escrow I7 e le altre di `SECURITY.md`, che sono
  argomentate nel codice ma non eseguite da un test;
- `Event::SPACE` e `BetEntry::SPACE`, calcolate a mano e mai confrontate
  con la dimensione borsh reale;
- qualunque forma di audit.

Il programma **non è deployabile e non va deployato**.
