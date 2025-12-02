let use = () => {
  React.useEffect0(() => {
    let handleGlobalKeyDown = (e: Dom.event) => {
      if DomBindings.Event.code(e) === "F12" {
        DomBindings.Event.preventDefault(e)
        Console.log("[FRONTEND] F12 pressed - Opening dev tools...")
        Tauri.Core.invokeNoArgs("open_devtools")->ignore
      }
    }

    DomBindings.Document.addEventListener("keydown", handleGlobalKeyDown)

    Some(() => {
      DomBindings.Document.removeEventListener("keydown", handleGlobalKeyDown)
    })
  })
}
