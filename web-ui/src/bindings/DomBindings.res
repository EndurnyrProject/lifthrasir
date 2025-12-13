@val external alert: string => unit = "alert"
@val external devicePixelRatio: float = "window.devicePixelRatio"

module Window = {
  @val external location: {..} = "window.location"
  @val external innerWidth: int = "window.innerWidth"
  @val external innerHeight: int = "window.innerHeight"
}

module Document = {
  @val external addEventListener: (string, Dom.event => unit) => unit = "document.addEventListener"
  @val external removeEventListener: (string, Dom.event => unit) => unit =
    "document.removeEventListener"
  @val external dispatchEvent: Dom.event => bool = "document.dispatchEvent"
}

module MouseEvent = {
  type mouseEventInit = {
    bubbles: bool,
    clientX: int,
    clientY: int,
  }
  @new external make: (string, mouseEventInit) => Dom.event = "MouseEvent"
}

module Element = {
  @get external tagName: Dom.element => string = "tagName"
  @send external hasAttribute: (Dom.element, string) => bool = "hasAttribute"
}

module Event = {
  @get external target: Dom.event => Dom.element = "target"
  @get external code: Dom.event => string = "code"
  @get external button: Dom.event => int = "button"
  @get external clientX: Dom.event => float = "clientX"
  @get external clientY: Dom.event => float = "clientY"
  @send external preventDefault: Dom.event => unit = "preventDefault"
}

module String = {
  @send external toLowerCase: string => string = "toLowerCase"
}
