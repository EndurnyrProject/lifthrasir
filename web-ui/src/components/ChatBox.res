%%raw(`import './ChatBox.css'`)

type chatMessage = {
  gid: int,
  message: string,
}

@send external scrollIntoView: (Dom.element, {..}) => unit = "scrollIntoView"

@react.component
let make = () => {
  let (messages, setMessages) = React.useState(() => [])
  let (inputValue, setInputValue) = React.useState(() => "")
  let messagesEndRef = React.useRef(Nullable.null)

  React.useEffect0(() => {
    let unlisten = ref(() => ())

    let setup = async () => {
      let unlistenFn = await Tauri.Event.listen("chat-message-received", event => {
        setMessages(prev => Array.concat(prev, [event.payload]))
      })
      unlisten := unlistenFn
    }

    setup()->ignore

    Some(() => unlisten.contents())
  })

  React.useEffect1(() => {
    switch messagesEndRef.current->Nullable.toOption {
    | Some(el) => el->scrollIntoView({"behavior": "smooth"})
    | None => ()
    }
    None
  }, [messages])

  let handleKeyDown = async (e: ReactEvent.Keyboard.t) => {
    let key = ReactEvent.Keyboard.key(e)
    let trimmedValue = String.trim(inputValue)

    if key === "Enter" && trimmedValue !== "" {
      try {
        let _ = await Tauri.Core.invoke("send_chat_message", {"message": inputValue})
        setInputValue(_ => "")
      } catch {
      | err => Console.error2("Failed to send chat message:", err)
      }
    }

    ReactEvent.Keyboard.stopPropagation(e)
  }

  <div className="chat-box">
    <div className="chat-messages">
      {messages
      ->Array.mapWithIndex((msg, index) =>
        <div key={Int.toString(index)} className="chat-message">
          {React.string(msg.message)}
        </div>
      )
      ->React.array}
      <div ref={ReactDOM.Ref.domRef(messagesEndRef)} />
    </div>
    <div className="chat-input-container">
      <input
        type_="text"
        className="chat-input"
        value={inputValue}
        onChange={e => {
          let value = ReactEvent.Form.target(e)["value"]
          setInputValue(_ => value)
        }}
        onKeyDown={e => handleKeyDown(e)->ignore}
        placeholder="Press Enter to chat..."
      />
    </div>
  </div>
}
