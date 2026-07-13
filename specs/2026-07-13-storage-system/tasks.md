# Storage System — Implementation Tasks

> Generated from architecture doc: `specs/2026-07-13-storage-system/architecture.md` (spec: `specs/2026-07-13-storage-system/spec.md`)
> Each task below is **one commit**. Implement top to bottom; respect `Depends on`.
> Tasks sharing a wave in `## Execution Waves` may be implemented in parallel.

**Goal:** Deliver server-authoritative account item storage from the existing Aesir protobufs through the neutral contract and engine state to a design-matched Bevy Feathers Bag/Vault window.

---

## Progress

- [x] Task 1: Define the protocol-neutral Storage contract
- [x] Task 2: Add authoritative Storage domain state
- [x] Task 3: Receive Storage protobuf events
- [x] Task 4: Send Storage protobuf requests
- [x] Task 5: Add the Storage UI interaction model
- [ ] Task 6: Render the Storage window
- [ ] Task 7: Wire transfers, amount prompting, and results

---

## Execution Waves

> Tasks in the same wave have no dependencies on each other and touch disjoint files — they can be implemented in parallel. Waves run in order; a wave starts only after the previous one is fully merged and green.

- Wave 1: Task 1
- Wave 2: Task 2, Task 3, Task 4
- Wave 3: Task 5
- Wave 4: Task 6
- Wave 5: Task 7

---

## Task 1: Define the protocol-neutral Storage contract

**What:** Add the Storage DTO, inbound events, outbound commands, and typed rejection reasons described in the architecture's “Protocol-neutral contract” section. Keep protobuf and transport types entirely outside `net-contract`, and preserve Storage indices and amounts as `u32`.

**Code pointers:**
- Create: `net-contract/src/dto/storage.rs` — define the protocol-neutral `StorageItem` and its complete field set.
- Create: `net-contract/src/events/storage.rs` — define `StorageOpened`, item deltas, `StorageResult`, and `StorageRejection`.
- Modify: `net-contract/src/dto/mod.rs` — export the Storage DTO.
- Modify: `net-contract/src/events/mod.rs` — export the Storage events.
- Modify: `net-contract/src/commands.rs` — add deposit, withdrawal, and close commands with automatic `NetContractPlugin` registration.
- Modify: `net-contract/src/lib.rs` — extend contract/plugin registration tests for every Storage message.
- Reference: `net-contract/src/dto/cart.rs` — existing neutral item DTO style.
- Reference: `net-contract/src/events/cart.rs` — existing inbound container messages and derives.
- Reference: `net-contract/src/commands.rs` — existing Cart commands and `auto_add_message` usage.

**Acceptance criteria:**
- [x] `StorageItem` contains `index`, `nameid`, `amount`, `type_`, `location`, `attribute`, `refine`, `look`, and `weight` as `u32`, `expire_time` as `u64`, `identified` as `bool`, and `cards` as `Vec<u32>`.
- [x] The contract exposes snapshot, add, remove, and typed result messages plus deposit, withdrawal, and close commands.
- [x] Every known rejection has a distinct enum variant and unknown numeric codes can be retained through `Unknown(i32)`.
- [x] Storage indices and amounts are never narrowed below `u32` in the new contract.
- [x] Plugin registration tests prove that all new Storage messages are registered.
- [x] `cargo test -p net-contract` passes.

**Depends on:** none

**Commit:** `feat(net-contract): add storage messages`

---

## Task 2: Add authoritative Storage domain state

**What:** Add the engine-owned Storage resource, its lifecycle and message-application systems, and shared RO item categorization. This implements the architecture's “Authoritative Storage domain” section without introducing a dependency on protobufs, QUIC, or `net-aesir`.

**Code pointers:**
- Create: `game-engine/src/domain/storage/mod.rs` — expose the Storage domain's focused public surface.
- Create: `game-engine/src/domain/storage/resource.rs` — implement the `Storage` resource over `BTreeMap<u32, StorageItem>`.
- Create: `game-engine/src/domain/storage/systems.rs` — apply snapshots/deltas, close locally, and reset on leaving gameplay.
- Create: `game-engine/src/domain/storage/plugin.rs` — initialize the resource, register systems, and enforce snapshot-before-delta-before-close ordering.
- Modify: `game-engine/src/domain/mod.rs` — export the Storage domain module.
- Modify: `game-engine/src/lib.rs` — re-export and install `StoragePlugin` next to Inventory and Cart.
- Modify: `game-engine/src/domain/inventory/item.rs` — expose `item_category(item_type: u32)` and delegate `Item::category` to it.
- Reference: `game-engine/src/domain/cart/resource.rs` — authoritative container resource operations.
- Reference: `game-engine/src/domain/cart/systems.rs` — neutral message application pattern.
- Reference: `game-engine/src/domain/cart/plugin.rs` — plugin and system-ordering pattern.
- Reference: `game-engine/src/domain/inventory/item.rs` — current item categorization rules.
- Reference: `game-engine/tests/no_transport_dep.rs` — transport-boundary regression test.

**Acceptance criteria:**
- [x] Opening Storage atomically replaces items and capacity and marks the resource open.
- [x] Added deltas upsert server-reported items; removals decrement stacks and delete entries at zero.
- [x] Deltas received while closed are ignored and cannot reopen or mutate Storage.
- [x] `CloseStorage` marks the resource closed locally after snapshot/delta application, while leaving the message available to other independent readers.
- [x] Leaving `GameState::InGame` clears items, capacity, and open state.
- [x] Iteration is deterministic by Storage index.
- [x] Existing Inventory categorization behavior remains unchanged after extracting `item_category`.
- [x] Unit/App tests cover replacement, capacity, upsert, decrement, zero removal, ordering, closed deltas, close, and reset.
- [x] `cargo test -p game-engine storage` and `cargo test -p game-engine --test no_transport_dep` pass.

**Depends on:** Task 1

**Commit:** `feat(game-engine): add storage domain state`

---

## Task 3: Receive Storage protobuf events

**What:** Translate the existing generated Storage snapshot, delta, and result protobuf bodies into neutral contract messages. Keep conversion logic pure where possible, preserve unknown result codes, and leave the generated protobuf file untouched.

**Code pointers:**
- Create: `net-aesir/src/zone/mapping/storage.rs` — map Storage item fields and every result code into neutral types.
- Create: `net-aesir/src/zone/flow/storage.rs` — drain matching incoming bodies into neutral Storage messages.
- Modify: `net-aesir/src/zone/mapping/mod.rs` — export the Storage mappings.
- Modify: `net-aesir/src/zone/flow/mod.rs` — export/register the Storage inbound flow.
- Reference: `net-aesir/src/zone/mapping/cart.rs` — field-by-field container mapping pattern.
- Reference: `net-aesir/src/zone/flow/cart.rs` — `IncomingMessage` draining and `auto_add_system` registration.
- Reference only: `net-aesir/src/proto/aesir.net.rs` — existing generated Storage bodies and result-code values; do not edit.

**Acceptance criteria:**
- [x] `StorageOpened`, `StorageItemAdded`, and `StorageItemRemoved` fields map exactly into neutral messages without index/amount narrowing.
- [x] Every known `StorageResultCode` maps to success or its matching `StorageRejection`.
- [x] An unknown numeric result becomes `StorageRejection::Unknown(code)`, is logged, and is never treated as success.
- [x] Each matching protobuf body emits exactly one corresponding neutral message; unrelated bodies emit none.
- [x] The generated `net-aesir/src/proto/aesir.net.rs` file is unchanged.
- [x] Mapping and flow tests cover snapshots, both deltas, all known results, an unknown result, and unrelated bodies.
- [x] `cargo test -p net-aesir storage` passes.

**Depends on:** Task 1

**Commit:** `feat(net-aesir): receive storage events`

---

## Task 4: Send Storage protobuf requests

**What:** Translate neutral deposit, withdrawal, and close commands into the existing generated protobuf request bodies and send them on the gameplay channel. Follow the current Cart phase guard and failure behavior.

**Code pointers:**
- Create: `net-aesir/src/send/storage.rs` — build and send Storage deposit, withdrawal, and close requests.
- Modify: `net-aesir/src/send/mod.rs` — export/register the Storage send systems.
- Reference: `net-aesir/src/send/cart.rs` — command readers, `ZonePhase::Playing` guard, gameplay channel, and send-error logging.
- Reference only: `net-aesir/src/proto/aesir.net.rs` — existing generated request bodies; do not edit.

**Acceptance criteria:**
- [x] Deposit requests carry the widened Bag inventory index and requested amount exactly.
- [x] Withdrawal requests carry the Storage index and requested amount exactly.
- [x] Close commands produce a `StorageCloseRequest` body.
- [x] Commands are cleared without network sends when disconnected or outside `ZonePhase::Playing`.
- [x] Send failures are logged and do not panic the application.
- [x] Builder and system tests cover all three request kinds plus phase gating.
- [x] The generated protobuf file remains unchanged and `cargo test -p net-aesir storage` passes.

**Depends on:** Task 1

**Commit:** `feat(net-aesir): send storage commands`

---

## Task 5: Add the Storage UI interaction model

**What:** Establish `StorageWindowPlugin`, presentation-only `StorageUi` state, and pure helpers for filtering, selection, double-click recognition, transfer intent, live amount validation, and rejection text. This commit deliberately stops before rendering the window so interaction rules can be tested independently.

**Code pointers:**
- Create: `lifthrasir-ui/src/widgets/storage_window/mod.rs` — define plugin/state types, shared interaction helpers, and focused unit tests.
- Modify: `lifthrasir-ui/src/widgets/mod.rs` — expose the new `storage_window` module without spawning its scene yet.
- Reference: `lifthrasir-ui/src/widgets/pushcart_window/mod.rs` — container-window presentation state and command emission pattern.
- Reference: `lifthrasir-ui/src/widgets/character_window/bag_tab.rs` — local timestamp-based double-click behavior.
- Reference: `game-engine/src/domain/inventory/item.rs` — shared category and equipped-item APIs.

**Acceptance criteria:**
- [x] `StorageUi` contains category, normalized query, Bag/Vault selection, pending transfer, awaiting-result flag, panel error, double-click state, and previous-open state only; authoritative items remain in engine resources.
- [x] Shared category/search projection applies the same case-insensitive predicate to Bag and Vault data and never mutates either source.
- [x] Equipped Bag items are excluded before category and search filtering.
- [x] Selection validation clears missing or filtered-out items.
- [x] A single transfer-intent helper distinguishes Bag deposits from Vault withdrawals and widens Bag indices losslessly.
- [x] Live amount validation accepts only `1..=available` and rejects empty, zero, non-numeric, excessive, or disappeared stacks.
- [x] Pure rejection-to-panel-message mapping covers all known variants and includes the numeric code for unknown variants.
- [x] Double-click timing and all pure helper behavior have focused tests.
- [x] `cargo test -p lifthrasir-ui storage_window` passes.

**Depends on:** Task 1, Task 2

**Commit:** `feat(ui): add storage interaction model`

---

## Task 6: Render the Storage window

**What:** Build the persistent Bevy Feathers Storage shell and dynamic pane/overlay hosts to match the supplied design. Render server-authoritative Bag/Vault projections, filters, search, capacity, controls, and empty/error regions while omitting Zeny and using only the title `Storage Vault`.

**Code pointers:**
- Create: `lifthrasir-ui/src/widgets/storage_window/scene.rs` — define BSN/`EntityScene` composition, view models, pane projections, and marked dynamic hosts.
- Modify: `lifthrasir-ui/src/widgets/storage_window/mod.rs` — install rendering/lifecycle systems and scene observers.
- Modify: `lifthrasir-ui/src/widgets/mod.rs` — register `StorageWindowPlugin` and spawn one hidden shell under the in-game HUD root.
- Reference: `lifthrasir-ui/src/widgets/pushcart_window/scene.rs` — Feathers container-window layout, scrolling, item cells, and chrome.
- Reference: `lifthrasir-ui/src/widgets/npc_dialog` — stable `EditableText` composition.
- Reference: `lifthrasir-ui/src/widgets/party/create_dialog.rs` — text-input state and observers.
- Reference: `lifthrasir-ui/src/focus.rs` — `EditableText`/`InputFocus` mirroring.
- Reference: `lifthrasir-ui/src/widgets/placeholder.rs` — input hint behavior.
- Reference: `designs/Endurnir Project/storage-window.jsx` — source layout and interactions, excluding Zeny and keeper identity.
- Reference: `designs/Endurnir Project/storage.css` — source visual details.
- Reference: `designs/Endurnir Project/screenshots/storage.png` — visual comparison target.

**Acceptance criteria:**
- [ ] `StorageOpened` causes the existing hidden shell to show; close or gameplay exit hides it and resets presentation state.
- [ ] The visible title/keeper chip says only `Storage Vault`; no Idunn/keeper identity or Zeny UI is rendered.
- [ ] The shell uses Bevy Feathers 0.19 controls, the Norse theme/chrome, existing item icons, and no new dependency.
- [ ] All/Use/Etc/Equip controls and one shared search field filter both panes.
- [ ] The search `EditableText` entity remains stable while pane contents rebuild, preserving focus/cursor state.
- [ ] Bag and Vault cells render deterministic item order, icons, names, quantities, refinement, and category; equipped Bag items remain hidden.
- [ ] Vault capacity renders as used slots over server-supplied capacity, and both panes render designed empty states.
- [ ] Directional buttons, per-cell quick-transfer controls, double-click hooks, red panel-error region, amount-overlay host, title close, and footer close are present with correct enabled/visible state.
- [ ] Quick-transfer child clicks stop pointer propagation so they do not also toggle cell selection.
- [ ] Either close control emits `CloseStorage`; authoritative engine state then hides the shell without optimistic item mutation.
- [ ] Scene/projection tests cover capacity, filtering, empty states, selection/disabled state, red-error styling, overlay visibility, and stable search identity.
- [ ] `cargo test -p lifthrasir-ui storage_window` passes.

**Depends on:** Task 5

**Commit:** `feat(ui): render storage window`

---

## Task 7: Wire transfers, amount prompting, and results

**What:** Connect every designed transfer affordance to the shared transfer path, implement stack amount entry and validation, and handle server outcomes without optimistic container changes. Finish the end-to-end lifecycle, error handling, and verification described by the architecture.

**Code pointers:**
- Modify: `lifthrasir-ui/src/widgets/storage_window/mod.rs` — wire observers/systems for transfer initiation, confirmation, cancellation, result handling, lifecycle reset, and system ordering.
- Modify: `lifthrasir-ui/src/widgets/storage_window/scene.rs` — bind transfer controls, numeric amount input, validation feedback, awaiting state, and red server errors.
- Reference: `lifthrasir-ui/src/widgets/pushcart_window/mod.rs` — neutral transfer command emission without UI-owned authoritative state.
- Reference: `lifthrasir-ui/src/widgets/character_window/bag_tab.rs` — double-click routing.
- Reference: `lifthrasir-ui/src/widgets/npc_dialog` — Bevy 0.19 `EditableText` observer flow.
- Reference: `lifthrasir-ui/src/widgets/party/create_dialog.rs` — editable input handling.

**Acceptance criteria:**
- [ ] Directional buttons, cell quick-transfer controls, and double-click all call the same `begin_transfer` behavior.
- [ ] A source stack of one immediately emits the correct neutral deposit/withdraw command; a larger stack opens the prompt initialized to `1`.
- [ ] The amount field uses `EditableTextFilter` to accept ASCII digits only, and confirmation revalidates against the live source stack.
- [ ] Invalid input stays in the prompt, shows red validation feedback, and emits no command; cancellation emits nothing.
- [ ] A valid confirmation emits the exact source index/amount and enters `awaiting_result`.
- [ ] While awaiting a result, all transfer affordances are disabled so the request/result association remains unambiguous.
- [ ] No UI observer or system mutates `Inventory` or `Storage`; visible moves occur only after server deltas update engine resources.
- [ ] A new transfer clears the old panel error; success clears awaiting/error state; each known rejection shows its readable red panel message.
- [ ] Unknown rejection codes are logged and shown in a generic red message containing the code.
- [ ] Results received while Storage is closed are ignored and cannot affect a later session.
- [ ] Closing or leaving gameplay cancels prompts and awaiting state; reopening starts from the new authoritative snapshot.
- [ ] UI rebuild runs after Inventory and Storage message application so a rendered frame does not expose a half-applied transfer.
- [ ] Tests cover every transfer affordance, single/stack paths, live validation, duplicate blocking, all result mappings, closed-session results, and server-authoritative rendering.
- [ ] Manual comparison against `designs/Endurnir Project/screenshots/storage.png` confirms the intended dual-pane layout, with Zeny omitted and the label exactly `Storage Vault`.
- [ ] Manual verification covers shared filters, both directions, cancellation, known rejections, both close controls, and reopen with a fresh snapshot.
- [ ] `cargo fmt --check`, targeted Storage tests in all four crates, and the full `cargo test` workspace suite pass.

**Depends on:** Task 3, Task 4, Task 6

**Commit:** `feat(ui): wire storage transfers`
