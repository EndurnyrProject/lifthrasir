# Storage System Specification

## Summary

Lifthrasir will gain a player-facing account item-storage system backed by the existing Storage protocol. The feature will present a design-matched dual-pane Bag and Vault window where players can find items and transfer selected quantities while the server remains authoritative.

## Context & Problem

The client already has generated protobuf messages for opening storage, depositing and withdrawing items, closing storage, receiving item deltas, and reporting storage outcomes. It does not yet expose that capability to players through the protocol-neutral contract, game state, or native Bevy UI.

Without this feature, a server can offer account storage but the client cannot display or operate it. Players need a clear view of their carried inventory (the Bag), their account-level stored inventory (the Vault), the Vault's capacity, and the result of transfer attempts.

The supplied Storage design establishes the intended visual experience: Bag and Vault panes, shared category filters and search, item grids, directional transfer controls, and explicit close controls. Its stored-Zeny controls are excluded because the current protocol supports item storage only.

## Goals & Non-Goals

### Goals

- Show the Storage window when the server opens account storage.
- Match the supplied dual-pane Storage design using the project's existing visual language.
- Show Bag and Vault items with icons, names, quantities, refinement, categories, and Vault capacity.
- Hide equipped Bag items.
- Filter both panes with shared All, Use, Etc, and Equip categories and a shared text search.
- Support deposit and withdrawal through selection, directional buttons, cell quick-transfer controls, and double-click.
- Prompt for an amount when transferring a stackable item, initialized to `1` and constrained to the available quantity.
- Transfer single items immediately without an unnecessary amount prompt.
- Keep visible contents server-authoritative.
- Show server rejection messages in red inside the Storage panel.
- Close the window cleanly and notify the server.

### Non-Goals

- Stored-Zeny deposit, withdrawal, or display.
- Drag-and-drop transfers.
- User-controlled sorting.
- A new item-details experience.
- Changes to the existing protobuf schema.
- Support for another network protocol as part of this work.
- Other storage types such as guild or premium storage.

## Considered Options

### Complete design-matched item storage

Build the full item-storage experience represented by the supplied design: dual panes, category filters, search, capacity, all designed transfer affordances, amount prompting, and inline failure feedback. This was chosen because it delivers the intended experience without extending the protocol or adding unrelated features.

### Minimal transfer window

Build only two item lists, a quantity prompt, transfer buttons, and close behavior. This would reduce initial scope, but it was rejected because removing filters, search, and quick-transfer affordances would knowingly under-deliver the supplied design.

### Expanded storage management

Build the complete design plus sorting, drag-and-drop, item-detail redesign, and richer transfer feedback. This was rejected because those additions have no current requirement and would expand the feature beyond the existing design and protocol.

## Chosen Direction

When the server opens Storage, the client displays a window with Bag items on the left and account Vault items on the right. The title bar uses the neutral label `Storage Vault`; it does not invent a keeper identity that the protocol cannot provide. The Vault header shows used slots against the capacity supplied by the server. Equipped Bag items do not appear.

A shared category selector and case-insensitive search field filter both panes without changing their underlying contents. Selecting an item enables transfer in the appropriate direction. The directional transfer button, the cell's quick-transfer control, and double-click all begin the same transfer flow.

For a stack larger than one, the client prompts for an amount. The prompt starts at `1`, accepts only a value within the available source stack, and can be cancelled without sending a request. A single item transfers immediately.

After confirmation, the client requests the transfer without moving the item optimistically. Bag and Vault contents change only in response to server inventory and storage messages. A new transfer attempt clears any previous error. Successful server activity keeps the error cleared; a rejected outcome displays a readable red message inside the Storage panel.

Both the title-bar close control and footer close button hide the window and request server closure. Leaving gameplay clears the current Storage snapshot, selection, prompt, filters, search, and error so stale state cannot leak into another session. A later Storage-open event replaces any previous snapshot.

## Success Criteria

- A Storage-open event displays the window with the supplied capacity and Vault contents.
- Bag and Vault panes show the correct item icons, names, quantities, refinement, and category.
- Equipped Bag items are absent.
- Search and category filters affect both panes without mutating their underlying contents.
- Stack transfers require a valid amount between `1` and the available quantity.
- Single-item transfers do not show an amount prompt.
- Deposit and withdrawal requests carry the selected source index and amount.
- Items move onscreen only after matching server updates arrive.
- Storage-full, inventory-full, overweight, unstorable, equipped, invalid-amount, not-open, and skill-required outcomes produce readable red panel messages.
- A new transfer attempt and successful server activity clear the prior error.
- Both close controls hide the window and request server closure.
- Reopening Storage replaces stale state with the new server snapshot.
- Leaving gameplay clears all Storage UI and snapshot state.

## Constraints

- Use native Bevy UI and the project's existing visual language.
- Follow the supplied Storage design, excluding stored Zeny.
- Preserve the network boundary: UI and game-engine behavior depend only on `net-contract`; protobuf and transport handling remain in `net-aesir`.
- Use the existing Storage protobuf messages unchanged.
- Treat the server as authoritative for snapshots, deltas, capacity, and transfer outcomes.
- Reuse existing inventory item metadata and icons.
- Do not expose equipped items as deposit candidates.
- Keep this work limited to account item storage.

## Critique Findings

The critique pass identified two misleading parts of the original mockup. First, its Zeny footer implied functionality absent from the protocol, so the footer is removed rather than rendered inactive. Second, its `Idunn · Vault Keeper` title implied an NPC identity that `StorageOpened` does not provide. The approved design uses the neutral `Storage Vault` label instead.

The pass also checked whether quick-transfer actions could bypass the requested amount prompt. They cannot: all transfer affordances share the same behavior, prompting for stacks and transferring single items immediately.

No additional scope or security concerns were found. The server-authoritative rule prevents the UI from presenting an unconfirmed transfer as successful.

## Open Questions

None. The product behavior and scope are approved; technical structure, event flow, error mapping, and verification strategy belong in the architecture phase.
