let isInteractiveElement = (element: Dom.element): bool => {
  let tag = element->DomBindings.Element.tagName->DomBindings.String.toLowerCase
  ["input", "textarea", "select", "button"]->Array.includes(tag) ||
  element->DomBindings.Element.hasAttribute("contenteditable")
}

let use = (~isActive: bool) => {
  React.useEffect1(() => {
    if !isActive {
      None
    } else {
      let handleKeyDown = (e: Dom.event) => {
        if !isInteractiveElement(DomBindings.Event.target(e)) {
          DomBindings.Event.preventDefault(e)
          let _ = Tauri.Core.invoke("forward_keyboard_input", {
            "code": DomBindings.Event.code(e),
            "pressed": true,
          })
        }
      }

      let handleKeyUp = (e: Dom.event) => {
        if !isInteractiveElement(DomBindings.Event.target(e)) {
          let _ = Tauri.Core.invoke("forward_keyboard_input", {
            "code": DomBindings.Event.code(e),
            "pressed": false,
          })
        }
      }

      Console.log("[FRONTEND] Setting up keyboard event forwarding to Bevy")
      DomBindings.Document.addEventListener("keydown", handleKeyDown)
      DomBindings.Document.addEventListener("keyup", handleKeyUp)

      Some(() => {
        Console.log("[FRONTEND] Removing keyboard event forwarding")
        DomBindings.Document.removeEventListener("keydown", handleKeyDown)
        DomBindings.Document.removeEventListener("keyup", handleKeyUp)
      })
    }
  }, [isActive])
}
