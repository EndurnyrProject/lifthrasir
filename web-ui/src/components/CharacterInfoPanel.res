%%raw(`import './CharacterInfoPanel.css'`)

type characterStatus = {
  name: string,
  @as("job_name") jobName: string,
  hp: int,
  @as("max_hp") maxHp: int,
  sp: int,
  @as("max_sp") maxSp: int,
  @as("base_level") baseLevel: int,
  @as("job_level") jobLevel: int,
  @as("base_exp") baseExp: int,
  @as("next_base_exp") nextBaseExp: int,
  @as("job_exp") jobExp: int,
  @as("next_job_exp") nextJobExp: int,
  zeny: int,
  weight: int,
  @as("max_weight") maxWeight: int,
}

module ProgressBar = {
  @react.component
  let make = (~current: int, ~max: int, ~color: string, ~label: string) => {
    let percentage = if max > 0 {
      Int.toFloat(current) /. Int.toFloat(max) *. 100.0
    } else {
      0.0
    }

    <div className="progress-bar-container">
      <div className="progress-bar-label">
        <span className="progress-label-text"> {React.string(label)} </span>
        <span className="progress-label-values">
          {React.string(`${Int.toString(current)} / ${Int.toString(max)}`)}
        </span>
      </div>
      <div className="progress-bar-track">
        <div
          className="progress-bar-fill"
          style={Styles.make({
            "width": `${Float.toString(percentage)}%`,
            "backgroundColor": color,
          })}
        />
      </div>
    </div>
  }
}

@react.component
let make = () => {
  let (status, setStatus) = React.useState(() => None)
  let (error, setError) = React.useState(() => None)
  let isMountedRef = React.useRef(true)

  React.useEffect0(() => {
    let unlisten = ref(() => ())
    isMountedRef.current = true

    let initialize = async () => {
      try {
        let initialStatus = await Tauri.Core.invokeNoArgs("get_character_status")

        if isMountedRef.current {
          setStatus(_ => Some(initialStatus))
          setError(_ => None)

          let unlistenFn = await Tauri.Event.listen("character-status-update", event => {
            if isMountedRef.current {
              setStatus(_ => Some(event.payload))
            }
          })
          unlisten := unlistenFn
        }
      } catch {
      | err =>
        if isMountedRef.current {
          setError(_ => Some(err))
          Console.error2("Failed to get character status:", err)
        }
      }
    }

    initialize()->ignore

    Some(
      () => {
        isMountedRef.current = false
        unlisten.contents()
      },
    )
  })

  if Option.isSome(error) {
    <div className="character-info-panel error">
      <div className="error-message"> {React.string("Failed to load character status")} </div>
    </div>
  } else {
    switch status {
    | None => React.null
    | Some(s) => {
        let baseExpPercent = if s.nextBaseExp > 0 {
          Int.toFloat(s.baseExp) /. Int.toFloat(s.nextBaseExp) *. 100.0
        } else {
          0.0
        }

        let jobExpPercent = if s.nextJobExp > 0 {
          Int.toFloat(s.jobExp) /. Int.toFloat(s.nextJobExp) *. 100.0
        } else {
          0.0
        }

        let weightPercent = if s.maxWeight > 0 {
          Int.toFloat(s.weight) /. Int.toFloat(s.maxWeight) *. 100.0
        } else {
          0.0
        }

        let weightColor = if weightPercent >= 90.0 {
          "var(--worn-crimson)"
        } else if weightPercent >= 50.0 {
          "#ffb74d"
        } else {
          "var(--energetic-green)"
        }

        <div className="character-info-panel">
          <div className="character-name">
            {React.string(`${s.name} - ${s.jobName}`)}
          </div>
          <ProgressBar current={s.hp} max={s.maxHp} color="var(--health-red)" label="HP" />
          <ProgressBar current={s.sp} max={s.maxSp} color="var(--mana-blue)" label="SP" />
          <div className="character-info-row">
            <span className="info-label"> {React.string("Base Lv:")} </span>
            <span className="info-value"> {React.string(Int.toString(s.baseLevel))} </span>
            <div className="exp-bar-mini">
              <div
                className="exp-bar-fill"
                style={Styles.make({"width": `${Float.toString(baseExpPercent)}%`})}
                title={`${Int.toString(s.baseExp)} / ${Int.toString(s.nextBaseExp)}`}
              />
            </div>
          </div>
          <div className="character-info-row">
            <span className="info-label"> {React.string("Job Lv:")} </span>
            <span className="info-value"> {React.string(Int.toString(s.jobLevel))} </span>
            <div className="exp-bar-mini">
              <div
                className="exp-bar-fill job"
                style={Styles.make({"width": `${Float.toString(jobExpPercent)}%`})}
                title={`${Int.toString(s.jobExp)} / ${Int.toString(s.nextJobExp)}`}
              />
            </div>
          </div>
          <div className="character-info-row bottom-row">
            <div className="stat-group">
              <span className="info-label"> {React.string("Zeny:")} </span>
              <span className="info-value zeny">
                {React.string(Int.toString(s.zeny))}
              </span>
            </div>
            <div className="stat-group">
              <span className="info-label"> {React.string("Weight:")} </span>
              <span className="info-value" style={Styles.make({"color": weightColor})}>
                {React.string(`${Int.toString(s.weight)} / ${Int.toString(s.maxWeight)}`)}
              </span>
            </div>
          </div>
        </div>
      }
    }
  }
}
