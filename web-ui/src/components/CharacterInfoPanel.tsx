import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import './CharacterInfoPanel.css';

interface CharacterStatus {
  name: string;
  job_name: string;
  hp: number;
  max_hp: number;
  sp: number;
  max_sp: number;
  base_level: number;
  job_level: number;
  base_exp: number;
  next_base_exp: number;
  job_exp: number;
  next_job_exp: number;
  zeny: number;
  weight: number;
  max_weight: number;
}

interface ProgressBarProps {
  current: number;
  max: number;
  color: string;
  label: string;
}

function ProgressBar({ current, max, color, label }: ProgressBarProps) {
  const percentage = max > 0 ? (current / max) * 100 : 0;

  return (
    <div className="progress-bar-container">
      <div className="progress-bar-label">
        <span className="progress-label-text">{label}</span>
        <span className="progress-label-values">{current} / {max}</span>
      </div>
      <div className="progress-bar-track">
        <div
          className="progress-bar-fill"
          style={{
            width: `${percentage}%`,
            backgroundColor: color
          }}
        />
      </div>
    </div>
  );
}

export function CharacterInfoPanel() {
  const [status, setStatus] = useState<CharacterStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const isMountedRef = useRef(true);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    isMountedRef.current = true;

    const initialize = async () => {
      try {
        const initialStatus = await invoke<CharacterStatus>('get_character_status');

        if (!isMountedRef.current) {
          return;
        }

        setStatus(initialStatus);
        setError(null);

        unlisten = await listen<CharacterStatus>('character-status-update', (event) => {
          if (isMountedRef.current) {
            setStatus(event.payload);
          }
        });

      } catch (err) {
        if (isMountedRef.current) {
          setError(err as string);
          console.error('Failed to get character status:', err);
        }
      }
    };

    initialize();

    return () => {
      isMountedRef.current = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  if (error) {
    return (
      <div className="character-info-panel error">
        <div className="error-message">Failed to load character status</div>
      </div>
    );
  }

  if (!status) {
    return null;
  }

  const baseExpPercent = status.next_base_exp > 0
    ? (status.base_exp / status.next_base_exp) * 100
    : 0;
  const jobExpPercent = status.next_job_exp > 0
    ? (status.job_exp / status.next_job_exp) * 100
    : 0;
  const weightPercent = status.max_weight > 0
    ? (status.weight / status.max_weight) * 100
    : 0;

  const weightColor = weightPercent >= 90
    ? 'var(--worn-crimson)'
    : weightPercent >= 50
    ? '#ffb74d'
    : 'var(--energetic-green)';

  return (
    <div className="character-info-panel">
      <div className="character-name">{status.name} - {status.job_name}</div>

      <ProgressBar
        current={status.hp}
        max={status.max_hp}
        color="var(--health-red)"
        label="HP"
      />

      <ProgressBar
        current={status.sp}
        max={status.max_sp}
        color="var(--mana-blue)"
        label="SP"
      />

      <div className="character-info-row">
        <span className="info-label">Base Lv:</span>
        <span className="info-value">{status.base_level}</span>
        <div className="exp-bar-mini">
          <div
            className="exp-bar-fill"
            style={{ width: `${baseExpPercent}%` }}
            title={`${status.base_exp} / ${status.next_base_exp}`}
          />
        </div>
      </div>

      <div className="character-info-row">
        <span className="info-label">Job Lv:</span>
        <span className="info-value">{status.job_level}</span>
        <div className="exp-bar-mini">
          <div
            className="exp-bar-fill job"
            style={{ width: `${jobExpPercent}%` }}
            title={`${status.job_exp} / ${status.next_job_exp}`}
          />
        </div>
      </div>

      <div className="character-info-row bottom-row">
        <div className="stat-group">
          <span className="info-label">Zeny:</span>
          <span className="info-value zeny">{status.zeny.toLocaleString()}</span>
        </div>
        <div className="stat-group">
          <span className="info-label">Weight:</span>
          <span className="info-value" style={{ color: weightColor }}>
            {status.weight} / {status.max_weight}
          </span>
        </div>
      </div>
    </div>
  );
}
