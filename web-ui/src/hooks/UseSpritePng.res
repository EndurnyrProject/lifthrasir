type useSpritePngOptions = {
  spritePath: string,
  actPath: option<string>,
  actionIndex: option<int>,
  frameIndex: option<int>,
  palettePath: option<string>,
  scale: option<float>,
}

type useSpritePngResult = {
  sprite: option<SpritePng.spritePngResponse>,
  loading: bool,
  error: option<string>,
  refetch: unit => unit,
}

let useSpritePng = (request: option<useSpritePngOptions>): useSpritePngResult => {
  let (sprite, setSprite) = React.useState(() => None)
  let (loading, setLoading) = React.useState(() => false)
  let (error, setError) = React.useState(() => None)

  let requestKey =
    request->Option.map(req =>
      JSON.stringifyAny({
        "sprite_path": req.spritePath,
        "act_path": req.actPath,
        "action_index": req.actionIndex->Option.getOr(0),
        "frame_index": req.frameIndex->Option.getOr(0),
        "palette_path": req.palettePath,
        "scale": req.scale->Option.getOr(1.0),
      })->Option.getOr("")
    )

  let loadSprite = React.useCallback1(async () => {
    switch request {
    | None => {
        setSprite(_ => None)
        setLoading(_ => false)
        setError(_ => None)
      }
    | Some(req) => {
        setLoading(_ => true)
        setError(_ => None)

        try {
          let response = await SpritePng.getSpritePng({
            spritePath: req.spritePath,
            actPath: req.actPath,
            actionIndex: req.actionIndex->Option.getOr(0),
            frameIndex: req.frameIndex->Option.getOr(0),
            palettePath: req.palettePath,
            scale: req.scale->Option.getOr(1.0),
          })
          setSprite(_ => Some(response))
        } catch {
        | _ => {
            setError(_ => Some("Failed to load sprite"))
            setSprite(_ => None)
          }
        }

        setLoading(_ => false)
      }
    }
  }, [requestKey])

  React.useEffect1(() => {
    loadSprite()->ignore
    None
  }, [loadSprite])

  let refetch = React.useCallback1(() => {
    loadSprite()->ignore
  }, [loadSprite])

  {sprite, loading, error, refetch}
}
