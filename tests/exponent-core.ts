import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ExponentCore } from "../target/types/exponent_core";

describe("exponent-core", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.exponentCore as Program<ExponentCore>;

  it("", async () => {
    return true;
  });
});
