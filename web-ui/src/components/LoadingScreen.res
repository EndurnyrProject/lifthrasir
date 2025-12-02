%%raw(`import './LoadingScreen.css'`)

@react.component
let make = (
  ~message: string="Loading...",
  ~backgroundUrl: option<string>=?,
  ~containerClassName: string="loading-screen-container",
) => {
  let backgroundStyle = switch backgroundUrl {
  | Some(url) =>
    Styles.make({
      "backgroundImage": `url(${url})`,
      "backgroundSize": "cover",
      "backgroundPosition": "center",
      "backgroundRepeat": "no-repeat",
    })
  | None => Styles.make({"backgroundColor": "var(--forge-soot)"})
  }

  <div className={containerClassName} style={backgroundStyle}>
    <div className="loading-screen-overlay">
      <div className="loading-screen-content">
        <LoadingSpinner />
        <p className="loading-screen-message"> {React.string(message)} </p>
      </div>
    </div>
  </div>
}
