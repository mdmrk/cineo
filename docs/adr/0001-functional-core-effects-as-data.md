# 0001. Functional core with effects as data

- Status: Accepted
- Date: 2026-10-06

## Context
The app aggregates many concurrent addon requests, player events and persistence. Bugs in this area (stale responses, partial failures, ordering) are hard to test through real IO. stremio-core solves it with an Elm-style runtime: models `update(msg) -> Effects`, effects are boxed futures executed through a static generic `Env` trait, with `ConditionalSend` to support WASM (VERIFIED-REF: `src/runtime/`).

## Options considered
1. **Port stremio-core's runtime design** — proven, but brings generic `Env` through every type, futures-as-effects (hard to assert on), derive macros, WASM constraints we do not have.
2. **Async services calling IO directly** (typical tokio app) — simple at first; logic and IO interleave, tests need mocks for everything.
3. **Functional core, effects as data** — pure `update(state, input) -> (state, Vec<Effect>)`; `Effect` is an enum; shells execute effects with real IO and feed results back.

## Decision
Option 3. `cineo-core` is pure (no IO, no async, no clock: time is an input). State transitions return `Effect` values. Shells own the executor. The reducer structure is introduced in M2 (aggregation), not before; M1 uses plain functions.

## Consequences
- Core logic is tested by asserting on state and returned effects, no mocks.
- Shells contain a small, boring effect interpreter; it must stay free of business logic.
- Long-running interactions (player) are modeled as event streams translated to inputs.

## Rejected alternatives
- Option 1: complexity justified by WASM/multi-platform constraints we do not have (ADR-0002, GOALS.md).
- Option 2: makes partial-failure and stale-response behavior untestable without heavy mocking.
