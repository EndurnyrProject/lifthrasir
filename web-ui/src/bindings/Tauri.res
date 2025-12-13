type unlistenFn = unit => unit

module Event = {
  type payload<'a> = {payload: 'a}

  @module("@tauri-apps/api/event")
  external listen: (string, payload<'a> => unit) => promise<unlistenFn> = "listen"
}

module Core = {
  @module("@tauri-apps/api/core")
  external invoke: (string, 'args) => promise<'result> = "invoke"

  @module("@tauri-apps/api/core")
  external invokeNoArgs: string => promise<'result> = "invoke"
}

module Dpi = {
  type physicalPosition
  type physicalSize

  @new @module("@tauri-apps/api/dpi")
  external makePhysicalPosition: (int, int) => physicalPosition = "PhysicalPosition"

  @new @module("@tauri-apps/api/dpi")
  external makePhysicalSize: (int, int) => physicalSize = "PhysicalSize"
}

module Window = {
  type physicalSize = {width: int, height: int}
  type physicalPosition = {x: int, y: int}

  type window

  @module("@tauri-apps/api/window")
  external getCurrentWindow: unit => window = "getCurrentWindow"

  @send external outerSize: window => promise<physicalSize> = "outerSize"

  @send external setSize: (window, Dpi.physicalSize) => promise<unit> = "setSize"

  @send external outerPosition: window => promise<physicalPosition> = "outerPosition"

  @send external setPosition: (window, Dpi.physicalPosition) => promise<unit> = "setPosition"

  @send external setDecorations: (window, bool) => promise<unit> = "setDecorations"

  @send external setIgnoreCursorEvents: (window, bool) => promise<unit> = "setIgnoreCursorEvents"

  @send external setFocus: window => promise<unit> = "setFocus"

  type effects = {effects: array<string>}
  @send external setEffects: (window, effects) => promise<unit> = "setEffects"
  @send external clearEffects: window => promise<unit> = "clearEffects"

  @send external setAlwaysOnTop: (window, bool) => promise<unit> = "setAlwaysOnTop"

  @send external setShadow: (window, bool) => promise<unit> = "setShadow"

  @send external setContentProtected: (window, bool) => promise<unit> = "setContentProtected"
}
