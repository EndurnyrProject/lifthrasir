type progress = {
  loaded: int,
  total: int,
}

type useSpritePngBatchResult = {
  sprites: Map.t<string, SpritePng.spritePngResponse>,
  loading: bool,
  error: option<string>,
  progress: progress,
  refetch: unit => unit,
}

let getRequestKey = (request: SpritePng.spritePngRequest): string => {
  JSON.stringifyAny({
    "sprite_path": request.spritePath,
    "act_path": request.actPath,
    "action_index": request.actionIndex,
    "frame_index": request.frameIndex,
    "palette_path": request.palettePath,
    "scale": request.scale,
  })->Option.getOr("")
}

let useSpritePngBatch = (requests: array<SpritePng.spritePngRequest>): useSpritePngBatchResult => {
  let (sprites, setSprites) = React.useState(() => Map.make())
  let (loading, setLoading) = React.useState(() => false)
  let (error, setError) = React.useState(() => None)
  let (progress, setProgress) = React.useState(() => {loaded: 0, total: 0})

  let requestsKey =
    requests
    ->Array.map(req =>
      JSON.stringifyAny({
        "sprite_path": req.spritePath,
        "act_path": req.actPath,
        "action_index": req.actionIndex,
        "frame_index": req.frameIndex,
        "palette_path": req.palettePath,
        "scale": req.scale,
      })
    )
    ->JSON.stringifyAny
    ->Option.getOr("")

  let loadSprites = React.useCallback1(async () => {
    if Array.length(requests) == 0 {
      setSprites(_ => Map.make())
      setLoading(_ => false)
      setError(_ => None)
      setProgress(_ => {loaded: 0, total: 0})
    } else {

    setLoading(_ => true)
    setError(_ => None)
    setProgress(_ => {loaded: 0, total: Array.length(requests)})

    let newSprites = ref(Map.make())
    let loadedCount = ref(0)
    let hasError = ref(false)

    let loadPromises =
      requests->Array.map(async request => {
        try {
          let response = await SpritePng.getSpritePng(request)
          let key = getRequestKey(request)
          let _ = newSprites.contents->Map.set(key, response)
        } catch {
        | _ => hasError := true
        }
        loadedCount := loadedCount.contents + 1
        setProgress(_ => {loaded: loadedCount.contents, total: Array.length(requests)})
      })

    let _ = await loadPromises->Promise.all

    setSprites(_ => newSprites.contents)
    setLoading(_ => false)

    let spritesCount = newSprites.contents->Map.size
    if hasError.contents && spritesCount == 0 {
      setError(_ => Some("Failed to load all sprites"))
    } else if hasError.contents {
      let failedCount = Array.length(requests) - spritesCount
      setError(_ => Some(`Failed to load ${Int.toString(failedCount)} of ${Int.toString(Array.length(requests))} sprites`))
    }
    }
  }, [requestsKey])

  React.useEffect1(() => {
    loadSprites()->ignore
    None
  }, [loadSprites])

  let refetch = React.useCallback1(() => {
    loadSprites()->ignore
  }, [loadSprites])

  {sprites, loading, error, progress, refetch}
}
