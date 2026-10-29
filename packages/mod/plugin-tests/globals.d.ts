/**
 * The plugin's skill names, which `cargo xtask test` defines in each `claude plugin test` bundle
 * (crates/xtask/src/modgen.rs, `harness`) and never in the product bundle: the host stand-in
 * refuses a command named like one, as the host does (DR-84).
 */
declare const __PLANROOM_SKILLS__: readonly string[];
