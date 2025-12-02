@react.component
let make = (
  ~gender: CharacterSprites.gender,
  ~hairStyle: int,
  ~hairColor: int,
  ~jobClass: int=0,
  ~actionIndex: int=0,
  ~frameIndex: int=0,
  ~scale: float=2.0,
  ~className: option<string>=?,
  ~style: option<ReactDOM.style>=?,
) => {
  let bodySpritePath = CharacterSprites.getBodySpritePath(jobClass, gender)
  let hairSpritePath = CharacterSprites.getHairSpritePath(hairStyle, gender)
  let hairPalettePath = CharacterSprites.getHairPalettePath(hairStyle, gender, hairColor)

  let containerStyle = Styles.combine(
    style->Option.getOr(Styles.empty),
    Styles.make({
      "position": "relative",
      "display": "inline-block",
    }),
  )

  let bodyStyle = Styles.make({
    "position": "absolute",
    "top": 0,
    "left": 0,
    "display": "none",
  })

  let hairSpriteStyle = Styles.make({
    "position": "absolute",
    "top": 0,
    "left": 0,
    "zIndex": 1,
  })

  let genderStr = switch gender {
  | Female => "Female"
  | Male => "Male"
  }

  let spacerSize = 64.0 *. scale

  <div className={className->Option.getOr("")} style={containerStyle}>
    <SpriteImage
      spritePath={bodySpritePath}
      actionIndex
      frameIndex
      scale
      alt={`${genderStr} body`}
      style={bodyStyle}
    />
    <SpriteImage
      spritePath={hairSpritePath}
      palettePath=?{hairPalettePath}
      actionIndex
      frameIndex
      scale
      alt={`Hair style ${Int.toString(hairStyle)} color ${Int.toString(hairColor)}`}
      style={hairSpriteStyle}
    />
    <div
      style={Styles.make({
        "width": `${Float.toString(spacerSize)}px`,
        "height": `${Float.toString(spacerSize)}px`,
        "visibility": "hidden",
      })}
    />
  </div>
}
