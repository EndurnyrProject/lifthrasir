%%raw(`import './CharacterCreation.css'`)

type hairstyleInfo = {
  id: int,
  @as("available_colors") availableColors: array<int>,
}

type hairstylesResponse = {
  success: bool,
  error: option<string>,
  hairstyles: option<array<hairstyleInfo>>,
}

type createCharacterRequest = {
  name: string,
  slot: int,
  @as("hair_style") hairStyle: int,
  @as("hair_color") hairColor: int,
  sex: CharacterSprites.gender,
}

type createCharacterResponse = {
  success: bool,
  error: option<string>,
}

@react.component
let make = (~selectedSlot: int, ~onCharacterCreated: unit => unit, ~onCancel: unit => unit) => {
  let (characterName, setCharacterName) = React.useState(() => "")
  let (selectedGender, setSelectedGender) = React.useState(() => CharacterSprites.Male)
  let (selectedHairStyle, setSelectedHairStyle) = React.useState(() => 1)
  let (selectedHairColor, setSelectedHairColor) = React.useState(() => 0)

  let (hairstyles, setHairstyles) = React.useState(() => [])
  let (availableColors, setAvailableColors) = React.useState(() => [0])
  let (loading, setLoading) = React.useState(() => false)
  let (error, setError) = React.useState(() => None)

  let assetsLoadedRef = React.useRef(false)

  let loadHairstyles = async (gender: CharacterSprites.gender) => {
    try {
      let result: hairstylesResponse = await Tauri.Core.invoke("get_hairstyles", {"gender": gender})

      if result.success {
        switch result.hairstyles {
        | Some(styles) => {
            setHairstyles(_ => styles)

            if Array.length(styles) > 0 {
              let firstStyle = styles->Array.getUnsafe(0)
              setSelectedHairStyle(_ => firstStyle.id)
              setAvailableColors(_ => firstStyle.availableColors)
              let firstColor = firstStyle.availableColors->Array.get(0)->Option.getOr(0)
              setSelectedHairColor(_ => firstColor)
            }
          }
        | None => setError(_ => Some("Failed to load hairstyles"))
        }
      } else {
        setError(_ => result.error->Option.orElse(Some("Failed to load hairstyles")))
      }
    } catch {
    | err =>
      setError(_ => Some(`Network error: ${JsExn.message(Obj.magic(err))->Option.getOr("Unknown")}`))
    }
  }

  React.useEffect0(() => {
    if !assetsLoadedRef.current {
      assetsLoadedRef.current = true

      let loadAssets = async () => {
        try {
          let _ = await loadHairstyles(selectedGender)
        } catch {
        | _ => setError(_ => Some("Failed to load assets"))
        }
      }

      loadAssets()->ignore
    }

    None
  })

  let handleGenderChange = async (gender: CharacterSprites.gender) => {
    setSelectedGender(_ => gender)
    setLoading(_ => true)
    let _ = await loadHairstyles(gender)
    setLoading(_ => false)
  }

  let handleHairstyleSelect = (styleInfo: hairstyleInfo) => {
    setSelectedHairStyle(_ => styleInfo.id)
    setAvailableColors(_ => styleInfo.availableColors)
    let newColor = styleInfo.availableColors->Array.get(0)->Option.getOr(0)
    setSelectedHairColor(_ => newColor)
  }

  let handleHairColorSelect = (color: int) => {
    setSelectedHairColor(_ => color)
  }

  let handleCreateCharacter = async () => {
    let trimmedName = String.trim(characterName)

    if trimmedName === "" {
      setError(_ => Some("Please enter a character name"))
    } else if String.length(trimmedName) < 4 {
      setError(_ => Some("Character name must be at least 4 characters"))
    } else {
      setLoading(_ => true)
      setError(_ => None)

      try {
        let request: createCharacterRequest = {
          name: trimmedName,
          slot: selectedSlot,
          hairStyle: selectedHairStyle,
          hairColor: selectedHairColor,
          sex: selectedGender,
        }
        let result: createCharacterResponse = await Tauri.Core.invoke(
          "create_character",
          {"request": request},
        )

        if result.success {
          onCharacterCreated()
        } else {
          setError(_ => result.error->Option.orElse(Some("Character creation failed")))
        }
      } catch {
      | err =>
        setError(_ => Some(`Network error: ${JsExn.message(Obj.magic(err))->Option.getOr("Unknown")}`))
      }

      setLoading(_ => false)
    }
  }

  if Array.length(hairstyles) === 0 && !loading && Option.isNone(error) {
    <div className="character-creation-container">
      <div className="customization-panel">
        <h1> {React.string("Loading...")} </h1>
      </div>
    </div>
  } else {
    let bodySpritePath = CharacterSprites.getBodySpritePath(0, selectedGender)
    let hairSpritePath = CharacterSprites.getHairSpritePath(selectedHairStyle, selectedGender)
    let hairPalettePath = CharacterSprites.getHairPalettePath(
      selectedHairStyle,
      selectedGender,
      selectedHairColor,
    )

    <div className="character-creation-container">
      <div className="character-preview-container">
        <div className="character-sprite-preview">
          <SpriteImage
            spritePath={bodySpritePath}
            actionIndex={0}
            frameIndex={0}
            scale={1.5}
            className="character-body-sprite"
            alt="Character body"
            applyOffset={false}
          />
          <SpriteImage
            spritePath={hairSpritePath}
            actionIndex={0}
            frameIndex={0}
            palettePath=?{hairPalettePath}
            scale={1.5}
            className="character-hair-sprite"
            alt="Character hair"
          />
        </div>
      </div>
      <div className="customization-panel">
        <h1> {React.string("Create Character")} </h1>
        <div className="input-group">
          <label htmlFor="char-name"> {React.string("Character Name")} </label>
          <input
            id="char-name"
            type_="text"
            value={characterName}
            onChange={e => {
              let value = ReactEvent.Form.target(e)["value"]
              setCharacterName(_ => value)
            }}
            maxLength={23}
            placeholder="Enter character name"
            disabled={loading}
          />
          <span className="input-hint"> {React.string("4-23 characters, alphanumeric only")} </span>
        </div>
        <div className="gender-selection">
          <label> {React.string("Gender")} </label>
          <div className="gender-buttons">
            <button
              onClick={_ => handleGenderChange(Male)->ignore}
              className={if selectedGender === Male {"selected"} else {""}}
              disabled={loading}>
              {React.string("Male")}
            </button>
            <button
              onClick={_ => handleGenderChange(Female)->ignore}
              className={if selectedGender === Female {"selected"} else {""}}
              disabled={loading}>
              {React.string("Female")}
            </button>
          </div>
        </div>
        <div className="hairstyle-selection">
          <label> {React.string("Hairstyle")} </label>
          <div className="hairstyle-grid">
            {hairstyles
            ->Array.map(style => {
              let isSelected = selectedHairStyle === style.id
              <button
                key={Int.toString(style.id)}
                onClick={_ => handleHairstyleSelect(style)}
                className={`hairstyle-item${if isSelected {" selected"} else {""}}`}
                disabled={loading}>
                <span className="hairstyle-id">
                  {React.string(`#${Int.toString(style.id)}`)}
                </span>
              </button>
            })
            ->React.array}
          </div>
        </div>
        {if Array.length(availableColors) > 1 {
          <div className="hair-color-selection">
            <label> {React.string("Hair Color")} </label>
            <div className="color-grid">
              {availableColors
              ->Array.map(color => {
                let isSelected = selectedHairColor === color
                <button
                  key={Int.toString(color)}
                  onClick={_ => handleHairColorSelect(color)}
                  className={`color-item${if isSelected {" selected"} else {""}}`}
                  disabled={loading}>
                  <span className="color-id"> {React.string(`#${Int.toString(color)}`)} </span>
                </button>
              })
              ->React.array}
            </div>
          </div>
        } else {
          React.null
        }}
        {switch error {
        | Some(msg) => <div className="error-message"> {React.string(msg)} </div>
        | None => React.null
        }}
        <div className="buttons-container">
          <button onClick={_ => onCancel()} className="cancel-button" disabled={loading}>
            {React.string("Cancel")}
          </button>
          <button
            onClick={_ => handleCreateCharacter()->ignore}
            className="create-button"
            disabled={loading || String.trim(characterName) === ""}>
            {React.string(if loading {"Creating..."} else {"Create Character"})}
          </button>
        </div>
      </div>
    </div>
  }
}
