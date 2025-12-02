%%raw(`import './Login.css'`)

type sessionData = {
  username: string,
  @as("login_id1") loginId1: int,
  @as("account_id") accountId: int,
  @as("login_id2") loginId2: int,
  sex: int,
  servers: array<Types.serverInfo>,
}

type loginResponse = {
  success: bool,
  error: option<string>,
  @as("session_data") sessionData: option<sessionData>,
}

type loginRequest = {
  username: string,
  password: string,
}

@react.component
let make = (~onLoginSuccess: array<Types.serverInfo> => unit) => {
  let (username, setUsername) = React.useState(() => "")
  let (password, setPassword) = React.useState(() => "")
  let (loading, setLoading) = React.useState(() => false)
  let (error, setError) = React.useState(() => None)

  let handleSubmit = async (e: ReactEvent.Form.t) => {
    ReactEvent.Form.preventDefault(e)
    setLoading(_ => true)
    setError(_ => None)

    Console.log("[FRONTEND] ========== LOGIN ATTEMPT ==========")
    Console.log2("[FRONTEND] Username:", username)
    Console.log("[FRONTEND] Calling invoke(\"login\")...")

    try {
      let request: loginRequest = {username, password}
      let result: loginResponse = await Tauri.Core.invoke("login", {"request": request})

      Console.log2("[FRONTEND] Login result received:", result)

      if result.success {
        switch result.sessionData {
        | Some(data) => {
            Console.log2("[FRONTEND] Login successful! Server list:", data.servers)
            onLoginSuccess(data.servers)
          }
        | None => {
            Console.error("[FRONTEND] Login succeeded but no session data")
            setError(_ => Some("Login failed: No session data"))
          }
        }
      } else {
        Console.error2("[FRONTEND] Login failed:", result.error)
        setError(_ => result.error->Option.orElse(Some("Login failed")))
      }
    } catch {
    | err => {
        Console.error2("[FRONTEND] Login error caught:", err)
        setError(_ => Some(`Network error: ${JsExn.message(Obj.magic(err))->Option.getOr("Unknown")}`))
      }
    }

    Console.log("[FRONTEND] Login attempt completed")
    setLoading(_ => false)
  }

  let isSubmitDisabled = loading || username === "" || password === ""

  <div className="login-container">
    <div className="login-box">
      <form onSubmit={e => handleSubmit(e)->ignore} className="login-form">
        <div className="input-group">
          <label htmlFor="username"> {React.string("Username")} </label>
          <input
            id="username"
            type_="text"
            value={username}
            onChange={e => {
              let value = ReactEvent.Form.target(e)["value"]
              setUsername(_ => value)
            }}
            disabled={loading}
            autoFocus=true
            required=true
          />
        </div>
        <div className="input-group">
          <label htmlFor="password"> {React.string("Password")} </label>
          <input
            id="password"
            type_="password"
            value={password}
            onChange={e => {
              let value = ReactEvent.Form.target(e)["value"]
              setPassword(_ => value)
            }}
            disabled={loading}
            required=true
          />
        </div>
        {switch error {
        | Some(msg) => <div className="error-message"> {React.string(msg)} </div>
        | None => React.null
        }}
        <button type_="submit" disabled={isSubmitDisabled} className="login-button">
          {React.string(if loading {"Logging in..."} else {"Login"})}
        </button>
      </form>
    </div>
  </div>
}
