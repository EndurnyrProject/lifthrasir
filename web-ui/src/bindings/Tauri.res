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
