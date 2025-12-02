%%raw(`import './ServerSelection.css'`)

type serverSelectionResponse = {
  success: bool,
  error: option<string>,
}

@react.component
let make = (
  ~servers: array<Types.serverInfo>,
  ~onServerSelected: unit => unit,
  ~onBackToLogin: unit => unit,
) => {
  let (selectedIndex, setSelectedIndex) = React.useState(() => None)
  let (loading, setLoading) = React.useState(() => false)
  let (error, setError) = React.useState(() => None)

  let handleServerSelect = (index: int, server: Types.serverInfo) => {
    if server.serverType !== "Maintenance" {
      setSelectedIndex(_ => Some(index))
    }
  }

  let handleConnect = async () => {
    switch selectedIndex {
    | None => ()
    | Some(index) => {
        setLoading(_ => true)
        setError(_ => None)

        try {
          let result: serverSelectionResponse = await Tauri.Core.invoke(
            "select_server",
            {"serverIndex": index},
          )

          if result.success {
            onServerSelected()
          } else {
            setError(_ => result.error->Option.orElse(Some("Server selection failed")))
          }
        } catch {
        | err =>
          setError(_ => Some(`Network error: ${JsExn.message(Obj.magic(err))->Option.getOr("Unknown")}`))
        }

        setLoading(_ => false)
      }
    }
  }

  let handleKeyDown = (e: ReactEvent.Keyboard.t) => {
    if Array.length(servers) === 0 {
      ()
    } else {
      let key = ReactEvent.Keyboard.key(e)
      let serversLen = Array.length(servers)

      if key === "ArrowUp" {
        ReactEvent.Keyboard.preventDefault(e)
        switch selectedIndex {
        | None => setSelectedIndex(_ => Some(serversLen - 1))
        | Some(0) => setSelectedIndex(_ => Some(serversLen - 1))
        | Some(idx) => setSelectedIndex(_ => Some(idx - 1))
        }
      } else if key === "ArrowDown" {
        ReactEvent.Keyboard.preventDefault(e)
        switch selectedIndex {
        | None => setSelectedIndex(_ => Some(0))
        | Some(idx) if idx === serversLen - 1 => setSelectedIndex(_ => Some(0))
        | Some(idx) => setSelectedIndex(_ => Some(idx + 1))
        }
      } else if key === "Enter" && Option.isSome(selectedIndex) {
        ReactEvent.Keyboard.preventDefault(e)
        handleConnect()->ignore
      } else if key === "Escape" {
        ReactEvent.Keyboard.preventDefault(e)
        onBackToLogin()
      }
    }
  }

  <div className="server-selection-container" onKeyDown={handleKeyDown} tabIndex={0}>
    <div className="server-selection-box">
      <h1 className="server-selection-title"> {React.string("Select Server")} </h1>
      <div className="server-list">
        {servers
        ->Array.mapWithIndex((server, index) => {
          let isSelected = selectedIndex === Some(index)
          let isMaintenance = server.serverType === "Maintenance"
          let className =
            "server-item" ++
            (if isSelected {" selected"} else {""}) ++
            (if isMaintenance {" maintenance"} else {""})

          <div key={Int.toString(index)} className onClick={_ => handleServerSelect(index, server)}>
            <span className="server-name"> {React.string(server.name)} </span>
            {if isMaintenance {
              <span className="maintenance-badge"> {React.string("Maintenance")} </span>
            } else {
              React.null
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
        <button onClick={_ => onBackToLogin()} className="back-button" disabled={loading}>
          {React.string("Back to Login")}
        </button>
        <button
          onClick={_ => handleConnect()->ignore}
          disabled={loading || Option.isNone(selectedIndex)}
          className="connect-button">
          {React.string(if loading {"Connecting..."} else {"Connect"})}
        </button>
      </div>
    </div>
  </div>
}
