let yOffsetAdjustment = 4.5

@react.component
let make = (
  ~spritePath: string,
  ~actPath: option<string>=?,
  ~actionIndex: int=0,
  ~frameIndex: int=0,
  ~palettePath: option<string>=?,
  ~scale: float=1.0,
  ~className: option<string>=?,
  ~style: option<ReactDOM.style>=?,
  ~alt: string="Sprite",
  ~loadingPlaceholder: option<React.element>=?,
  ~errorPlaceholder: option<React.element>=?,
  ~applyOffset: bool=true,
) => {
  let {sprite, loading, error} = UseSpritePng.useSpritePng(
    Some({
      spritePath,
      actPath,
      actionIndex: Some(actionIndex),
      frameIndex: Some(frameIndex),
      palettePath,
      scale: Some(scale),
    }),
  )

  let loadingStyle = Styles.make({
    "display": "inline-flex",
    "alignItems": "center",
    "justifyContent": "center",
    "minWidth": "100px",
    "minHeight": "100px",
    "opacity": "0.5",
  })

  let errorStyle = Styles.make({
    "display": "inline-flex",
    "alignItems": "center",
    "justifyContent": "center",
    "minWidth": "100px",
    "minHeight": "100px",
    "opacity": "0.3",
  })

  if loading {
    switch loadingPlaceholder {
    | Some(placeholder) => placeholder
    | None =>
      <div
        className={className->Option.getOr("")}
        style={Styles.combine(style->Option.getOr(Styles.empty), loadingStyle)}>
        {React.string("Loading...")}
      </div>
    }
  } else if Option.isSome(error) {
    switch errorPlaceholder {
    | Some(placeholder) => placeholder
    | None =>
      <div
        className={className->Option.getOr("")}
        style={Styles.combine(style->Option.getOr(Styles.empty), errorStyle)}>
        {React.string("Error")}
      </div>
    }
  } else {
    switch sprite {
    | None => React.null
    | Some(s) => {
        let baseStyle = Styles.make({
          "width": `${Int.toString(s.width)}px`,
          "height": `${Int.toString(s.height)}px`,
        })

        let offsetStyle = if applyOffset {
          Styles.make({
            "marginLeft": `${Int.toString(s.offsetX)}px`,
            "marginTop": `${Float.toString(Int.toFloat(s.offsetY) +. yOffsetAdjustment)}px`,
          })
        } else {
          Styles.empty
        }

        let combinedStyle = Styles.combine(
          Styles.combine(style->Option.getOr(Styles.empty), baseStyle),
          offsetStyle,
        )

        <img src={s.dataUrl} alt className={className->Option.getOr("")} style={combinedStyle} />
      }
    }
  }
}
