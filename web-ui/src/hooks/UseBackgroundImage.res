type useBackgroundImageResult = {
  backgroundUrl: option<string>,
  isLoading: bool,
  error: option<string>,
}

let useBackgroundImage = (assetPath: string): useBackgroundImageResult => {
  let (backgroundUrl, setBackgroundUrl) = React.useState(() => None)
  let (isLoading, setIsLoading) = React.useState(() => true)
  let (error, setError) = React.useState(() => None)

  React.useEffect1(() => {
    let isMounted = ref(true)

    let loadBackground = async () => {
      try {
        setIsLoading(_ => true)
        let url = await Assets.loadAsset(assetPath)

        if isMounted.contents {
          setBackgroundUrl(_ => Some(url))
          setError(_ => None)
        }
      } catch {
      | _ =>
        if isMounted.contents {
          setError(_ => Some("Failed to load background"))
        }
      }

      if isMounted.contents {
        setIsLoading(_ => false)
      }
    }

    loadBackground()->ignore

    Some(
      () => {
        isMounted := false
        switch backgroundUrl {
        | Some(url) => Assets.revokeAssetUrl(url)
        | None => ()
        }
      },
    )
  }, [assetPath])

  {backgroundUrl, isLoading, error}
}
