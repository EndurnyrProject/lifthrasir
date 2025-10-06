import { useState, useEffect } from "react";
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import Login from "./screens/Login";
import ServerSelection from "./screens/ServerSelection";
import CharacterSelection from "./screens/CharacterSelection";
import { AssetsProvider, useAssets } from "./contexts";
import { LoadingScreen, ScreenTransition } from "./components";
import "./styles/theme.css";
import "./App.css";

type AppScreen = "login" | "server_selection" | "character_selection" | "in_game";

interface ServerInfo {
  ip: number;
  port: number;
  name: string;
  users: number;
  server_type: any;
  new_server: number;
}

function AppContent() {
  const [currentScreen, setCurrentScreen] = useState<AppScreen>("login");
  const [isGameLoading, setIsGameLoading] = useState(false);
  const [servers, setServers] = useState<ServerInfo[]>([]);
  const [zoneStatus, setZoneStatus] = useState<string>("Connecting to zone server...");
  const { backgroundUrl } = useAssets();

  // Set up zone event listeners
  useEffect(() => {
    const unlistenPromises: Promise<UnlistenFn>[] = [];

    // Zone connecting event
    unlistenPromises.push(listen('zone-connecting', (event: any) => {
      const mapName = event.payload.map_name || 'unknown';
      console.log(`📥 [FRONTEND] Received 'zone-connecting' event for map:`, mapName);
      setZoneStatus(`Connecting to ${mapName}...`);
    }));

    // Zone connected event
    unlistenPromises.push(listen('zone-connected', () => {
      console.log(`📥 [FRONTEND] Received 'zone-connected' event`);
      setZoneStatus('Connected! Authenticating...');
    }));

    // Zone authenticated event
    unlistenPromises.push(listen('zone-authenticated', (event: any) => {
      const { spawn_x, spawn_y } = event.payload;
      console.log(`📥 [FRONTEND] Received 'zone-authenticated' event - spawn at (${spawn_x}, ${spawn_y})`);
      setZoneStatus(`Authenticated! Loading map at (${spawn_x}, ${spawn_y})...`);
    }));

    // Map loading event
    unlistenPromises.push(listen('map-loading', (event: any) => {
      const mapName = event.payload.map_name || 'map';
      console.log(`📥 [FRONTEND] Received 'map-loading' event for map:`, mapName);
      setZoneStatus(`Loading ${mapName}...`);
    }));

    // Map loaded event
    unlistenPromises.push(listen('map-loaded', (event: any) => {
      const mapName = event.payload.map_name || 'map';
      console.log(`📥 [FRONTEND] Received 'map-loaded' event for map:`, mapName);
      setZoneStatus(`${mapName} loaded! Entering world...`);
    }));

    // Entering world event
    unlistenPromises.push(listen('entering-world', () => {
      console.log(`📥 [FRONTEND] Received 'entering-world' event`);
      setZoneStatus('Entering world...');
      setIsGameLoading(false);
    }));

    // Zone error event - return to character selection
    unlistenPromises.push(listen('zone-error', (event: any) => {
      const error = event.payload.error || 'Connection failed';
      console.error(`📥 [FRONTEND] Received 'zone-error' event:`, error);
      alert(`Zone connection failed: ${error}`);
      setCurrentScreen('character_selection');
      setZoneStatus('Connecting to zone server...');
    }));

    // Map loading failed event - return to character selection
    unlistenPromises.push(listen('map-loading-failed', (event: any) => {
      const error = event.payload.error || 'Map loading failed';
      console.error(`📥 [FRONTEND] Received 'map-loading-failed' event:`, error);
      alert(`Map loading failed: ${error}`);
      setCurrentScreen('character_selection');
      setZoneStatus('Connecting to zone server...');
    }));

    // Cleanup function
    return () => {
      unlistenPromises.forEach(promise => {
        promise.then(unlisten => unlisten()).catch(console.error);
      });
    };
  }, []);

  const handleLoginSuccess = (serverList: ServerInfo[]) => {
    setServers(serverList);
    setCurrentScreen("server_selection");
  };

  const handleServerSelected = () => {
    setCurrentScreen("character_selection");
  };

  const handleCharacterSelected = () => {
    // Character selected - transition to game loading screen
    console.log(`🎮 [FRONTEND] Transitioning UI to 'in_game' screen (loading screen)`);
    setCurrentScreen("in_game");
    setIsGameLoading(true);
  };

  const handleBackToLogin = () => {
    setCurrentScreen("login");
    setServers([]);
  };

  const handleBackToServerSelection = () => {
    setCurrentScreen("server_selection");
  };

  return (
    <div style={{ position: 'relative', minHeight: '100vh' }}>
      {/* Static background layer - doesn't transition */}
      {/* Hide background when in game to reveal Bevy 3D canvas */}
      {backgroundUrl && !(currentScreen === "in_game" && !isGameLoading) && (
        <div
          style={{
            position: 'fixed',
            top: 0,
            left: 0,
            width: '100%',
            height: '100%',
            backgroundImage: `url(${backgroundUrl})`,
            backgroundSize: 'cover',
            backgroundPosition: 'center',
            backgroundRepeat: 'no-repeat',
            zIndex: -1,
          }}
        />
      )}

      {/* Content layer - transitions smoothly */}
      <ScreenTransition transitionKey={currentScreen}>
        {currentScreen === "login" && (
          <Login onLoginSuccess={handleLoginSuccess} />
        )}
        {currentScreen === "server_selection" && (
          <ServerSelection
            servers={servers}
            onServerSelected={handleServerSelected}
            onBackToLogin={handleBackToLogin}
          />
        )}
        {currentScreen === "character_selection" && (
          <CharacterSelection
            onCharacterSelected={handleCharacterSelected}
            onBackToServerSelection={handleBackToServerSelection}
          />
        )}
        {currentScreen === "in_game" && (
          isGameLoading ? (
            <LoadingScreen
              message={zoneStatus}
              backgroundUrl={backgroundUrl}
            />
          ) : null
        )}
      </ScreenTransition>
    </div>
  );
}

function App() {
  return (
    <AssetsProvider>
      <AppWithAssets />
    </AssetsProvider>
  );
}

function AppWithAssets() {
  const { isLoading, backgroundUrl, error } = useAssets();

  // Show loading screen during initial asset preload
  if (isLoading) {
    return (
      <LoadingScreen
        message="Loading Lifthrasir..."
        backgroundUrl={backgroundUrl}
      />
    );
  }

  // Show error if assets failed to load (with fallback background)
  if (error) {
    return (
      <div style={{
        minHeight: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: 'var(--forge-soot)',
        color: 'var(--worn-crimson)',
        padding: '20px',
        textAlign: 'center'
      }}>
        <div>
          <h1>Failed to Load Assets</h1>
          <p>{error}</p>
          <button
            onClick={() => window.location.reload()}
            style={{
              marginTop: '20px',
              padding: '10px 20px',
              backgroundColor: 'var(--energetic-green)',
              color: 'var(--forge-soot)',
              border: 'none',
              borderRadius: '6px',
              cursor: 'pointer'
            }}
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  // Assets loaded successfully - render app
  return <AppContent />;
}

export default App;
