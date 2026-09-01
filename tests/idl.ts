/**
 * IDL <-> Rust source cross-check.
 *
 * `anchor build` derives `target/idl/solclash_events.json` from the Rust
 * source, and everything on the TypeScript side of this project is built
 * on that file. This suite asserts the derived IDL still describes the
 * program the Rust source actually declares: same program id, same
 * instruction set, same error codes in the same order.
 *
 * It deliberately needs no validator and no deployed program — it reads
 * the build output and the `.rs` files off disk — so it stays runnable in
 * CI and in any sandbox, which is exactly where a silent IDL drift would
 * otherwise go unnoticed.
 *
 * Run with `yarn test` after `anchor build`.
 */
import { expect } from "chai";
import * as fs from "fs";
import * as path from "path";

const ROOT = path.resolve(__dirname, "..");
const SRC = path.join(ROOT, "programs/solclash_events/src");
const IDL_PATH = path.join(ROOT, "target/idl/solclash_events.json");

interface Idl {
  address: string;
  metadata: { name: string; version: string; description?: string };
  instructions: { name: string }[];
  accounts: { name: string }[];
  errors: { code: number; name: string; msg: string }[];
  types: { name: string }[];
}

function readIdl(): Idl {
  if (!fs.existsSync(IDL_PATH)) {
    throw new Error(
      `${IDL_PATH} is missing — run \`anchor build\` before \`yarn test\`.`,
    );
  }
  return JSON.parse(fs.readFileSync(IDL_PATH, "utf8")) as Idl;
}

function readSource(file: string): string {
  return fs.readFileSync(path.join(SRC, file), "utf8");
}

describe("IDL matches the Rust source", () => {
  const idl = readIdl();

  it("carries the program id from declare_id!", () => {
    const lib = readSource("lib.rs");
    const match = lib.match(/declare_id!\("([^"]+)"\)/);
    expect(match, "declare_id! not found in lib.rs").to.not.be.null;
    expect(idl.address).to.equal(match![1]);
  });

  it("exposes exactly the eleven instructions of the state machine", () => {
    // Sorted, so this compares the *set* rather than anchor's emission
    // order, which is not part of the program's contract.
    expect([...idl.instructions.map((i) => i.name)].sort()).to.deep.equal(
      [
        "cancel_bet",
        "challenge_resolution",
        "claim",
        "claim_refund",
        "close_event",
        "create_event",
        "finalize_resolution",
        "lock_event",
        "mark_refundable",
        "place_bet",
        "resolve_event",
      ].sort(),
    );
  });

  it("declares every instruction that lib.rs routes", () => {
    const lib = readSource("lib.rs");
    const program = lib.slice(lib.indexOf("pub mod solclash_events"));
    const routed = [...program.matchAll(/pub fn (\w+)\s*\(/g)].map((m) => m[1]);
    expect(routed.length).to.be.greaterThan(0);
    expect([...routed].sort()).to.deep.equal(
      [...idl.instructions.map((i) => i.name)].sort(),
    );
  });

  it("lists the error variants of errors.rs, in order, from code 6000", () => {
    const errors = readSource("errors.rs");
    const body = errors.slice(errors.indexOf("pub enum SolclashError"));
    // Variant names are the identifiers that directly follow a `#[msg(...)]`
    // attribute; that pairing is what anchor turns into an IDL error entry.
    const variants = [
      ...body.matchAll(/#\[msg\("[^"]*"\)\]\s*\n\s*(\w+),/g),
    ].map((m) => m[1]);

    expect(variants.length).to.equal(idl.errors.length);
    idl.errors.forEach((err, i) => {
      expect(err.name, `error #${i}`).to.equal(variants[i]);
      // Anchor numbers user errors from 6000 in declaration order; the
      // whole point of one variant per `require!` is that a client can map
      // a code back to a named invariant, so the numbering is a contract.
      expect(err.code, `error ${err.name}`).to.equal(6000 + i);
      expect(err.msg, `error ${err.name}`).to.be.a("string").and.not.empty;
    });
  });

  it("is an oracle-mock build, and says so", () => {
    // `oracle-mock` is a default feature in Fase 1, so `MockPriceUpdate`
    // is part of the shipped interface. When Fase 3 introduces the real
    // Pyth `PriceUpdateV2` path and flips that default, this expectation
    // must be revisited on purpose rather than drift silently.
    expect(idl.accounts.map((a) => a.name)).to.include("MockPriceUpdate");
    expect(idl.types.map((t) => t.name)).to.include("MockVerificationLevel");
  });

  it("names the accounts the program owns", () => {
    expect([...idl.accounts.map((a) => a.name)].sort()).to.deep.equal(
      ["BetEntry", "Event", "MockPriceUpdate"].sort(),
    );
  });
});
