let use = (~isActive: bool) => {
  let isRightDraggingRef = React.useRef(false)
  let lastMouseXRef = React.useRef(0.0)
  let lastMouseYRef = React.useRef(0.0)

  React.useEffect1(() => {
    if !isActive {
      None
    } else {
      let handleContextMenu = (e: Dom.event) => {
        DomBindings.Event.preventDefault(e)
      }

      let handleMouseDown = (e: Dom.event) => {
        if DomBindings.Event.button(e) === 2 {
          isRightDraggingRef.current = true
          lastMouseXRef.current = DomBindings.Event.clientX(e)
          lastMouseYRef.current = DomBindings.Event.clientY(e)
        }
      }

      let handleMouseMove = (e: Dom.event) => {
        if isRightDraggingRef.current {
          let deltaX = DomBindings.Event.clientX(e) -. lastMouseXRef.current
          let deltaY = DomBindings.Event.clientY(e) -. lastMouseYRef.current

          if deltaX !== 0.0 || deltaY !== 0.0 {
            let _ = Tauri.Core.invoke("forward_camera_rotation", {
              "deltaX": deltaX,
              "deltaY": deltaY,
            })
            lastMouseXRef.current = DomBindings.Event.clientX(e)
            lastMouseYRef.current = DomBindings.Event.clientY(e)
          }
        }
      }

      let handleMouseUp = (e: Dom.event) => {
        if DomBindings.Event.button(e) === 2 {
          isRightDraggingRef.current = false
        }
      }

      DomBindings.Document.addEventListener("contextmenu", handleContextMenu)
      DomBindings.Document.addEventListener("mousedown", handleMouseDown)
      DomBindings.Document.addEventListener("mousemove", handleMouseMove)
      DomBindings.Document.addEventListener("mouseup", handleMouseUp)

      Some(() => {
        DomBindings.Document.removeEventListener("contextmenu", handleContextMenu)
        DomBindings.Document.removeEventListener("mousedown", handleMouseDown)
        DomBindings.Document.removeEventListener("mousemove", handleMouseMove)
        DomBindings.Document.removeEventListener("mouseup", handleMouseUp)
      })
    }
  }, [isActive])
}
