type serverInfo = {
  ip: int,
  port: int,
  name: string,
  users: int,
  @as("server_type") serverType: string,
  @as("new_server") newServer: int,
}
