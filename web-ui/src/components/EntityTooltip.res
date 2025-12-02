%%raw(`import './EntityTooltip.css'`)

type entityTooltipData = {
  @as("entity_id") entityId: int,
  name: string,
  @as("party_name") partyName: option<string>,
  @as("guild_name") guildName: option<string>,
  @as("position_name") positionName: option<string>,
  @as("screen_x") screenX: int,
  @as("screen_y") screenY: int,
}

@react.component
let make = () => {
  let (tooltipData, setTooltipData) = React.useState(() => None)

  React.useEffect0(() => {
    let unlistenShow = ref(() => ())
    let unlistenHide = ref(() => ())

    let setupListeners = async () => {
      let showFn = await Tauri.Event.listen("entity-name-show", event => {
        setTooltipData(_ => Some(event.payload))
      })
      unlistenShow := showFn

      let hideFn = await Tauri.Event.listen("entity-name-hide", _ => {
        setTooltipData(_ => None)
      })
      unlistenHide := hideFn
    }

    setupListeners()->ignore

    Some(
      () => {
        unlistenShow.contents()
        unlistenHide.contents()
      },
    )
  })

  switch tooltipData {
  | None => React.null
  | Some(data) =>
    <div
      className="entity-tooltip"
      style={Styles.make({
        "left": `${Int.toString(data.screenX)}px`,
        "top": `${Int.toString(data.screenY - 40)}px`,
      })}>
      <div className="entity-tooltip-name"> {React.string(data.name)} </div>
      {switch data.partyName {
      | Some(party) =>
        <div className="entity-tooltip-party"> {React.string(`Party: ${party}`)} </div>
      | None => React.null
      }}
      {switch data.guildName {
      | Some(guild) =>
        <div className="entity-tooltip-guild"> {React.string(`Guild: ${guild}`)} </div>
      | None => React.null
      }}
      {switch data.positionName {
      | Some(position) =>
        <div className="entity-tooltip-position"> {React.string(position)} </div>
      | None => React.null
      }}
    </div>
  }
}
