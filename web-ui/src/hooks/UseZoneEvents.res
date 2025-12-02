type zoneEventCallbacks = {
  onZoneError: string => unit,
  onMapLoadingFailed: string => unit,
  onEnteringWorld: unit => unit,
}

let use = (callbacks: zoneEventCallbacks) => {
  let (zoneStatus, setZoneStatus) = React.useState(() => "Connecting to zone server...")

  React.useEffect0(() => {
    let unlistenRefs: array<ref<unit => unit>> = []

    let setup = async () => {
      let u1 = await Tauri.Event.listen("zone-connecting", event => {
        let mapName: string = event.payload["map_name"]->Option.getOr("unknown")
        Console.log2("[FRONTEND] Received 'zone-connecting' event for map:", mapName)
        setZoneStatus(_ => `Connecting to ${mapName}...`)
      })
      let _ = Array.push(unlistenRefs, ref(u1))

      let u2 = await Tauri.Event.listen("zone-connected", _ => {
        Console.log("[FRONTEND] Received 'zone-connected' event")
        setZoneStatus(_ => "Connected! Authenticating...")
      })
      let _ = Array.push(unlistenRefs, ref(u2))

      let u3 = await Tauri.Event.listen("zone-authenticated", event => {
        let spawnX: int = event.payload["spawn_x"]->Option.getOr(0)
        let spawnY: int = event.payload["spawn_y"]->Option.getOr(0)
        Console.log2("[FRONTEND] Received 'zone-authenticated' event - spawn at", (spawnX, spawnY))
        setZoneStatus(_ =>
          `Authenticated! Loading map at (${Int.toString(spawnX)}, ${Int.toString(spawnY)})...`
        )
      })
      let _ = Array.push(unlistenRefs, ref(u3))

      let u4 = await Tauri.Event.listen("map-loading", event => {
        let mapName: string = event.payload["map_name"]->Option.getOr("map")
        Console.log2("[FRONTEND] Received 'map-loading' event for map:", mapName)
        setZoneStatus(_ => `Loading ${mapName}...`)
      })
      let _ = Array.push(unlistenRefs, ref(u4))

      let u5 = await Tauri.Event.listen("map-loaded", event => {
        let mapName: string = event.payload["map_name"]->Option.getOr("map")
        Console.log2("[FRONTEND] Received 'map-loaded' event for map:", mapName)
        setZoneStatus(_ => `${mapName} loaded! Entering world...`)
      })
      let _ = Array.push(unlistenRefs, ref(u5))

      let u6 = await Tauri.Event.listen("entering-world", _ => {
        Console.log("[FRONTEND] Received 'entering-world' event")
        setZoneStatus(_ => "Entering world...")
        callbacks.onEnteringWorld()
      })
      let _ = Array.push(unlistenRefs, ref(u6))

      let u7 = await Tauri.Event.listen("zone-error", event => {
        let error: string = event.payload["error"]->Option.getOr("Connection failed")
        Console.error2("[FRONTEND] Received 'zone-error' event:", error)
        callbacks.onZoneError(error)
        setZoneStatus(_ => "Connecting to zone server...")
      })
      let _ = Array.push(unlistenRefs, ref(u7))

      let u8 = await Tauri.Event.listen("map-loading-failed", event => {
        let error: string = event.payload["error"]->Option.getOr("Map loading failed")
        Console.error2("[FRONTEND] Received 'map-loading-failed' event:", error)
        callbacks.onMapLoadingFailed(error)
        setZoneStatus(_ => "Connecting to zone server...")
      })
      let _ = Array.push(unlistenRefs, ref(u8))
    }

    setup()->ignore

    Some(() => {
      unlistenRefs->Array.forEach(uRef => uRef.contents())
    })
  })

  zoneStatus
}
