import { useState } from "react";
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
  const [servers, setServers] = useState<ServerInfo[]>([]);
  const { backgroundUrl } = useAssets();

  const handleLoginSuccess = (serverList: ServerInfo[]) => {
    setServers(serverList);
    setCurrentScreen("server_selection");
  };

  const handleServerSelected = () => {
    setCurrentScreen("character_selection");
  };

  const handleCharacterSelected = () => {
    // Character selected - transition to game
    setCurrentScreen("in_game");
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
      {backgroundUrl && (
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
          <div style={{ color: "white", padding: "20px" }}>
            Loading game world...
          </div>
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
