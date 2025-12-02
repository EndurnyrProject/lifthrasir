open Tauri

type zoneConnectingPayload = {@as("map_name") mapName: string}
type zoneAuthenticatedPayload = {@as("spawn_x") spawnX: int, @as("spawn_y") spawnY: int}
type zoneErrorPayload = {error: string}
type mapLoadingPayload = {@as("map_name") mapName: string}
type mapLoadedPayload = {@as("map_name") mapName: string}
type mapLoadingFailedPayload = {error: string}

type chatMessagePayload = {gid: int, message: string}

type cursorType =
  | Default
  | Add
  | Attack
  | Impossible
  | Talk

type cursorChangePayload = {@as("cursor_type") cursorType: string}

type entityTooltipPayload = {
  name: string,
  party: option<string>,
  guild: option<string>,
  x: int,
  y: int,
}

let listenZoneConnecting = (callback: zoneConnectingPayload => unit): promise<unlistenFn> => {
  Event.listen("zone-connecting", event => callback(event.payload))
}

let listenZoneConnected = (callback: unit => unit): promise<unlistenFn> => {
  Event.listen("zone-connected", _ => callback())
}

let listenZoneAuthenticated = (callback: zoneAuthenticatedPayload => unit): promise<unlistenFn> => {
  Event.listen("zone-authenticated", event => callback(event.payload))
}

let listenZoneError = (callback: zoneErrorPayload => unit): promise<unlistenFn> => {
  Event.listen("zone-error", event => callback(event.payload))
}

let listenMapLoading = (callback: mapLoadingPayload => unit): promise<unlistenFn> => {
  Event.listen("map-loading", event => callback(event.payload))
}

let listenMapLoaded = (callback: mapLoadedPayload => unit): promise<unlistenFn> => {
  Event.listen("map-loaded", event => callback(event.payload))
}

let listenMapLoadingFailed = (callback: mapLoadingFailedPayload => unit): promise<unlistenFn> => {
  Event.listen("map-loading-failed", event => callback(event.payload))
}

let listenEnteringWorld = (callback: unit => unit): promise<unlistenFn> => {
  Event.listen("entering-world", _ => callback())
}

let listenCharacterStatusUpdate = (
  callback: TauriCommands.characterStatus => unit,
): promise<unlistenFn> => {
  Event.listen("character-status-update", event => callback(event.payload))
}

let listenChatMessageReceived = (callback: chatMessagePayload => unit): promise<unlistenFn> => {
  Event.listen("chat-message-received", event => callback(event.payload))
}

let listenCursorChange = (callback: cursorChangePayload => unit): promise<unlistenFn> => {
  Event.listen("cursor-change", event => callback(event.payload))
}

let listenEntityNameShow = (callback: entityTooltipPayload => unit): promise<unlistenFn> => {
  Event.listen("entity-name-show", event => callback(event.payload))
}

let listenEntityNameHide = (callback: unit => unit): promise<unlistenFn> => {
  Event.listen("entity-name-hide", _ => callback())
}
