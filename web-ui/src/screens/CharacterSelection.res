%%raw(`import './CharacterSelection.css'`)

type characterData = {
  @as("char_id") charId: int,
  name: string,
  @as("class") classId: int,
  @as("job_name") jobName: string,
  @as("body_sprite_path") bodySpritePath: string,
  @as("hair_sprite_path") hairSpritePath: string,
  @as("hair_palette_path") hairPalettePath: option<string>,
  @as("base_level") baseLevel: int,
  @as("job_level") jobLevel: int,
  @as("base_exp") baseExp: int,
  @as("job_exp") jobExp: int,
  hp: int,
  @as("max_hp") maxHp: int,
  sp: int,
  @as("max_sp") maxSp: int,
  zeny: int,
  str: int,
  agi: int,
  vit: int,
  @as("int") intStat: int,
  dex: int,
  luk: int,
  hair: int,
  @as("hair_color") hairColor: int,
  @as("clothes_color") clothesColor: int,
  weapon: int,
  shield: int,
  @as("head_top") headTop: int,
  @as("head_mid") headMid: int,
  @as("head_bottom") headBottom: int,
  robe: int,
  @as("last_map") lastMap: string,
  @as("delete_date") deleteDate: option<int>,
  sex: CharacterSprites.gender,
}

type characterListResponse = {
  success: bool,
  error: option<string>,
  characters: option<array<characterData>>,
}

type selectCharacterResponse = {
  success: bool,
  error: option<string>,
}

type screen = Loading | List | Creation

@react.component
let make = (~onCharacterSelected: unit => unit, ~onBackToServerSelection: unit => unit) => {
  let {slotWithCharUrl, slotNoCharUrl} = AssetsContext.useAssets()
  let (screen, setScreen) = React.useState(() => Loading)
  let (characters, setCharacters) = React.useState(() => [])
  let (selectedSlot, setSelectedSlot) = React.useState(() => None)
  let (creationSlot, setCreationSlot) = React.useState(() => 0)
  let (loading, setLoading) = React.useState(() => false)
  let (error, setError) = React.useState(() => None)

  React.useEffect0(() => {
    let loadCharacters = async () => {
      try {
        let result: characterListResponse = await Tauri.Core.invokeNoArgs("get_character_list")

        if result.success {
          switch result.characters {
          | Some(chars) => {
              setCharacters(_ => chars)
              setScreen(_ => List)
            }
          | None => {
              setError(_ => result.error->Option.orElse(Some("Failed to load characters")))
              setScreen(_ => List)
            }
          }
        } else {
          setError(_ => result.error->Option.orElse(Some("Failed to load characters")))
          setScreen(_ => List)
        }
      } catch {
      | err => {
          setError(_ =>
            Some(`Network error: ${JsExn.message(Obj.magic(err))->Option.getOr("Unknown")}`)
          )
          setScreen(_ => List)
        }
      }
    }

    loadCharacters()->ignore
    None
  })

  let handleCharacterSelect = (slot: int, character: option<characterData>) => {
    switch character {
    | None => {
        setCreationSlot(_ => slot)
        setScreen(_ => Creation)
      }
    | Some(_) => setSelectedSlot(_ => Some(slot))
    }
  }

  let handlePlayCharacter = async () => {
    switch selectedSlot {
    | None => ()
    | Some(slot) => {
        Console.log2("[FRONTEND] User clicked ENTER button for slot", slot)
        setLoading(_ => true)
        setError(_ => None)

        try {
          Console.log2("[FRONTEND] Invoking select_character command for slot", slot)
          let result: selectCharacterResponse = await Tauri.Core.invoke(
            "select_character",
            {"slot": slot},
          )
          Console.log2("[FRONTEND] Received response from select_character:", result)

          if result.success {
            Console.log("[FRONTEND] Character selection successful, transitioning to in_game screen")
            onCharacterSelected()
          } else {
            Console.error2("[FRONTEND] Character selection failed:", result.error)
            setError(_ => result.error->Option.orElse(Some("Character selection failed")))
          }
        } catch {
        | err => {
            Console.error2("[FRONTEND] Network error during character selection:", err)
            setError(_ =>
              Some(`Network error: ${JsExn.message(Obj.magic(err))->Option.getOr("Unknown")}`)
            )
          }
        }

        setLoading(_ => false)
      }
    }
  }

  let handleCharacterCreated = async () => {
    try {
      let result: characterListResponse = await Tauri.Core.invokeNoArgs("get_character_list")

      if result.success {
        switch result.characters {
        | Some(chars) => setCharacters(_ => chars)
        | None => ()
        }
      }
    } catch {
    | _ => setError(_ => Some("Failed to reload character list"))
    }

    setScreen(_ => List)
  }

  let getSlotBackgroundImage = (character: option<characterData>): option<string> => {
    switch character {
    | Some(_) => slotWithCharUrl
    | None => slotNoCharUrl
    }
  }

  switch screen {
  | Loading =>
    <div className="character-selection-container">
      <div className="character-selection-box">
        <h1> {React.string("Loading Characters...")} </h1>
      </div>
    </div>
  | Creation =>
    <CharacterCreation
      selectedSlot={creationSlot}
      onCharacterCreated={() => handleCharacterCreated()->ignore}
      onCancel={() => setScreen(_ => List)}
    />
  | List =>
    <div
      className="character-selection-container"
      style={Styles.make({"background": "transparent"})}>
      <div
        className="character-selection-box"
        style={Styles.make({"background": "transparent", "boxShadow": "none"})}>
        <h1 style={Styles.make({"display": "none"})}> {React.string("Select Character")} </h1>
        <div className="character-grid">
          {Array.fromInitializer(~length=8, i => i)
          ->Array.map(index => {
            let character = characters->Array.get(index)
            let isSelected = selectedSlot === Some(index)
            let isEmpty = Option.isNone(character)
            let backgroundImage = getSlotBackgroundImage(character)

            let cardClassName =
              "character-card" ++
              (if isSelected {" selected"} else {""}) ++
              (if isEmpty {" empty"} else {""})

            let cardStyle = switch backgroundImage {
            | Some(url) =>
              Styles.make({
                "backgroundImage": `url(${url})`,
                "backgroundSize": "contain",
                "backgroundPosition": "center",
                "backgroundRepeat": "no-repeat",
              })
            | None => Styles.empty
            }

            <div
              key={Int.toString(index)}
              className={cardClassName}
              onClick={_ =>
                if Option.isSome(character) {
                  handleCharacterSelect(index, character)
                }}
              style={cardStyle}>
              {switch character {
              | Some(char) =>
                <>
                  <div className="character-sprite-container">
                    <SpriteImage
                      spritePath={char.bodySpritePath}
                      actionIndex={0}
                      frameIndex={0}
                      scale={1.5}
                      className="character-body-sprite"
                      alt={`${char.name} body`}
                      applyOffset={false}
                    />
                    <SpriteImage
                      spritePath={char.hairSpritePath}
                      actionIndex={0}
                      frameIndex={0}
                      palettePath=?{char.hairPalettePath}
                      scale={1.5}
                      className="character-hair-sprite"
                      alt={`${char.name} hair`}
                    />
                  </div>
                  <div className="character-info">
                    <div className="character-name"> {React.string(char.name)} </div>
                    <div className="character-level">
                      {React.string(
                        `Lv. ${Int.toString(char.baseLevel)} / ${Int.toString(char.jobLevel)}`,
                      )}
                    </div>
                    <div className="character-class"> {React.string(char.jobName)} </div>
                  </div>
                </>
              | None =>
                <button
                  onClick={_ => handleCharacterSelect(index, None)}
                  className="create-char-button"
                  disabled={loading}>
                  {React.string("Create Character")}
                </button>
              }}
            </div>
          })
          ->React.array}
        </div>
        {switch error {
        | Some(msg) => <div className="error-message"> {React.string(msg)} </div>
        | None => React.null
        }}
        <div className="buttons-container">
          <button
            onClick={_ => onBackToServerSelection()} className="back-button" disabled={loading}>
            {React.string("Back to Server Selection")}
          </button>
          <button
            onClick={_ => handlePlayCharacter()->ignore}
            disabled={loading || Option.isNone(selectedSlot)}
            className="play-button">
            {React.string(if loading {"Entering..."} else {"Play"})}
          </button>
        </div>
      </div>
    </div>
  }
}
