%%raw(`import "./styles/theme.css"`)
%%raw(`import "./App.css"`)

type appScreen = Login | ServerSelection | CharacterSelection | InGame

module AppContent = {
  @react.component
  let make = () => {
    let (currentScreen, setCurrentScreen) = React.useState(() => Login)
    let (isGameLoading, setIsGameLoading) = React.useState(() => false)
    let (servers, setServers): (
      array<Types.serverInfo>,
      (array<Types.serverInfo> => array<Types.serverInfo>) => unit,
    ) = React.useState(() => [])
    let {backgroundUrl} = AssetsContext.useAssets()

    let zoneStatus = UseZoneEvents.use({
      onZoneError: error => {
        DomBindings.alert(`Zone connection failed: ${error}`)
        setCurrentScreen(_ => CharacterSelection)
      },
      onMapLoadingFailed: error => {
        DomBindings.alert(`Map loading failed: ${error}`)
        setCurrentScreen(_ => CharacterSelection)
      },
      onEnteringWorld: () => {
        setIsGameLoading(_ => false)
      },
    })

    let isInputActive = currentScreen === InGame && !isGameLoading
    UseDevTools.use()
    UseKeyboardForwarding.use(~isActive=isInputActive)
    UseMouseForwarding.use(~isActive=isInputActive)
    UseCameraRotation.use(~isActive=isInputActive)

    let handleLoginSuccess = (serverList: array<Types.serverInfo>) => {
      setServers(_ => serverList)
      setCurrentScreen(_ => ServerSelection)
    }

    let handleServerSelected = () => {
      setCurrentScreen(_ => CharacterSelection)
    }

    let handleCharacterSelected = () => {
      Console.log("[FRONTEND] Transitioning UI to 'in_game' screen (loading screen)")
      setCurrentScreen(_ => InGame)
      setIsGameLoading(_ => true)
    }

    let handleBackToLogin = () => {
      setCurrentScreen(_ => Login)
      setServers(_ => [])
    }

    let handleBackToServerSelection = () => {
      setCurrentScreen(_ => ServerSelection)
    }

    let screenKey = switch currentScreen {
    | Login => "login"
    | ServerSelection => "server_selection"
    | CharacterSelection => "character_selection"
    | InGame => "in_game"
    }

    let showBackground = switch (currentScreen, isGameLoading) {
    | (InGame, false) => false
    | _ => true
    }

    <div style={Styles.make({"position": "relative", "minHeight": "100vh"})}>
      <CursorManager />
      <EntityTooltip />
      {switch backgroundUrl {
      | Some(url) if showBackground =>
        <div
          style={Styles.make({
            "position": "fixed",
            "top": 0,
            "left": 0,
            "width": "100%",
            "height": "100%",
            "backgroundImage": `url(${url})`,
            "backgroundSize": "cover",
            "backgroundPosition": "center",
            "backgroundRepeat": "no-repeat",
            "zIndex": -1,
          })}
        />
      | _ => React.null
      }}
      <ScreenTransition transitionKey={screenKey}>
        {switch currentScreen {
        | Login => <Login onLoginSuccess={handleLoginSuccess} />
        | ServerSelection =>
          <ServerSelection
            servers onServerSelected={handleServerSelected} onBackToLogin={handleBackToLogin}
          />
        | CharacterSelection =>
          <CharacterSelection
            onCharacterSelected={handleCharacterSelected}
            onBackToServerSelection={handleBackToServerSelection}
          />
        | InGame =>
          if isGameLoading {
            <LoadingScreen message={zoneStatus} backgroundUrl=?{backgroundUrl} />
          } else {
            <>
              <CharacterInfoPanel />
              <ChatBox />
            </>
          }
        }}
      </ScreenTransition>
    </div>
  }
}

module AppWithAssets = {
  @react.component
  let make = () => {
    let {isLoading, backgroundUrl, error} = AssetsContext.useAssets()

    if isLoading {
      <LoadingScreen message="Loading Lifthrasir..." backgroundUrl=?{backgroundUrl} />
    } else {
      switch error {
      | Some(err) =>
        <div
          style={Styles.make({
            "minHeight": "100vh",
            "display": "flex",
            "alignItems": "center",
            "justifyContent": "center",
            "backgroundColor": "var(--forge-soot)",
            "color": "var(--worn-crimson)",
            "padding": "20px",
            "textAlign": "center",
          })}>
          <div>
            <h1> {React.string("Failed to Load Assets")} </h1>
            <p> {React.string(err)} </p>
            <button
              onClick={_ => DomBindings.Window.location["reload"]()}
              style={Styles.make({
                "marginTop": "20px",
                "padding": "10px 20px",
                "backgroundColor": "var(--energetic-green)",
                "color": "var(--forge-soot)",
                "border": "none",
                "borderRadius": "6px",
                "cursor": "pointer",
              })}>
              {React.string("Retry")}
            </button>
          </div>
        </div>
      | None => <AppContent />
      }
    }
  }
}

@react.component
let make = () => {
  <AssetsContext.AssetsProvider>
    <AppWithAssets />
  </AssetsContext.AssetsProvider>
}
