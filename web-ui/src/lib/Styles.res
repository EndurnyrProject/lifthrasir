external make: {..} => ReactDOM.style = "%identity"

let empty: ReactDOM.style = %raw(`{}`)

let combine: (ReactDOM.style, ReactDOM.style) => ReactDOM.style = %raw(`
  function(s1, s2) { return Object.assign({}, s1, s2); }
`)
