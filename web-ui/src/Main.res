%%raw(`import "./styles/fonts.css"`)
%%raw(`import "./styles/typography.css"`)

@val @scope("document")
external getElementById: string => Nullable.t<Dom.element> = "getElementById"

module ReactDOM = {
  module Root = {
    type t
    @send external render: (t, React.element) => unit = "render"
  }

  @module("react-dom/client")
  external createRoot: Dom.element => Root.t = "createRoot"
}

switch getElementById("root")->Nullable.toOption {
| Some(root) => {
    let reactRoot = ReactDOM.createRoot(root)
    reactRoot->ReactDOM.Root.render(
      <React.StrictMode>
        <App />
      </React.StrictMode>,
    )
  }
| None => Console.error("Could not find root element")
}
