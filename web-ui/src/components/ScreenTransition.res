%%raw(`import './ScreenTransition.css'`)

@react.component
let make = (~children: React.element, ~transitionKey: string) => {
  let (displayChildren, setDisplayChildren) = React.useState(() => children)
  let (isAnimating, setIsAnimating) = React.useState(() => false)

  React.useEffect2(() => {
    setIsAnimating(_ => true)

    let timeoutId = setTimeout(() => {
      setDisplayChildren(_ => children)
      setIsAnimating(_ => false)
    }, 300)

    Some(() => clearTimeout(timeoutId))
  }, (transitionKey, children))

  let className = if isAnimating {
    "screen-transition screen-transition-out"
  } else {
    "screen-transition screen-transition-in"
  }

  <div className> {displayChildren} </div>
}
