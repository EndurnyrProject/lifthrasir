type assetsContextValue = {
  backgroundUrl: option<string>,
  slotWithCharUrl: option<string>,
  slotNoCharUrl: option<string>,
  slotBlockedUrl: option<string>,
  cursorDefaultUrl: option<string>,
  cursorAddUrl: option<string>,
  cursorAttackUrl: option<string>,
  cursorImpossibleUrl: option<string>,
  cursorTalkUrl: option<string>,
  isLoading: bool,
  error: option<string>,
}

type urlsForCleanup = {
  mutable backgroundUrl: option<string>,
  mutable slotWithCharUrl: option<string>,
  mutable slotNoCharUrl: option<string>,
  mutable slotBlockedUrl: option<string>,
  mutable cursorDefaultUrl: option<string>,
  mutable cursorAddUrl: option<string>,
  mutable cursorAttackUrl: option<string>,
  mutable cursorImpossibleUrl: option<string>,
  mutable cursorTalkUrl: option<string>,
}

let defaultValue: assetsContextValue = {
  backgroundUrl: None,
  slotWithCharUrl: None,
  slotNoCharUrl: None,
  slotBlockedUrl: None,
  cursorDefaultUrl: None,
  cursorAddUrl: None,
  cursorAttackUrl: None,
  cursorImpossibleUrl: None,
  cursorTalkUrl: None,
  isLoading: true,
  error: None,
}

let context = React.createContext(defaultValue)

module Provider = {
  let make = context->React.Context.provider
}

@val @scope("URL") external revokeObjectURL: string => unit = "revokeObjectURL"

module AssetsProvider = {
  @react.component
  let make = (~children: React.element) => {
    let (backgroundUrl, setBackgroundUrl) = React.useState(() => None)
    let (slotWithCharUrl, setSlotWithCharUrl) = React.useState(() => None)
    let (slotNoCharUrl, setSlotNoCharUrl) = React.useState(() => None)
    let (slotBlockedUrl, setSlotBlockedUrl) = React.useState(() => None)
    let (cursorDefaultUrl, setCursorDefaultUrl) = React.useState(() => None)
    let (cursorAddUrl, setCursorAddUrl) = React.useState(() => None)
    let (cursorAttackUrl, setCursorAttackUrl) = React.useState(() => None)
    let (cursorImpossibleUrl, setCursorImpossibleUrl) = React.useState(() => None)
    let (cursorTalkUrl, setCursorTalkUrl) = React.useState(() => None)
    let (isLoading, setIsLoading) = React.useState(() => true)
    let (error, setError) = React.useState(() => None)

    let urlsRef: React.ref<urlsForCleanup> = React.useRef({
      backgroundUrl: None,
      slotWithCharUrl: None,
      slotNoCharUrl: None,
      slotBlockedUrl: None,
      cursorDefaultUrl: None,
      cursorAddUrl: None,
      cursorAttackUrl: None,
      cursorImpossibleUrl: None,
      cursorTalkUrl: None,
    })

    React.useEffect0(() => {
      let preloadAssets = async () => {
        try {
          setIsLoading(_ => true)

          let bgUrl = await Assets.loadAsset("login_screen.png")
          setBackgroundUrl(_ => Some(bgUrl))
          urlsRef.current.backgroundUrl = Some(bgUrl)

          let slotWithChar = await Assets.loadAsset("textures/ui/character_screen/slot_with_char.png")
          setSlotWithCharUrl(_ => Some(slotWithChar))
          urlsRef.current.slotWithCharUrl = Some(slotWithChar)

          let slotNoChar = await Assets.loadAsset("textures/ui/character_screen/slot_no_char.png")
          setSlotNoCharUrl(_ => Some(slotNoChar))
          urlsRef.current.slotNoCharUrl = Some(slotNoChar)

          let slotBlocked = await Assets.loadAsset("textures/ui/character_screen/slot_blocked_char.png")
          setSlotBlockedUrl(_ => Some(slotBlocked))
          urlsRef.current.slotBlockedUrl = Some(slotBlocked)

          let cursorDefault = await Assets.loadAsset("textures/ui/cursors/cursor_default.png")
          setCursorDefaultUrl(_ => Some(cursorDefault))
          urlsRef.current.cursorDefaultUrl = Some(cursorDefault)

          let cursorAdd = await Assets.loadAsset("textures/ui/cursors/cursor_add.png")
          setCursorAddUrl(_ => Some(cursorAdd))
          urlsRef.current.cursorAddUrl = Some(cursorAdd)

          let cursorAttack = await Assets.loadAsset("textures/ui/cursors/cursor_attack.png")
          setCursorAttackUrl(_ => Some(cursorAttack))
          urlsRef.current.cursorAttackUrl = Some(cursorAttack)

          let cursorImpossible = await Assets.loadAsset("textures/ui/cursors/cursor_impossible.png")
          setCursorImpossibleUrl(_ => Some(cursorImpossible))
          urlsRef.current.cursorImpossibleUrl = Some(cursorImpossible)

          let cursorTalk = await Assets.loadAsset("textures/ui/cursors/cursor_talk.png")
          setCursorTalkUrl(_ => Some(cursorTalk))
          urlsRef.current.cursorTalkUrl = Some(cursorTalk)

          setError(_ => None)
        } catch {
        | err => {
            setError(_ => Some(`Failed to load assets: ${JsExn.message(Obj.magic(err))->Option.getOr("Unknown error")}`))
            Console.error2("Asset preloading failed:", err)
          }
        }
        setIsLoading(_ => false)
      }

      preloadAssets()->ignore

      Some(
        () => {
          let urls = urlsRef.current
          urls.backgroundUrl->Option.forEach(revokeObjectURL)
          urls.slotWithCharUrl->Option.forEach(revokeObjectURL)
          urls.slotNoCharUrl->Option.forEach(revokeObjectURL)
          urls.slotBlockedUrl->Option.forEach(revokeObjectURL)
          urls.cursorDefaultUrl->Option.forEach(revokeObjectURL)
          urls.cursorAddUrl->Option.forEach(revokeObjectURL)
          urls.cursorAttackUrl->Option.forEach(revokeObjectURL)
          urls.cursorImpossibleUrl->Option.forEach(revokeObjectURL)
          urls.cursorTalkUrl->Option.forEach(revokeObjectURL)
        },
      )
    })

    let value: assetsContextValue = {
      backgroundUrl,
      slotWithCharUrl,
      slotNoCharUrl,
      slotBlockedUrl,
      cursorDefaultUrl,
      cursorAddUrl,
      cursorAttackUrl,
      cursorImpossibleUrl,
      cursorTalkUrl,
      isLoading,
      error,
    }

    <Provider value> children </Provider>
  }
}

let useAssets = (): assetsContextValue => {
  React.useContext(context)
}
