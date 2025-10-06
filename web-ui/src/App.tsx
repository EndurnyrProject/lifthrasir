import { useState } from "react";
import Login from "./screens/Login";
import ServerSelection from "./screens/ServerSelection";
import CharacterSelection from "./screens/CharacterSelection";
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

function App() {
  const [currentScreen, setCurrentScreen] = useState<AppScreen>("login");
  const [servers, setServers] = useState<ServerInfo[]>([]);

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
    <>
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
    </>
  );
}

export default App;
