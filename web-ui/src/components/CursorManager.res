type cursorType = [#default | #add | #attack | #impossible | #talk]

type cursorChangeEvent = {@as("cursor_type") cursorType: cursorType}

type hotspot = {x: int, y: int}

let cursorHotspots: Map.t<cursorType, hotspot> = {
  let map = Map.make()
  let _ = map->Map.set(#default, {x: 17, y: 17})
  let _ = map->Map.set(#add, {x: 17, y: 17})
  let _ = map->Map.set(#attack, {x: 10, y: 5})
  let _ = map->Map.set(#impossible, {x: 17, y: 17})
  let _ = map->Map.set(#talk, {x: 17, y: 17})
  map
}

@val external document: {..} = "document"

@react.component
let make = () => {
  let assets = AssetsContext.useAssets()

  React.useEffect5(() => {
    let allCursorsLoaded =
      Option.isSome(assets.cursorDefaultUrl) &&
      Option.isSome(assets.cursorAddUrl) &&
      Option.isSome(assets.cursorAttackUrl) &&
      Option.isSome(assets.cursorImpossibleUrl) &&
      Option.isSome(assets.cursorTalkUrl)

    if !allCursorsLoaded {
      None
    } else {
      let cursorUrls = Map.make()
      let _ = cursorUrls->Map.set(#default, assets.cursorDefaultUrl->Option.getOr(""))
      let _ = cursorUrls->Map.set(#add, assets.cursorAddUrl->Option.getOr(""))
      let _ = cursorUrls->Map.set(#attack, assets.cursorAttackUrl->Option.getOr(""))
      let _ = cursorUrls->Map.set(#impossible, assets.cursorImpossibleUrl->Option.getOr(""))
      let _ = cursorUrls->Map.set(#talk, assets.cursorTalkUrl->Option.getOr(""))

      let updateCursor = (ct: cursorType) => {
        let url = cursorUrls->Map.get(ct)->Option.getOr("")
        let hotspot = cursorHotspots->Map.get(ct)->Option.getOr({x: 17, y: 17})

        if url === "" {
          Console.error2("[CursorManager] Missing cursor URL for type:", ct)
        } else {
          document["body"]["style"]["cursor"] = `url(${url}) ${Int.toString(hotspot.x)} ${Int.toString(hotspot.y)}, auto`
        }
      }

      let unlisten = ref(() => ())

      let setupListener = async () => {
        try {
          let unlistenFn = await Tauri.Event.listen("cursor-change", (event: Tauri.Event.payload<cursorChangeEvent>) => {
            updateCursor(event.payload.cursorType)
          })
          unlisten := unlistenFn

          updateCursor(#default)
        } catch {
        | err => Console.error2("[CursorManager] Failed to set up cursor-change listener:", err)
        }
      }

      setupListener()->ignore

      Some(
        () => {
          unlisten.contents()
          document["body"]["style"]["cursor"] = ""
        },
      )
    }
  }, (
    assets.cursorDefaultUrl,
    assets.cursorAddUrl,
    assets.cursorAttackUrl,
    assets.cursorImpossibleUrl,
    assets.cursorTalkUrl,
  ))

  React.null
}
