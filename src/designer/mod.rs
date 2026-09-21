// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Mode 2: the designer's **generator** — a project document becomes Rust source (BLUE19 T-23).
//!
//! # The two modes this crate now has (D7)
//!
//! | Mode | Where | Cost | Gives |
//! |---|---|---|---|
//! | 1 | `crate::json` — interpret the document at run time | Links a parser, the name table, a layout kind table | Edit UI without recompiling |
//! | 2 | this module — generate Rust, compile it | A compile per UI change | No runtime interpreter, and it works on `mini`/`embedded`, where mode 1 does not compile at all |
//!
//! Both exist because they answer different questions. The design loop wants the first; shipping
//! wants the second. `ViewEngine::mount`'s `create` callback is the seam that makes them share one
//! runtime (BLUE19 §5.1.4) — this module generates a `View` and a `create`, mode 1 passes a
//! JSON-backed `create`, and nothing else in the program changes.
//!
//! # Where this module lives, and why it is gated
//!
//! The generator **parses** a project, so it needs `serde_json` and the parsed-project type, both of
//! which live behind `full_widgets`. Gating this module the same way is therefore not a choice about
//! importance — it is the module's actual dependency. The consequence, stated plainly:
//!
//! > **A `mini`/`embedded` device cannot run the generator. It is the *target* of one.**
//!
//! That is the honest division and it matches how the feature is used: a designer runs on a desktop
//! host, and `mini`/`embedded` receive the generated file. BLUE19 §5.3.4 says the same thing in
//! one line — "`mini`/`embedded` are 生成**目标**，不是运行**环境**".
//!
//! What is *not* assumed is the ability to construct a target's controls:
//! [`generator::availability`] **asks the factory** instead of consulting a list, and reports a
//! name it cannot resolve rather than emitting code that would fail to compile on the target.
//!
//! # Reachability
//!
//! **State:** Production callers: `tests/generator_output_compiles_test.rs:44`,
//! `tests/mode_consistency_test.rs:128`. Those two are the *delivered* consumers of this round: the
//! first compiles a generated program for real on five profiles, the second asserts the two modes
//! describe the same UI. Beyond them the generator's caller is the designer, which does not exist in
//! this repository yet — so this module is **an enabling API rather than a used one**, and saying so
//! is more honest than claiming a design tool that is not here.
//!
//! That is the same position `crate::json` was in before this round (`src/json/mod.rs` Reachability),
//! with one difference worth stating: the generator's output is **verified by compilation**, so
//! "unused" here does not mean "unchecked". `tools/check_generator_output_compiles.sh` builds what
//! this module emits for `desktop`, `tablet`, `mobile`, `mini` and `embedded`, and
//! `tools/check_mode_consistency.sh` fails if the emitted tree drifts from mode 1's.
//!
//! # Where generated files live (D7-b-3)
//!
//! Generated sources are **committed** and written through [`artifact::regenerate_into`];
//! `tools/check_generated_sources.sh` is the regenerate-and-compare gate that makes committing
//! safe. See [`artifact`] for the decision and its reasoning.
//!
//! # Scope
//!
//! [`generator::generate`] is a pure function: document text in, source text out, no filesystem and no
//! clock. That is what lets `tests/generator_output_compiles_test.rs` compile its output for real and
//! what makes two runs over one document byte-identical. Writing files is a separate concern with its
//! own failure modes, so it lives in [`artifact`] and the core stays pure.

pub mod artifact;
pub mod generator;

pub use artifact::{
    artifact_source, is_generated, plan_artifacts, regenerate_into, Artifact, ArtifactOutcome,
    ArtifactPaths, GENERATED_MARKER,
};
pub use generator::{
    availability, constructor_type_name, generate, is_style_only_property, is_wire_key,
    shared_wire_rule_count, Availability, GeneratedSource, GenerationGap, GenerationReport,
    GenerationRequest, TargetProfile, DEFAULT_CHILD_CAPACITY, MINI_CHILD_CAPACITY,
};
