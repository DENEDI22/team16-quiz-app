import { inject, Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { CreateLobbyRequest, Lobby } from '../../models/lobby';

export const MULTIPLAYER_API_URL = '/api/multiplayer';

@Injectable({
  providedIn: 'root',
})
export class LobbyService {
  private http = inject(HttpClient);
  private apiUrl = MULTIPLAYER_API_URL;

  getLobbies() {
    return this.http.get<Lobby[]>(`${this.apiUrl}/lobbies`);
  }

  createLobby(request: CreateLobbyRequest) {
    return this.http.post<Lobby>(`${this.apiUrl}/lobbies/`, request);
  }

  joinLobby(lobbyId: string) {
    return this.http.post<Lobby>(`${this.apiUrl}/lobbies/${lobbyId}/join`, {});
  }

  deleteLobby(lobbyId: string) {
    return this.http.delete(`${this.apiUrl}/lobbies/${lobbyId}`);
  }
}
