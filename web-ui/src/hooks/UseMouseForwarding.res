let use = (~isActive: bool) => {
  let lastMousePosition = React.useRef(None)

  React.useEffect0(() => {
    let trackMouse = (e: Dom.event) => {
      lastMousePosition.current = Some({
        "x": DomBindings.Event.clientX(e),
        "y": DomBindings.Event.clientY(e),
      })
    }
    DomBindings.Document.addEventListener("mousemove", trackMouse)
    Some(() => DomBindings.Document.removeEventListener("mousemove", trackMouse))
  })

  React.useEffect1(() => {
    if !isActive {
      None
    } else {
      switch lastMousePosition.current {
      | Some(pos) => {
          let x: float = pos["x"]
          let y: float = pos["y"]
          let _ = Tauri.Core.invoke(
            "forward_mouse_position",
            {
              "x": x -. 36.0,
              "y": y +. 20.0,
            },
          )
        }
      | None => ()
      }

      let handleMouseMove = (e: Dom.event) => {
        let _ = Tauri.Core.invoke(
          "forward_mouse_position",
          {
            "x": DomBindings.Event.clientX(e) -. 36.0,
            "y": DomBindings.Event.clientY(e) +. 20.0,
          },
        )
      }

      DomBindings.Document.addEventListener("mousemove", handleMouseMove)
      Some(() => DomBindings.Document.removeEventListener("mousemove", handleMouseMove))
    }
  }, [isActive])

  React.useEffect1(() => {
    if !isActive {
      None
    } else {
      let handleMouseClick = (e: Dom.event) => {
        if DomBindings.Event.button(e) === 0 {
          let _ = Tauri.Core.invoke(
            "forward_mouse_click",
            {
              "x": DomBindings.Event.clientX(e) -. 34.0,
              "y": DomBindings.Event.clientY(e) -. 17.0,
            },
          )
        }
      }

      DomBindings.Document.addEventListener("click", handleMouseClick)
      Some(() => DomBindings.Document.removeEventListener("click", handleMouseClick))
    }
  }, [isActive])
}
