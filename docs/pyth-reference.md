# Riferimento sorgente: `pyth-crosschain`

> Documento di sola estrazione: nessuna riga qui sotto è stata scritta a
> memoria. Ogni affermazione riporta `file:riga` sul commit indicato. Dove
> il sorgente clonato non contiene l'informazione richiesta, è scritto
> esplicitamente "non presente nel sorgente".

**Repository:** `https://github.com/pyth-network/pyth-crosschain`
**Commit:** `465e8dcb5592c57b4909a6cb933d58d6d6b50a43`
**Data del commit:** 2026-08-28 19:03:47 +0000
**Metodo:** `git clone --depth 1` via proxy (rete diretta a `mcp.solana.com`,
`crates.io`, `npmjs.org` bloccata in questo ambiente; GitHub raggiungibile).
Questo documento è stato scritto senza toolchain Solana/Anchor: nessuna
delle affermazioni qui sotto è stata verificata compilando o eseguendo
codice, solo leggendo il sorgente testualmente.

---

## 1. `PriceUpdateV2` — definizione esatta e offset byte per byte

File: `target_chains/solana/pyth_solana_receiver_sdk/src/price_update.rs:49-56`

```rust
#[account]
#[derive(BorshSchema)]
pub struct PriceUpdateV2 {
    pub write_authority: Pubkey,
    pub verification_level: VerificationLevel,
    pub price_message: PriceFeedMessage,
    pub posted_slot: u64,
}
```

Costante di lunghezza, `price_update.rs:58-60`:

```rust
impl PriceUpdateV2 {
    pub const LEN: usize = 8 + 32 + 2 + 32 + 8 + 8 + 4 + 8 + 8 + 8 + 8 + 8;
}
```

`8 + 32 + 2 + 32 + 8 + 8 + 4 + 8 + 8 + 8 + 8 + 8 = 134` byte.

Un test dedicato, `price_update.rs:331-335`, asserisce che questa costante
coincide con `PriceUpdateV2::DISCRIMINATOR.len() + get_packed_len::<PriceUpdateV2>()`:

```rust
#[test]
fn check_size() {
    let len = PriceUpdateV2::DISCRIMINATOR.len() + v1::get_packed_len::<PriceUpdateV2>();
    assert_eq!(len, PriceUpdateV2::LEN);
}
```

`get_packed_len` viene da `solana_borsh::v1` (dipendenza `solana-borsh`,
`price_update.rs:328`), un crate **esterno a questo repository**: il suo
sorgente non è vendorizzato qui e non è stato scaricato (rete a crates.io
bloccata in questo ambiente). **Non presente nel sorgente**: come
`get_packed_len` calcola la dimensione di un enum a varianti di lunghezza
diversa (vedi punto 1.1 sotto) non è verificabile da questo repo.

### 1.1 Perché l'offset di `verification_level` non è fisso — attenzione per il ramo C

`VerificationLevel` (`price_update.rs:18-25`) è un enum Borsh a varianti di
lunghezza diversa:

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone, PartialEq, BorshSchema, Debug)]
pub enum VerificationLevel {
    Partial {
        #[allow(unused)]
        num_signatures: u8,
    },
    Full,
}
```

Nessun `impl` manuale di `Serialize`/`Deserialize` per `VerificationLevel`
esiste in questo file (verificato per assenza: nessun'altra occorrenza di
`VerificationLevel` con un blocco `impl ... Serialize` in
`price_update.rs`). Con la codifica Borsh di default per un enum — 1 byte
di discriminante + i campi della sola variante scelta — la lunghezza
**serializzata effettiva** di questo campo è:

- **1 byte** se `Full` (discriminante `1`, nessun campo);
- **2 byte** se `Partial { num_signatures }` (discriminante `0` + 1 byte `u8`).

La costante `LEN = 134` assume il caso da 2 byte (il termine `2` nella
somma sopra), cioè la dimensione **massima**, non quella effettiva per ogni
istanza. Questo è coerente con il fatto che lo spazio di un account Solana
è allocato una volta a dimensione fissa e non si restringe quando viene
scritta una variante più corta — ma significa che **un offset fisso per
tutto ciò che segue `verification_level` è corretto solo se si conosce già
quale variante è stata scritta**, altrimenti bisogna leggere prima il byte
di discriminante a offset 40 e poi diramare.

Tabella offset (byte, da 0), **valida solo per `verification_level ==
Full`** (caso rilevante per questo programma, che per spec richiede
esplicitamente `VerificationLevel::Full`):

| Campo | Offset | Lunghezza | Tipo |
|---|---|---|---|
| discriminator Anchor | 0 | 8 | `[u8; 8]` |
| `write_authority` | 8 | 32 | `Pubkey` |
| `verification_level` (tag) | 40 | 1 | `u8` (`1` = Full) |
| `price_message.feed_id` | 41 | 32 | `[u8; 32]` |
| `price_message.price` | 73 | 8 | `i64` |
| `price_message.conf` | 81 | 8 | `u64` |
| `price_message.exponent` | 89 | 4 | `i32` |
| `price_message.publish_time` | 93 | 8 | `i64` |
| `price_message.prev_publish_time` | 101 | 8 | `i64` |
| `price_message.ema_price` | 109 | 8 | `i64` |
| `price_message.ema_conf` | 117 | 8 | `u64` |
| `posted_slot` | 125 | 8 | `u64` |
| totale dati significativi | — | **133** | — |

Se invece `verification_level == Partial { num_signatures }`, ogni offset
da `price_message.feed_id` in poi è **+1** rispetto alla tabella sopra
(perché il tag occupa 2 byte, non 1), e il totale dati significativi è 134.
Il byte 133 (l'ultimo dei 134 allocati) nel caso `Full` non fa parte del
messaggio serializzato: è spazio dell'account non scritto da questo campo
(tipicamente zero se l'account è stato azzerato alla creazione, ma questo
non è garantito da questo sorgente).

**Implicazione per il ramo C (deserializzazione manuale):** leggere prima
il byte 40 per determinare la variante di `verification_level`, poi
applicare l'offset corretto. Un layout fisso a 134 byte che assume sempre 2
byte per `verification_level` legge campi shiftati di un byte quando
l'update è `Full`.

---

## 2. `PriceFeedMessage`

File: `pythnet/pythnet_sdk/src/messages.rs:84-114`

```rust
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, BorshSchema)]
#[cfg_attr(feature = "solana-program", derive(AnchorSerialize, AnchorDeserialize))]
#[cfg_attr(
    not(feature = "solana-program"),
    derive(BorshSerialize, BorshDeserialize)
)]
pub struct PriceFeedMessage {
    pub feed_id: [u8; 32],
    pub price: i64,
    pub conf: u64,
    pub exponent: i32,
    pub publish_time: i64,
    pub prev_publish_time: i64,
    pub ema_price: i64,
    pub ema_conf: u64,
}
```

`prev_publish_time` è documentato (`messages.rs:99-101`) come: "per ogni
istante `t`, l'update unico è quello per cui `prev_publish_time < t <=
publish_time`" — coerente con la regola di risoluzione della spec
(`publish_time <= resolution_time`, ultimo update pubblicato prima di
`resolution_time`).

Tutti i campi hanno dimensione fissa (nessun enum, nessuna lunghezza
variabile): 32+8+8+4+8+8+8+8 = 84 byte, corrispondente esattamente ai
termini `32 + 8 + 8 + 4 + 8 + 8 + 8 + 8` nella formula di `PriceUpdateV2::LEN`
sopra.

`pyth_solana_receiver_sdk/src/price_update.rs:1` re-esporta questo tipo:
`pub use pythnet_sdk::messages::{FeedId, PriceFeedMessage};` — è lo stesso
tipo usato in `PriceUpdateV2.price_message`, non una copia.

---

## 3. `VerificationLevel` — semantica e confronto con `Full`

File: `pyth_solana_receiver_sdk/src/price_update.rs:7-25`

Commento di spec (`price_update.rs:7-17`, tradotto): gli update di prezzo
Pyth sono ponte-verso-le-altre-chain via Wormhole; verificare le firme di
due terzi dei guardiani è pesante per i limiti di dimensione delle
transazioni Solana, quindi è ammessa una verifica parziale. `Full` significa
che le firme di due terzi dei guardiani correnti sono state verificate;
`Partial` significa che solo `num_signatures` firme sono state verificate.
Il commento include un avviso esplicito: usare update parzialmente
verificati è pericoloso, abbassa la soglia di guardiani collusi necessaria
per produrre un update malevolo.

Confronto (`price_update.rs:27-41`):

```rust
impl VerificationLevel {
    pub fn gte(&self, other: VerificationLevel) -> bool {
        match self {
            VerificationLevel::Full => true,
            VerificationLevel::Partial { num_signatures } => match other {
                VerificationLevel::Full => false,
                VerificationLevel::Partial {
                    num_signatures: other_num_signatures,
                } => *num_signatures >= other_num_signatures,
            },
        }
    }
}
```

Nessun metodo `is_full()` o equivalente esiste su `VerificationLevel` in
questo file (verificato per assenza — solo `gte` è definito). Per asserire
"Full" esplicitamente, come richiesto dalla spec, il confronto corretto
sourced da questo file è:

```rust
self.verification_level == VerificationLevel::Full  // richiede PartialEq, derivato riga 18
```

oppure equivalentemente `self.verification_level.gte(VerificationLevel::Full)`
(vero solo se `self.verification_level` è a sua volta `Full`, per via del
primo braccio del `match` sopra).

`get_price_no_older_than` (`price_update.rs:279-291`) usa internamente
`get_price_no_older_than_with_custom_verification_level(..., VerificationLevel::Full)`,
che a sua volta chiama `self.verification_level.gte(verification_level)`
(`price_update.rs:242-245`) — questa è esattamente la funzione SDK che la
spec dice di **non** usare ("non usiamo `get_price_no_older_than`"),
perché non espone la sola asserzione "è Full" senza anche applicare la sua
propria logica di età massima (`maximum_age`), che questo programma non
vuole delegare all'SDK.

---

## 4. Versione di `anchor-lang`

`pyth-solana-receiver-sdk`'s `Cargo.toml` (`target_chains/solana/pyth_solana_receiver_sdk/Cargo.toml:18`):

```toml
anchor-lang = { workspace = true }
```

Risolto nel workspace root, `target_chains/solana/Cargo.toml:31`:

```toml
anchor-lang = "1.0.2"
```

**`pyth-solana-receiver-sdk` è pinnato ad `anchor-lang = "1.0.2"`** su
questo commit. Il crate stesso è alla versione `2.0.0`
(`target_chains/solana/pyth_solana_receiver_sdk/Cargo.toml:3`, non
riportato sopra ma letto dallo stesso file).

> **Confermato in Fase 0** contro l'indice di crates.io: la 2.0.0 pubblicata
> richiede `anchor-lang ^1.0.2`, compatibile con la 1.1.2 che questo
> workspace usa. È l'unica linea che lo fa: la 1.2.0 e precedenti pinnano
> `anchor-lang ^0.32.1`, e le 0.6.x `>=0.28.0` con `solana-program 1.x`.
> Vedi `TOOLCHAIN.md`.

---

## 5. Discriminator Anchor di `PriceUpdateV2`

**Non presente nel sorgente [pyth-crosschain].** Il codice usa
`PriceUpdateV2::DISCRIMINATOR` come slice (`price_update.rs:333`,
`.len()`), che dimostra che l'accessor esiste (generato dalla macro
`#[account]` di `anchor-lang`), ma non ne stampa né asserisce mai il
valore concreto in questo repository. L'IDL JSON generato,
`target_chains/solana/sdk/js/pyth_solana_receiver/src/idl/pyth_solana_receiver.json`,
elenca gli account (`Config`, `priceUpdateV2`, `twapUpdate`) senza un campo
`discriminator` esplicito per nessuno di essi — è un IDL in formato
pre-0.30 style, che non serializza i discriminator come dati statici.

Il valore effettivo è calcolato a build-time dalla macro `#[account]` di
`anchor-lang` 1.0.2, il cui sorgente non è in questo repository (crate
esterno, non vendorizzato, non scaricabile in questo ambiente: rete a
crates.io bloccata). Non riportiamo qui un valore calcolato a mano (es. da
`sha256("account:PriceUpdateV2")[..8]`, la convenzione storica di Anchor
in versioni precedenti) perché non è stato possibile confermare che
`anchor-lang` 1.0.2 usi ancora quella stessa formula da questa fonte.

---

## 6. Accessor per `verification_level`

**Non esiste un accessor dedicato.** `verification_level` è un campo
pubblico diretto della struct (`price_update.rs:53`,
`pub verification_level: VerificationLevel,`), letto come
`price_update.verification_level`, non tramite un metodo. L'unico metodo
associato a `VerificationLevel` in questo file è `gte` (sezione 3 sopra);
non esiste in questo sorgente un metodo tipo `PriceUpdateV2::verification_level(&self)`
o `is_fully_verified(&self)`.

---

## 7. Cosa NON è coperto da questo documento

- Il costo (compute units) del posting `Full` via il receiver program: non
  misurato in questo passaggio (richiede esecuzione, non solo lettura del
  sorgente — rimandato alla Fase 3, quando un toolchain sarà disponibile).
- I feed id reali di SOL/USD, BTC/USD, ETH/USD: non recuperati (richiede
  accesso alla rete Pyth/pyth.network, bloccato in questo ambiente).
  `constants::FEED_WHITELIST_DEV` nel Fase 1 draft usa placeholder
  esplicitamente marcati, da sostituire in Fase 3.
- Fixture di byte reali da devnet (account `PriceUpdateV2` effettivamente
  postato): non raccolte, per lo stesso motivo di assenza di rete verso
  un RPC Solana.
