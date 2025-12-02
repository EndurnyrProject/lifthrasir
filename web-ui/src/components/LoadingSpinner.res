%%raw(`import './LoadingSpinner.css'`)

@react.component
let make = () => {
  <div className="loading-spinner">
    <div className="spinner-ring" />
    <div className="spinner-ring" />
    <div className="spinner-ring" />
  </div>
}
