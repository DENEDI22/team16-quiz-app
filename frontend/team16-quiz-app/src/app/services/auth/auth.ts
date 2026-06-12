import { inject, Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Router } from '@angular/router';
import { Observable } from 'rxjs';
import { map, tap } from 'rxjs/operators';

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
  providedIn: 'root',
})
export class AuthService {
  private http = inject(HttpClient);
  private router = inject(Router);

  private apiUrl = 'http://localhost:3000';
  private refreshTimer: ReturnType<typeof setTimeout> | null = null;

  constructor() {
    const token = localStorage.getItem('token');
    if (token) this.scheduleRefresh(token);
  }

  register(data: RegisterRequest) {
    return this.http.post<AuthResponse>(`${this.apiUrl}/register`, data);
  }

  login(data: LoginRequest) {
    return this.http.post<AuthResponse>(`${this.apiUrl}/login`, data).pipe(
      tap((res) => {
        const token = res.data?.token;
        if (token) this.scheduleRefresh(token);
      }),
    );
  }

  /** Refresht den Token und gibt den neuen Token zurück. Wird vom Interceptor und Timer verwendet. */
  refreshToken(): Observable<string> {
    const token = localStorage.getItem('token') ?? '';
    return this.http
      .post<{ token: string }>(`${this.apiUrl}/refresh`, {}, {
        headers: { Authorization: `Bearer ${token}` },
      })
      .pipe(
        map((res) => res.token),
        tap((newToken) => {
          localStorage.setItem('token', newToken);
          this.scheduleRefresh(newToken);
        }),
      );
  }

  isLoggedIn(): boolean {
    return !!localStorage.getItem('token');
  }

  logout(): void {
    if (this.refreshTimer) clearTimeout(this.refreshTimer);
    this.refreshTimer = null;
    localStorage.removeItem('token');
  }

  private scheduleRefresh(token: string): void {
    if (this.refreshTimer) clearTimeout(this.refreshTimer);
    try {
      const { exp } = JSON.parse(atob(token.split('.')[1]));
      const delay = exp * 1000 - Date.now() - 60_000; // 1 min vor Ablauf
      this.refreshTimer = setTimeout(() => this.doRefresh(), Math.max(delay, 0));
    } catch {
      /* ungültiges Token-Format */
    }
  }

  private doRefresh(): void {
    this.refreshToken().subscribe({
      error: () => {
        this.logout();
        this.router.navigate(['/login']);
      },
    });
  }
}
