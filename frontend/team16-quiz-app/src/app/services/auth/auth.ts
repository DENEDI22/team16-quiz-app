import { inject, Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';

interface RegisterRequest {
  email: string;
  username: string;
  password: string;
}

interface LoginRequest {
  email: string;
  password: string;
}

interface AuthResponse {
  success: boolean;
  data?: {
    token?: string;
    message?: string;
  };
}

@Injectable({
  providedIn: 'root'
})
export class AuthService {
  private http = inject(HttpClient);

  private apiUrl = 'http://localhost:3000';

  register(data: RegisterRequest) {
    return this.http.post<AuthResponse>(
      `${this.apiUrl}/register`,
      data
    );
  }

  login(data: LoginRequest) {
    return this.http.post<AuthResponse>(
      `${this.apiUrl}/login`,
      data
    );
  }
  isLoggedIn(): boolean {
    return !!localStorage.getItem('token');
  }

  logout(): void {
    localStorage.removeItem('token');
  }
}
